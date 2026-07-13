// OmniDeck — controller input (the *real* input path we ship, proven in M0 inside gamescope
// on NVIDIA): gilrs reads evdev on a dedicated std thread (gilrs is !Send, so it cannot live
// in a tokio task) and forwards typed events to the webview via Tauri events.
use serde::Serialize;
use tauri::Emitter;

#[derive(Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
struct GamepadEvent {
    kind: String,
    code: String,
    value: f32,
    gamepad: String,
    name: String,
}

/// Synthetic pad input (the phone remote, remote.rs): emit the same `gamepad-event`
/// press/release pair a physical button produces — `code` uses gilrs debug names
/// ("DPadUp", "South", …) so the webview's existing input handling can't tell the
/// difference and no new frontend path is needed.
pub fn emit_synthetic_button(handle: &tauri::AppHandle, code: &str) {
    for (kind, value) in [("button_pressed", 1.0), ("button_released", 0.0)] {
        let _ = handle.emit(
            "gamepad-event",
            GamepadEvent {
                kind: kind.into(),
                code: code.into(),
                value,
                gamepad: "remote".into(),
                name: "Phone Remote".into(),
            },
        );
    }
}

pub fn gamepad_loop(handle: tauri::AppHandle) {
    let mut gilrs = match gilrs::Gilrs::new() {
        Ok(g) => g,
        Err(e) => {
            tracing::error!("gilrs init FAILED: {e}");
            let _ = handle.emit("gamepad-status", format!("gilrs init FAILED: {e}"));
            return;
        }
    };

    let pads: Vec<String> = gilrs
        .gamepads()
        .map(|(id, g)| format!("{id:?}:{}", g.name()))
        .collect();
    tracing::info!("gilrs ready — {} pad(s): {pads:?}", pads.len());
    let _ = handle.emit(
        "gamepad-status",
        format!("gilrs ready — {} pad(s) connected: {pads:?}", pads.len()),
    );

    // Coalesce noisy AxisChanged: a jittery resting stick streams ~125 events/s/axis; the
    // frontend only needs coarse values for its 0.6 deadband. Emit only when an axis has moved
    // at least AXIS_EPS from its last EMITTED value (cuts IPC volume ~10x on drifty sticks).
    let mut last_axis: std::collections::HashMap<(gilrs::GamepadId, gilrs::Axis), f32> =
        std::collections::HashMap::new();
    const AXIS_EPS: f32 = 0.05;

    // Guide/Home button, console-style: SHORT press switches between OmniDeck and the
    // launched app (it keeps running — music keeps playing); LONG hold (default 800 ms,
    // config `[input] guide_hold_ms`) closes it. The close fires the moment the hold
    // crosses the threshold — while the button is still down, like a console power chord —
    // not at release (M2 feedback: release-time close feels laggy and unconfirmed). The
    // short-press switch still decides at release (that's the only way to know it STAYED
    // short). gilrs reads evdev directly, so all of this works even while the launched app
    // holds window focus. The threshold is read ONCE at thread start (normalize() clamped
    // it 200–5000 ms) — a boot-time knob isn't worth config I/O on a 125 Hz loop.
    let guide_hold_close =
        std::time::Duration::from_millis(crate::config::load_or_create().input.guide_hold_ms);
    let mut guide_down: Option<std::time::Instant> = None; // Some = held, hold not yet fired

    // Virtual keyboard/mouse bridge: while a launched app is in front, the pad drives IT
    // (arrows/Enter/Esc, pointer on the right stick — see navpad.rs). None when /dev/uinput
    // isn't writable; everything else works without it.
    let mut navpad = crate::navpad::NavPad::new();

    loop {
        while let Some(gilrs::Event { id, event, .. }) = gilrs.next_event() {
            let name = gilrs.gamepad(id).name().to_string();
            match &event {
                gilrs::EventType::ButtonPressed(gilrs::Button::Mode, _) => {
                    guide_down = Some(std::time::Instant::now());
                    continue; // swallow; acted on at threshold (close) or release (switch)
                }
                gilrs::EventType::ButtonReleased(gilrs::Button::Mode, _) => {
                    // None here means the hold already fired (or a stray release) — ignore.
                    if guide_down.take().is_some() {
                        // Short press opens/closes the deck switcher (iOS-style app cards);
                        // the frontend owns the overlay and calls deck_open (which hides the
                        // apps so the overlay shows). Guide HOLD still closes-all below.
                        tracing::info!("guide: tap — toggle deck");
                        let _ = handle.emit("guide-tap", ());
                    }
                    continue; // swallow; never forward Guide as a UI event
                }
                _ => {}
            }
            // App in front → the pad drives the app through the uinput bridge. Events are
            // STILL forwarded to the webview below (existing behavior — it's hidden and
            // ignores them); Guide never reaches here.
            if let Some(np) = navpad.as_mut() {
                np.handle(&event);
            }
            // Drop sub-epsilon axis jitter before it crosses the IPC boundary.
            if let gilrs::EventType::AxisChanged(a, v, _) = &event {
                let key = (id, *a);
                if last_axis.get(&key).is_some_and(|p| (*p - *v).abs() < AXIS_EPS) {
                    continue;
                }
                last_axis.insert(key, *v);
            }
            let (kind, code, value) = match event {
                gilrs::EventType::ButtonPressed(b, _) => {
                    ("button_pressed".to_string(), format!("{b:?}"), 1.0)
                }
                gilrs::EventType::ButtonReleased(b, _) => {
                    ("button_released".to_string(), format!("{b:?}"), 0.0)
                }
                gilrs::EventType::ButtonChanged(b, v, _) => {
                    ("button_changed".to_string(), format!("{b:?}"), v)
                }
                gilrs::EventType::AxisChanged(a, v, _) => {
                    // debug-level so RUST_LOG can expose the normalized sign convention
                    // (gilrs: +Y = up) when chasing per-controller inversion reports.
                    tracing::debug!("axis {a:?} = {v:.2}");
                    ("axis_changed".to_string(), format!("{a:?}"), v)
                }
                gilrs::EventType::Connected => ("connected".to_string(), String::new(), 0.0),
                gilrs::EventType::Disconnected => {
                    ("disconnected".to_string(), String::new(), 0.0)
                }
                _ => ("other".to_string(), String::new(), 0.0),
            };
            let _ = handle.emit(
                "gamepad-event",
                GamepadEvent {
                    kind,
                    code,
                    value,
                    gamepad: format!("{id:?}"),
                    name,
                },
            );
        }
        // Threshold check AFTER draining the queue (the loop wakes every 8 ms): a release
        // already sitting in the queue must win — otherwise a ~790 ms press whose release
        // we haven't read yet would misfire as a hold. Fire the close mid-hold and consume
        // the press so the eventual release is a no-op.
        if guide_down.is_some_and(|t| t.elapsed() >= guide_hold_close) {
            guide_down = None;
            if crate::watchdog::return_home() {
                tracing::info!("guide (hold): closed the current app");
                let _ = handle.emit("app-closed", ());
            }
        }
        // Bridge housekeeping each tick: arrow auto-repeat, right-stick pointer motion,
        // and releasing anything held if the app vanished mid-press.
        if let Some(np) = navpad.as_mut() {
            np.tick();
        }
        std::thread::sleep(std::time::Duration::from_millis(8));
    }
}
