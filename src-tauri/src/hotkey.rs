// OmniDeck — global session hotkeys:
//   Ctrl+Alt+Home = switch between OmniDeck and the launched app (hide/show — it keeps running)
//   Ctrl+Alt+End  = close the launched app and return home
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
// own system-wide chords (set OMNIDECK_FORCE_HOTKEY=1 to test the grabs on a desktop X11
// session). Chord choice: three keys so a game or on-screen keyboard can't hit them by
// accident, not among gamescope's own Super binds, and mnemonic: Home = go home, End = end.
use tauri::Emitter;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, GrabMode, ModMask};
use x11rb::protocol::Event;

const XK_HOME: u32 = 0xff50; // nav-cluster Home
const XK_KP_HOME: u32 = 0xff95; // numpad Home (7 with NumLock off)
const XK_END: u32 = 0xff57; // nav-cluster End
const XK_KP_END: u32 = 0xff9c; // numpad End (1 with NumLock off)

pub fn spawn_if_session(app: tauri::AppHandle) {
    let in_gamescope = std::env::var_os("GAMESCOPE_WAYLAND_DISPLAY").is_some();
    if !in_gamescope && std::env::var_os("OMNIDECK_FORCE_HOTKEY").is_none() {
        return;
    }
    std::thread::spawn(move || {
        if let Err(e) = run(app) {
            tracing::error!("hotkey: {e} — Ctrl+Alt+Home/End unavailable");
        }
    });
}

/// The grab setup: which keycodes carry a keysym set, grabbed with Ctrl+Alt in all
/// lock-modifier variants (the classic 4-grab: plain, NumLock, CapsLock, both).
struct Grabs {
    conn: x11rb::rust_connection::RustConnection,
    home_keycodes: Vec<u8>,
    end_keycodes: Vec<u8>,
}

/// Connect to $DISPLAY and grab Ctrl+Alt+{Home,End} on the root window. Keycodes are
/// resolved from the server's keyboard mapping (never hardcode: keycodes are layout/server
/// specific), including the numpad variants.
fn connect_and_grab() -> Result<Grabs, Box<dyn std::error::Error>> {
    let (conn, screen_num) = x11rb::connect(None)?; // gamescope's Xwayland via $DISPLAY
    let root = conn.setup().roots[screen_num].root;

    let setup = conn.setup();
    let (min_kc, max_kc) = (setup.min_keycode, setup.max_keycode);
    let mapping = conn.get_keyboard_mapping(min_kc, max_kc - min_kc + 1)?.reply()?;
    let per = mapping.keysyms_per_keycode as usize;
    let keycodes_for = |syms: &[u32]| -> Vec<u8> {
        mapping
            .keysyms
            .chunks(per)
            .enumerate()
            .filter(|(_, chunk)| chunk.iter().any(|s| syms.contains(s)))
            .map(|(i, _)| min_kc + i as u8)
            .collect()
    };
    let home_keycodes = keycodes_for(&[XK_HOME, XK_KP_HOME]);
    let end_keycodes = keycodes_for(&[XK_END, XK_KP_END]);
    if home_keycodes.is_empty() && end_keycodes.is_empty() {
        return Err("keyboard has no Home/End keys".into());
    }

    let base = ModMask::CONTROL | ModMask::M1;
    for &keycode in home_keycodes.iter().chain(&end_keycodes) {
        for locks in [ModMask::from(0u16), ModMask::M2, ModMask::LOCK, ModMask::M2 | ModMask::LOCK] {
            conn.grab_key(true, root, base | locks, keycode, GrabMode::ASYNC, GrabMode::ASYNC)?;
        }
    }
    conn.flush()?;
    Ok(Grabs { conn, home_keycodes, end_keycodes })
}

fn run(app: tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let grabs = connect_and_grab()?;
    tracing::info!("hotkey: grabbed Ctrl+Alt+Home (switch) and Ctrl+Alt+End (close app)");

    loop {
        // Only our grabbed chords are delivered here; the keycode says which one.
        let Event::KeyPress(e) = grabs.conn.wait_for_event()? else { continue };
        if grabs.home_keycodes.contains(&e.detail) {
            match crate::switcher::toggle() {
                Some(what) => tracing::info!("hotkey: Ctrl+Alt+Home — app {what}"),
                None => tracing::info!("hotkey: Ctrl+Alt+Home — no app to switch to"),
            }
        } else if grabs.end_keycodes.contains(&e.detail) {
            let closed = crate::watchdog::return_home();
            tracing::info!("hotkey: Ctrl+Alt+End — {}", if closed { "closed the current app" } else { "no app to close" });
            if closed {
                let _ = app.emit("app-closed", ());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// End-to-end grab check against a live X server: grab the chords, inject them with
    /// xdotool (XTEST), and assert both KeyPresses are delivered to us with the right
    /// keycode class. Ignored by default (needs $DISPLAY + xdotool); run against the
    /// target server with:  DISPLAY=:0 cargo test grab_smoke -- --ignored
    #[test]
    #[ignore]
    fn grab_smoke() {
        let grabs = connect_and_grab().expect("connect + grab failed");
        for (chord, expected) in [("ctrl+alt+Home", &grabs.home_keycodes), ("ctrl+alt+End", &grabs.end_keycodes)] {
            std::process::Command::new("xdotool")
                .args(["key", chord])
                .status()
                .expect("xdotool not available");
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                if let Ok(Some(Event::KeyPress(e))) = grabs.conn.poll_for_event() {
                    assert!(expected.contains(&e.detail), "{chord} delivered an unexpected keycode {}", e.detail);
                    break;
                }
                assert!(std::time::Instant::now() < deadline, "{chord} was not delivered within 5s");
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
}
