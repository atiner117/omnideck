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
    // isn't writable; everything else works without it.
    let mut navpad = crate::navpad::NavPad::new();

    // Screensaver idle detection ([screensaver] in config.rs, roadmap Appendix C #1): a
    // single "idle" event after `idle_dim_secs` without pad input past the AXIS_EPS filter,
    // "active" on the next input. The frontend owns the staged dim → Ken-Burns → blank
    // presentation (and the `enabled` gate) — the backend only reports the transition.
    // The threshold is read once at thread start (this thread outlives config edits; a
    // changed idle_dim_secs applies on restart, same as other backend-side config).
    let idle_after =
        std::time::Duration::from_secs(crate::config::load_or_create().screensaver.idle_dim_secs);
    let mut last_input = std::time::Instant::now();
    let mut idle = false;

    loop {
        // Any button/axis event this tick (post-epsilon) counts as screensaver activity.
        let mut saw_input = false;
        while let Some(gilrs::Event { id, event, .. }) = gilrs.next_event() {
            let name = gilrs.gamepad(id).name().to_string();
            match &event {
                gilrs::EventType::ButtonPressed(gilrs::Button::Mode, _) => {
                    guide_down = Some(std::time::Instant::now());
                    saw_input = true; // swallowed below, but it's still user activity
                    continue; // swallow; acted on at threshold (close) or release (switch)
                }
                gilrs::EventType::ButtonReleased(gilrs::Button::Mode, _) => {
                    saw_input = true;
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
            // Real user input (buttons, above-epsilon axis motion) resets the idle clock;
            // Connected/Disconnected/other are not someone touching the pad.
            if matches!(
                &event,
                gilrs::EventType::ButtonPressed(..)
                    | gilrs::EventType::ButtonReleased(..)
                    | gilrs::EventType::ButtonChanged(..)
                    | gilrs::EventType::AxisChanged(..)
            ) {
                saw_input = true;
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
        // Screensaver transitions, checked after the drain so a wake-up press already in
        // the queue wins over an idle expiry in the same tick.
        if saw_input {
            last_input = std::time::Instant::now();
            if idle {
                idle = false;
                tracing::info!("screensaver: input — active");
                let _ = handle.emit("active", ());
            }
        } else if !idle && last_input.elapsed() >= idle_after {
            if crate::mpris::any_playing() {
                // Playback counts as activity (the dim/blank must never trigger mid-movie):
                // restart the countdown so "idle" fires idle_after AFTER playback stops.
                // (MPRIS only — a launched game without a media player is not covered here;
                // the frontend can additionally suppress while an app session is in front.)
                last_input = std::time::Instant::now();
            } else {
                idle = true;
                tracing::info!("screensaver: no pad input for {idle_after:?} — idle");
                let _ = handle.emit("idle", ());
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(8));
    }
}
