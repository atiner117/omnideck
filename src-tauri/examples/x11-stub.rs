// OmniDeck test tool — a minimal, well-behaved X client for the nested-session harness
// (packaging/test-session.sh).
//
// It maps exactly one normal top-level window, sets WM_NAME and _NET_WM_PID (the property the
// switcher uses to recognize a launched app's windows), then sits idle — it NEVER re-maps or
// otherwise touches its own window. That determinism is the point: GTK/Qt dialogs re-assert
// their mapping in reaction to focus changes, which races the switcher's unmap and made the
// harness flaky. A real launched app (kiosk browser, Steam, native player) is a plain
// top-level like this one, not a self-managing dialog — so this is also the truer stand-in.
//
//   cargo run --example x11-stub -- <title>
use std::process;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt, CreateWindowAux, EventMask, PropMode, WindowClass,
};
use x11rb::wrapper::ConnectionExt as _;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let title = std::env::args().nth(1).unwrap_or_else(|| "x11-stub".into());
    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let win = conn.generate_id()?;

    conn.create_window(
        screen.root_depth,
        win,
        screen.root,
        0, 0, 400, 300, 0,
        WindowClass::INPUT_OUTPUT,
        screen.root_visual,
        &CreateWindowAux::new()
            .background_pixel(screen.white_pixel)
            .event_mask(EventMask::EXPOSURE),
    )?;

    // WM_NAME so the harness can find us by title; _NET_WM_PID so the switcher owns us.
    conn.change_property8(PropMode::REPLACE, win, AtomEnum::WM_NAME, AtomEnum::STRING, title.as_bytes())?;
    let net_wm_pid = conn.intern_atom(false, b"_NET_WM_PID")?.reply()?.atom;
    conn.change_property32(PropMode::REPLACE, win, net_wm_pid, AtomEnum::CARDINAL, &[process::id()])?;

    conn.map_window(win)?;
    conn.flush()?;

    // Idle forever, draining events but never acting on them (no self-remap). The harness
    // ends us by killing the process (Ctrl+Alt+End / Guide-hold path, or its own cleanup).
    loop {
        conn.wait_for_event()?;
    }
}
