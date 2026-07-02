// OmniDeck — global session hotkey: Ctrl+Alt+Home = close the launched app, return home.
//
// Inside a gamescope session a launched fullscreen app (browser PWA, native player) takes
// window focus, so nothing typed on the keyboard ever reaches OmniDeck's webview — a
// keyboard-only user has no way back (M2 finding). The gamepad Guide button escapes this
// because gilrs reads evdev directly; the keyboard twin is an X *key grab* on the root
// window: grabs are resolved by the X server before focus delivery, so our client receives
// the chord no matter which app is focused. Everything in the session is an Xwayland client
// (gamescope is an X compositor at heart), so this covers all launched apps there.
//
// Deliberately session-only: on a normal desktop, OmniDeck is just a window and shouldn't
// own a system-wide chord (set OMNIDECK_FORCE_HOTKEY=1 to test the grab on a desktop X11
// session). Chord choice: Ctrl+Alt+Home — memorable ("go home"), three keys so a game or
// on-screen keyboard can't hit it by accident, and not among gamescope's own Super binds.
use tauri::Emitter;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, GrabMode, ModMask};
use x11rb::protocol::Event;

const XK_HOME: u32 = 0xff50; // keysym for the Home key

pub fn spawn_if_session(app: tauri::AppHandle) {
    let in_gamescope = std::env::var_os("GAMESCOPE_WAYLAND_DISPLAY").is_some();
    if !in_gamescope && std::env::var_os("OMNIDECK_FORCE_HOTKEY").is_none() {
        return;
    }
    std::thread::spawn(move || {
        if let Err(e) = run(app) {
            eprintln!("[omnideck] hotkey: {e} — Ctrl+Alt+Home return-home unavailable");
        }
    });
}

/// Connect to $DISPLAY and grab Ctrl+Alt+Home on the root window, in all lock-modifier
/// variants (the classic 4-grab: plain, NumLock, CapsLock, both) so the chord works
/// regardless of lock state. Keycode is resolved from the server's keyboard mapping
/// (never hardcode: keycodes are layout/server specific even though Home is stable).
fn connect_and_grab() -> Result<x11rb::rust_connection::RustConnection, Box<dyn std::error::Error>> {
    let (conn, screen_num) = x11rb::connect(None)?; // gamescope's Xwayland via $DISPLAY
    let root = conn.setup().roots[screen_num].root;

    let setup = conn.setup();
    let (min_kc, max_kc) = (setup.min_keycode, setup.max_keycode);
    let mapping = conn.get_keyboard_mapping(min_kc, max_kc - min_kc + 1)?.reply()?;
    let per = mapping.keysyms_per_keycode as usize;
    let keycode = mapping
        .keysyms
        .chunks(per)
        .position(|syms| syms.contains(&XK_HOME))
        .map(|i| min_kc + i as u8)
        .ok_or("keyboard has no Home key")?;

    let base = ModMask::CONTROL | ModMask::M1;
    for locks in [ModMask::from(0u16), ModMask::M2, ModMask::LOCK, ModMask::M2 | ModMask::LOCK] {
        conn.grab_key(true, root, base | locks, keycode, GrabMode::ASYNC, GrabMode::ASYNC)?;
    }
    conn.flush()?;
    Ok(conn)
}

fn run(app: tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let conn = connect_and_grab()?;
    eprintln!("[omnideck] hotkey: Ctrl+Alt+Home grabbed (close launched app / return home)");

    loop {
        // Only our grabbed chord is ever delivered here, so any KeyPress means "go home".
        if let Event::KeyPress(_) = conn.wait_for_event()? {
            let closed = crate::watchdog::return_home();
            eprintln!("[omnideck] hotkey: Ctrl+Alt+Home — {}", if closed { "closed the current app" } else { "no app to close" });
            if closed {
                let _ = app.emit("app-closed", ());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// End-to-end grab check against a live X server: grab the chord, inject it with
    /// xdotool (XTEST), and assert the KeyPress is delivered to us. Ignored by default
    /// (needs $DISPLAY + xdotool); run against the target server with:
    ///   DISPLAY=:0 cargo test grab_smoke -- --ignored
    #[test]
    #[ignore]
    fn grab_smoke() {
        let conn = connect_and_grab().expect("connect + grab failed");
        std::process::Command::new("xdotool")
            .args(["key", "ctrl+alt+Home"])
            .status()
            .expect("xdotool not available");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Ok(Some(Event::KeyPress(_))) = conn.poll_for_event() {
                return; // chord delivered through the grab — pass
            }
            assert!(std::time::Instant::now() < deadline, "chord was not delivered within 5s");
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}
