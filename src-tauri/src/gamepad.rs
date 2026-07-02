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
            eprintln!("[omnideck] gilrs init FAILED: {e}");
            let _ = handle.emit("gamepad-status", format!("gilrs init FAILED: {e}"));
            return;
        }
    };

    let pads: Vec<String> = gilrs
        .gamepads()
        .map(|(id, g)| format!("{id:?}:{}", g.name()))
        .collect();
    eprintln!("[omnideck] gilrs ready — {} pad(s): {pads:?}", pads.len());
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
    // it. Decided on release, so we time the press. gilrs reads evdev directly, so this
    // works even while the launched app holds window focus.
    const GUIDE_HOLD_CLOSE: std::time::Duration = std::time::Duration::from_millis(800);
    let mut guide_down: Option<std::time::Instant> = None;

    loop {
        while let Some(gilrs::Event { id, event, .. }) = gilrs.next_event() {
            let name = gilrs.gamepad(id).name().to_string();
            match &event {
                gilrs::EventType::ButtonPressed(gilrs::Button::Mode, _) => {
                    guide_down = Some(std::time::Instant::now());
                    continue; // swallow; acted on at release
                }
                gilrs::EventType::ButtonReleased(gilrs::Button::Mode, _) => {
                    let long = guide_down.take().is_some_and(|t| t.elapsed() >= GUIDE_HOLD_CLOSE);
                    if long {
                        if crate::watchdog::return_home() {
                            eprintln!("[omnideck] guide (hold): closed the current app");
                            let _ = handle.emit("app-closed", ());
                        }
                    } else if let Some(what) = crate::switcher::toggle() {
                        eprintln!("[omnideck] guide: app {what}");
                    } // nothing launched — ignore quietly
                    continue; // swallow; never forward Guide as a UI event
                }
                _ => {}
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
        std::thread::sleep(std::time::Duration::from_millis(8));
    }
}
