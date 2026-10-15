// OmniDeck — controller input (the *real* input path we ship, proven in M0 inside gamescope
// on NVIDIA): gilrs reads evdev on a dedicated std thread (gilrs is !Send, so it cannot live
// in a tokio task) and forwards typed events to the webview via Tauri events.
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
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

/// Set to `1` to accept virtual (uinput) pads even while a physical pad is connected — the
/// nested-session harness drives OmniDeck with its own uinput pad (examples/virtual-pad.rs).
const ALLOW_VIRTUAL_PADS_ENV: &str = "OMNIDECK_ALLOW_VIRTUAL_PADS";

/// One evdev input device as sysfs describes it: the kernel name plus USB-style ids, and
/// whether it hangs off `/devices/virtual/` (uinput — Steam Input's "Microsoft X-Box 360
/// pad 0", InputPlumber, our own harness pad) rather than real hardware.
#[derive(Debug, Clone, PartialEq, Eq)]
struct InputDev {
    name: String,
    vendor: Option<u16>,
    product: Option<u16>,
    is_virtual: bool,
}

/// Does a RESOLVED sysfs path (`realpath /sys/class/input/eventN`) belong to a virtual
/// device? uinput devices are parented at `/sys/devices/virtual/input/inputN`; real pads sit
/// under their bus (`/sys/devices/pci…/usb…`, `…/bluetooth/…`, `…/hid…`).
fn is_virtual_sysfs_path(resolved: &std::path::Path) -> bool {
    resolved.to_string_lossy().contains("/devices/virtual/")
}

/// Every `eventN` under `class_dir` (normally `/sys/class/input`), resolved and identified.
/// Unreadable entries are skipped — this only ever narrows what we filter, never widens it.
fn scan_input_devices(class_dir: &std::path::Path) -> Vec<InputDev> {
    let Ok(rd) = std::fs::read_dir(class_dir) else {
        return Vec::new();
    };
    let hex_id = |dev: &std::path::Path, f: &str| -> Option<u16> {
        let s = std::fs::read_to_string(dev.join("id").join(f)).ok()?;
        u16::from_str_radix(s.trim(), 16).ok()
    };
    let mut out = Vec::new();
    for e in rd.flatten() {
        if !e.file_name().to_string_lossy().starts_with("event") {
            continue;
        }
        let Ok(resolved) = std::fs::canonicalize(e.path()) else {
            continue;
        };
        // eventN/device -> the parent inputN node, which carries name and id/.
        let dev = e.path().join("device");
        let Ok(name) = std::fs::read_to_string(dev.join("name")) else {
            continue;
        };
        out.push(InputDev {
            name: name.trim_end_matches('\n').to_string(),
            vendor: hex_id(&dev, "vendor"),
            product: hex_id(&dev, "product"),
            is_virtual: is_virtual_sysfs_path(&resolved),
        });
    }
    out
}

/// Is the gilrs pad `(os_name, vendor, product)` ONLY backed by virtual evdev devices?
/// gilrs does not expose the device path, so match by kernel name + ids. Ambiguity resolves
/// toward "physical": a pad is virtual only when at least one device matches and every
/// match is virtual, so a real pad that happens to share a name is never dropped.
fn pad_is_virtual(
    name: &str,
    vendor: Option<u16>,
    product: Option<u16>,
    devs: &[InputDev],
) -> bool {
    let id_ok =
        |want: Option<u16>, got: Option<u16>| want.is_none() || got.is_none() || want == got;
    let mut matches = devs
        .iter()
        .filter(|d| d.name == name && id_ok(vendor, d.vendor) && id_ok(product, d.product))
        .peekable();
    matches.peek().is_some() && matches.all(|d| d.is_virtual)
}

/// Drop events from a virtual pad while a physical one is connected. Steam Input re-emits
/// the physical pad it is reading through a uinput "Microsoft X-Box 360 pad 0"; gilrs opens
/// both, so every Steam-forwarded press arrived twice (r2d2 couch box) — and a doubled Guide
/// tap is two `guide-tap`s, i.e. the deck opening and closing again. A virtual pad that is
/// the ONLY pad (InputPlumber on a handheld hides the real device behind one) still works,
/// and `OMNIDECK_ALLOW_VIRTUAL_PADS=1` keeps them all.
fn ignore_virtual_pad(is_virtual: bool, any_physical: bool, allow_virtual: bool) -> bool {
    is_virtual && any_physical && !allow_virtual
}

/// Classify one connected gilrs pad against a fresh sysfs scan.
fn classify_pad(gilrs: &gilrs::Gilrs, id: gilrs::GamepadId) -> bool {
    let pad = gilrs.gamepad(id);
    let devs = scan_input_devices(std::path::Path::new("/sys/class/input"));
    let virt = pad_is_virtual(pad.os_name(), pad.vendor_id(), pad.product_id(), &devs);
    tracing::info!(
        "gamepad: {id:?} '{}' is {}",
        pad.os_name(),
        if virt { "virtual (uinput)" } else { "physical" }
    );
    virt
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

    // Virtual-pad filter (see ignore_virtual_pad): which connected pads are uinput-backed.
    // Re-classified on Connected (sysfs read), dropped on Disconnected — never per event.
    let allow_virtual = std::env::var(ALLOW_VIRTUAL_PADS_ENV).is_ok_and(|v| v == "1");
    let mut pad_virtual: std::collections::HashMap<gilrs::GamepadId, bool> = gilrs
        .gamepads()
        .map(|(id, _)| (id, classify_pad(&gilrs, id)))
        .collect();

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
            match &event {
                gilrs::EventType::Connected => {
                    let v = classify_pad(&gilrs, id);
                    pad_virtual.insert(id, v);
                }
                gilrs::EventType::Disconnected => {
                    // Still let this Disconnected through below if the pad was in use.
                    let was_virtual = pad_virtual.remove(&id).unwrap_or(false);
                    let any_physical = pad_virtual.values().any(|v| !v);
                    if ignore_virtual_pad(was_virtual, any_physical, allow_virtual) {
                        continue;
                    }
                }
                _ => {}
            }
            // FIRST, before Guide handling: a duplicated Guide press would toggle twice.
            if ignore_virtual_pad(
                pad_virtual.get(&id).copied().unwrap_or(false),
                pad_virtual.values().any(|v| !v),
                allow_virtual,
            ) {
                continue;
            }
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
    use super::{
        ignore_virtual_pad, is_virtual_sysfs_path, pad_is_virtual, scan_input_devices, InputDev,
    };
    use std::path::Path;

    #[test]
    fn virtual_sysfs_path_predicate() {
        // Steam Input's pad on the r2d2 couch box (uinput).
        assert!(is_virtual_sysfs_path(Path::new(
            "/sys/devices/virtual/input/input42/event21"
        )));
        // Real pads: USB and Bluetooth DualSense.
        assert!(!is_virtual_sysfs_path(Path::new(
            "/sys/devices/pci0000:00/0000:00:14.0/usb1/1-4/1-4:1.3/0003:054C:0CE6.0007/input/input30/event18"
        )));
        assert!(!is_virtual_sysfs_path(Path::new(
            "/sys/devices/pci0000:00/0000:00:14.0/usb1/1-9/1-9:1.0/bluetooth/hci0/hci0:256/0005:054C:0CE6.000A/input/input35/event19"
        )));
        // An unresolved class link must not count: callers pass the realpath.
        assert!(!is_virtual_sysfs_path(Path::new(
            "/sys/class/input/event21"
        )));
    }

    fn dev(name: &str, vendor: u16, product: u16, is_virtual: bool) -> InputDev {
        InputDev {
            name: name.into(),
            vendor: Some(vendor),
            product: Some(product),
            is_virtual,
        }
    }

    #[test]
    fn pad_matching_resolves_ambiguity_toward_physical() {
        let devs = [
            dev(
                "Sony Interactive Entertainment DualSense Wireless Controller",
                0x054c,
                0x0ce6,
                false,
            ),
            dev("Microsoft X-Box 360 pad 0", 0x045e, 0x028e, true),
        ];
        assert!(pad_is_virtual(
            "Microsoft X-Box 360 pad 0",
            Some(0x045e),
            Some(0x028e),
            &devs
        ));
        assert!(!pad_is_virtual(
            "Sony Interactive Entertainment DualSense Wireless Controller",
            Some(0x054c),
            Some(0x0ce6),
            &devs
        ));
        // Unknown to sysfs -> treated as physical (never filter what we can't identify).
        assert!(!pad_is_virtual("Mystery Pad", None, None, &devs));
        // Same name on a real device and a virtual one -> physical.
        let both = [dev("Pad", 1, 2, true), dev("Pad", 1, 2, false)];
        assert!(!pad_is_virtual("Pad", Some(1), Some(2), &both));
        // Ids disambiguate same-named devices.
        let split = [dev("Pad", 1, 2, true), dev("Pad", 3, 4, false)];
        assert!(pad_is_virtual("Pad", Some(1), Some(2), &split));
        assert!(!pad_is_virtual("Pad", Some(3), Some(4), &split));
    }

    #[test]
    fn ignore_policy() {
        // Steam's duplicate next to the real pad: dropped.
        assert!(ignore_virtual_pad(true, true, false));
        // Only pad is virtual (InputPlumber handheld, harness): kept.
        assert!(!ignore_virtual_pad(true, false, false));
        // Override keeps everything; physical pads are never dropped.
        assert!(!ignore_virtual_pad(true, true, true));
        assert!(!ignore_virtual_pad(false, true, false));
    }

    #[test]
    fn scan_reads_a_fake_sysfs_tree() {
        let root = std::env::temp_dir().join(format!("omnideck-sysfs-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mk =
            |parent: &str, input: &str, event: &str, name: &str, vendor: &str, product: &str| {
                let node = root.join(parent).join(input);
                std::fs::create_dir_all(node.join("id")).unwrap();
                std::fs::create_dir_all(node.join(event)).unwrap();
                std::fs::write(node.join("name"), format!("{name}\n")).unwrap();
                std::fs::write(node.join("id/vendor"), format!("{vendor}\n")).unwrap();
                std::fs::write(node.join("id/product"), format!("{product}\n")).unwrap();
                // eventN/device -> ../ (the inputN node), as in real sysfs.
                std::os::unix::fs::symlink("..", node.join(event).join("device")).unwrap();
                let class = root.join("class/input");
                std::fs::create_dir_all(&class).unwrap();
                std::os::unix::fs::symlink(node.join(event), class.join(event)).unwrap();
            };
        mk(
            "devices/virtual/input",
            "input42",
            "event21",
            "Microsoft X-Box 360 pad 0",
            "045e",
            "028e",
        );
        mk(
            "devices/pci0000:00/usb1/1-4/input",
            "input30",
            "event18",
            "DualSense",
            "054c",
            "0ce6",
        );
        let mut devs = scan_input_devices(&root.join("class/input"));
        devs.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(
            devs,
            vec![
                dev("DualSense", 0x054c, 0x0ce6, false),
                dev("Microsoft X-Box 360 pad 0", 0x045e, 0x028e, true),
            ]
        );
        assert!(pad_is_virtual(
            "Microsoft X-Box 360 pad 0",
            Some(0x045e),
            Some(0x028e),
            &devs
        ));
        let _ = std::fs::remove_dir_all(&root);
    }
}
