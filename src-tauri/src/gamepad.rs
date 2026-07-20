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
    // launched app (it keeps running — music keeps playing); LONG hold (>= 800 ms) closes
    // it. The close fires the moment the hold crosses the threshold — while the button is
    // still down, like a console power chord — not at release (M2 feedback: release-time
    // close feels laggy and unconfirmed). The short-press switch still decides at release
    // (that's the only way to know it STAYED short). gilrs reads evdev directly, so all of
    // this works even while the launched app holds window focus.
    const GUIDE_HOLD_CLOSE: std::time::Duration = std::time::Duration::from_millis(800);
    let mut guide_down: Option<std::time::Instant> = None; // Some = held, hold not yet fired

    // Virtual keyboard/mouse bridge: while a launched app is in front, the pad drives IT
    // (arrows/Enter/Esc, pointer on the right stick — see navpad.rs). None when /dev/uinput
    // isn't writable; everything else works without it. Only built where it can ever fire —
    // on a plain desktop the activation gate never passes, so constructing it just left a
    // phantom kernel input device registered for the whole run.
    let mut navpad = if crate::switcher::session_ok() {
        crate::navpad::NavPad::new()
    } else {
        tracing::info!("navpad: not in a gamescope session — virtual input bridge not created");
        None
    };

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
            // App in front → the pad drives the app through the uinput bridge, and the
            // event is CONSUMED. The hidden dashboard's handler has no app-in-front gate,
            // so forwarding the same press also activated tiles / toggled favorites /
            // opened modals behind the app the user was driving. Guide never reaches here.
            if let Some(np) = navpad.as_mut() {
                if np.handle(&event) {
                    continue;
                }
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
        if guide_down.is_some_and(|t| t.elapsed() >= GUIDE_HOLD_CLOSE) {
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
