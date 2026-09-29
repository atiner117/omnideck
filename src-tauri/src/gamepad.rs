// OmniDeck — controller input (the *real* input path we ship, proven in M0 inside gamescope
// on NVIDIA): gilrs reads evdev on a dedicated std thread (gilrs is !Send, so it cannot live
// in a tokio task) and forwards typed events to the webview via Tauri events.
use serde::Serialize;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::Emitter;

/// One gesture across all connected controllers. A second pad's release cannot dismiss the
/// first pad's hold, and the hold event fires only once even if two buttons remain down.
struct GuideGesture<Id> {
    pressed: HashMap<Id, Instant>,
    held: bool,
}

impl<Id: Eq + Hash> GuideGesture<Id> {
    fn new() -> Self { Self { pressed: HashMap::new(), held: false } }

    fn press(&mut self, id: Id, now: Instant) {
        if self.pressed.is_empty() { self.held = false; }
        self.pressed.entry(id).or_insert(now);
    }

    fn release(&mut self, id: &Id) -> bool {
        let was_pressed = self.pressed.remove(id).is_some();
        was_pressed && self.pressed.is_empty() && !self.held
    }

    fn disconnect(&mut self, id: &Id) { self.pressed.remove(id); }

    fn tick(&mut self, now: Instant, threshold: Duration) -> bool {
        if self.held || !self.pressed.values().any(|t| now.duration_since(*t) >= threshold) {
            return false;
        }
        self.held = true;
        true
    }
}

#[derive(Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
struct GamepadEvent {
    kind: String,
    code: String,
    value: f32,
    gamepad: String,
    name: String,
}

/// Set by `notify_activity`, consumed (swap-to-false) once per gamepad-loop tick. Lets
/// non-pad input reset the screensaver idle clock without the loop owning any other
/// input device.
static EXTERNAL_ACTIVITY: AtomicBool = AtomicBool::new(false);

/// Non-pad user activity (keyboard/mouse in the webview): the frontend calls this —
/// throttled — from its DOM keydown/pointermove handlers so desktop-mode use without a
/// pad doesn't trip the screensaver. See the idle-detection comment in `gamepad_loop`
/// for what this does and doesn't cover.
#[tauri::command]
pub fn notify_activity() {
    EXTERNAL_ACTIVITY.store(true, Ordering::Relaxed);
}

/// Synthetic pad input (the phone remote, remote.rs): emit the same `gamepad-event`
/// press/release pair a physical button produces — `code` uses gilrs debug names
/// ("DPadUp", "South", …) so the webview's existing input handling can't tell the
/// difference and no new frontend path is needed.
pub fn emit_synthetic_button(handle: &tauri::AppHandle, code: &str) {
    // Remote presses bypass gilrs, so they'd never reset the screensaver's idle clock —
    // count them as external activity like DOM input.
    EXTERNAL_ACTIVITY.store(true, Ordering::Relaxed);
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

    // Guide/Home: tap toggles Home / the last running app; hold opens the task overview
    // at the threshold, while still pressed. Neither gesture kills an app. Track presses per
    // controller: a release from a second (or virtual) pad must not turn another pad's hold
    // into a tap. One gesture emits only once even when two pads overlap.
    let guide_hold = Duration::from_millis(crate::config::load_or_create().input.guide_hold_ms);
    let mut guide = GuideGesture::<gilrs::GamepadId>::new();

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

    // Screensaver idle detection ([screensaver] in config.rs, roadmap Appendix C #1): a
    // single "idle" event after `idle_dim_secs` without user input, "active" on the next
    // input. The frontend owns the staged dim → Ken-Burns → blank presentation (and the
    // `enabled` gate) — the backend only reports the transition.
    //
    // What counts as input: pad events past the AXIS_EPS filter, plus anything reported
    // via `notify_activity` (the webview's own DOM keydown/pointermove — covers desktop-
    // mode keyboard/mouse use, since the webview has focus in exactly that case).
    // KNOWN LIMITATION: keyboard/mouse input that goes to a LAUNCHED app (which holds
    // focus, so neither the pad bridge nor the webview sees it) does not reset the clock —
    // the backend owns no keyboard/mouse device. The frontend overlay (#29) compensates
    // locally: it suppresses/resets on its own input events and while an app session is
    // in front.
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
                    guide.press(id, Instant::now());
                    saw_input = true;
                    continue;
                }
                gilrs::EventType::ButtonReleased(gilrs::Button::Mode, _) => {
                    saw_input = true;
                    if guide.release(&id) {
                        tracing::info!("guide: tap — Home / last app");
                        let _ = handle.emit("guide-tap", ());
                    }
                    continue; // Guide is never forwarded to a launched app
                }
                gilrs::EventType::Disconnected => {
                    guide.disconnect(&id); // don't hold forever after Bluetooth drops
                }
                _ => {}
            }
            // Keep axis diagnostics observable even when the navpad consumes the event.
            // The nested-session stick test drives a focused app, so logging only on the
            // dashboard forwarding path would incorrectly report that gilrs never saw it.
            if let gilrs::EventType::AxisChanged(a, v, _) = &event {
                tracing::debug!("axis {a:?} = {v:.2}");
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
        // Check after draining the queue: a release already queued must win over a hold.
        // Keep the pressed entries until release, so a hold can fire once while held and
        // every subsequent release is a no-op.
        if guide.tick(Instant::now(), guide_hold) {
            tracing::info!("guide: hold — task overview");
            let _ = handle.emit("guide-hold", ());
        }
        // Bridge housekeeping each tick: arrow auto-repeat, right-stick pointer motion,
        // and releasing anything held if the app vanished mid-press.
        if let Some(np) = navpad.as_mut() {
            np.tick();
        }
        // Screensaver transitions, checked after the drain so a wake-up press already in
        // the queue wins over an idle expiry in the same tick. Non-pad activity (webview
        // keyboard/mouse via notify_activity) counts the same as pad input.
        if EXTERNAL_ACTIVITY.swap(false, Ordering::Relaxed) {
            saw_input = true;
        }
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

#[cfg(test)]
mod tests {
    use super::GuideGesture;
    use std::time::{Duration, Instant};

    #[test]
    fn tap_and_hold_are_exclusive_and_hold_fires_before_release() {
        let start = Instant::now();
        let mut guide = GuideGesture::<u8>::new();
        guide.press(1, start);
        assert!(!guide.tick(start + Duration::from_millis(799), Duration::from_millis(800)));
        assert!(guide.release(&1));
        guide.press(1, start);
        assert!(guide.tick(start + Duration::from_millis(800), Duration::from_millis(800)));
        assert!(!guide.tick(start + Duration::from_secs(2), Duration::from_millis(800)));
        assert!(!guide.release(&1));
    }

    #[test]
    fn overlapping_pads_and_disconnect_never_emit_a_stray_tap() {
        let start = Instant::now();
        let mut guide = GuideGesture::<u8>::new();
        guide.press(1, start);
        guide.press(2, start + Duration::from_millis(100));
        assert!(!guide.release(&2));
        assert!(guide.tick(start + Duration::from_millis(800), Duration::from_millis(800)));
        guide.disconnect(&1);
        assert!(!guide.release(&1));
        guide.press(2, start + Duration::from_secs(1));
        assert!(guide.release(&2));
    }
}
