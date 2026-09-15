// OmniDeck test tool — virtual Xbox-style gamepad over /dev/uinput.
//
// Used by packaging/test-session.sh to exercise the REAL controller input path: the kernel
// device it creates is indistinguishable from a physical pad to gilrs (evdev), including
// the udev hotplug event, so this tests gamepad_loop's Guide press/hold logic end to end.
//
//   cargo run --example virtual-pad -- guide-short        # press+release (< hold threshold)
//   cargo run --example virtual-pad -- guide-hold [ms]    # hold BTN_MODE (default 1000 ms)
//   cargo run --example virtual-pad -- press-south        # A button
//   cargo run --example virtual-pad -- stick-up [ms]      # left stick full up (default 300 ms)
//   cargo run --example virtual-pad -- stick-down [ms]    # left stick full down
//   cargo run --example virtual-pad -- seq <steps>        # ONE device for a whole sequence
//
// `seq` steps are comma-separated `action[:ms]` items, so a scripted couch test does not
// re-hotplug a pad (and make Steam Input re-enumerate) for every press:
//   south|east|north|west|start|select|guide[:ms]   button press (default 120 ms; guide
//                                                   past the hold threshold = close-all)
//   up|down|left|right[:ms]                         d-pad (HAT0) tap (default 150 ms)
//   stick-up|stick-down|stick-left|stick-right[:ms] left stick full deflection (default 300)
//   sleep:ms                                        pause with the device alive
//   e.g.  seq south,sleep:2000,down,down,sleep:500,guide,sleep:1500,guide
//
// Needs write access to /dev/uinput (root:input on Arch — be in the `input` group).
use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AbsInfo, AbsoluteAxisType, AttributeSet, BusType, InputEvent, InputId, Key, UinputAbsSetup};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let action = args.next().unwrap_or_default();
    let arg = args.next();
    let arg_ms: Option<u64> = arg.as_deref().and_then(|s| s.parse().ok());
    let press_ms = match action.as_str() {
        "guide-short" => 120,
        "guide-hold" => arg_ms.unwrap_or(1000),
        "press-south" => 120, // A button: deck "open card" / activate
        "stick-up" | "stick-down" => arg_ms.unwrap_or(300),
        "seq" => 0,
        _ => {
            eprintln!("usage: virtual-pad guide-short | guide-hold [ms] | press-south | stick-up [ms] | stick-down [ms] | seq <steps>");
            std::process::exit(2);
        }
    };

    // The standard xpad capability set; udev's input_id classifies BTN_GAMEPAD (=BTN_SOUTH)
    // devices as joysticks, which is what makes gilrs pick it up.
    let mut keys = AttributeSet::<Key>::new();
    for k in [
        Key::BTN_SOUTH, Key::BTN_EAST, Key::BTN_NORTH, Key::BTN_WEST,
        Key::BTN_TL, Key::BTN_TR, Key::BTN_SELECT, Key::BTN_START,
        Key::BTN_MODE, Key::BTN_THUMBL, Key::BTN_THUMBR,
    ] {
        keys.insert(k);
    }
    let stick = AbsInfo::new(0, -32768, 32767, 16, 128, 1);
    let trigger = AbsInfo::new(0, 0, 255, 0, 0, 1);
    let dpad = AbsInfo::new(0, -1, 1, 0, 0, 1);

    let mut dev = VirtualDeviceBuilder::new()?
        .name("OmniDeck Virtual Pad")
        // Xbox 360 ids so controller databases recognize the layout (BTN_MODE = Guide).
        .input_id(InputId::new(BusType::BUS_USB, 0x045e, 0x028e, 0x110))
        .with_keys(&keys)?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisType::ABS_X, stick))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisType::ABS_Y, stick))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisType::ABS_RX, stick))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisType::ABS_RY, stick))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisType::ABS_Z, trigger))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisType::ABS_RZ, trigger))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisType::ABS_HAT0X, dpad))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisType::ABS_HAT0Y, dpad))?
        .build()?;

    // Give udev + gilrs's hotplug monitor time to enumerate the new pad before pressing.
    std::thread::sleep(Duration::from_millis(1500));

    if action == "seq" {
        let steps = arg.unwrap_or_default();
        for step in steps.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let (name, ms) = match step.split_once(':') {
                Some((n, m)) => (n, m.parse::<u64>().ok()),
                None => (step, None),
            };
            run_step(&mut dev, name, ms)?;
        }
    } else if action.starts_with("stick-") {
        run_step(&mut dev, &action, Some(press_ms))?;
    } else {
        // guide-*  → BTN_MODE (Guide); press-south → BTN_SOUTH (A).
        let name = if action == "press-south" { "south" } else { "guide" };
        run_step(&mut dev, name, Some(press_ms))?;
    }

    // Keep the device alive long enough for the reader to drain the release event.
    std::thread::sleep(Duration::from_millis(500));
    Ok(())
}

/// One scripted step on the live device. `ms` = press/deflection length (or the sleep).
fn run_step(dev: &mut VirtualDevice, name: &str, ms: Option<u64>) -> Result<(), Box<dyn std::error::Error>> {
    let key = evdev::EventType::KEY;
    let abs = evdev::EventType::ABSOLUTE;
    let button = |k: Key| -> Option<Key> { Some(k) };
    match name {
        "sleep" => {
            let d = ms.unwrap_or(500);
            eprintln!("virtual-pad: sleep {d} ms");
            std::thread::sleep(Duration::from_millis(d));
        }
        "up" | "down" | "left" | "right" => {
            let (axis, v) = match name {
                "up" => (AbsoluteAxisType::ABS_HAT0Y, -1),
                "down" => (AbsoluteAxisType::ABS_HAT0Y, 1),
                "left" => (AbsoluteAxisType::ABS_HAT0X, -1),
                _ => (AbsoluteAxisType::ABS_HAT0X, 1),
            };
            let d = ms.unwrap_or(150);
            dev.emit(&[InputEvent::new(abs, axis.0, v)])?;
            eprintln!("virtual-pad: dpad {name} ({d} ms)");
            std::thread::sleep(Duration::from_millis(d));
            dev.emit(&[InputEvent::new(abs, axis.0, 0)])?;
            std::thread::sleep(Duration::from_millis(80));
        }
        "stick-up" | "stick-down" | "stick-left" | "stick-right" => {
            // evdev Y axis: NEGATIVE raw = stick pushed up (gilrs normalizes to LeftStickY +1).
            let (axis, raw) = match name {
                "stick-up" => (AbsoluteAxisType::ABS_Y, -32768),
                "stick-down" => (AbsoluteAxisType::ABS_Y, 32767),
                "stick-left" => (AbsoluteAxisType::ABS_X, -32768),
                _ => (AbsoluteAxisType::ABS_X, 32767),
            };
            let d = ms.unwrap_or(300);
            dev.emit(&[InputEvent::new(abs, axis.0, raw)])?;
            eprintln!("virtual-pad: {axis:?} {raw} ({d} ms)");
            std::thread::sleep(Duration::from_millis(d));
            dev.emit(&[InputEvent::new(abs, axis.0, 0)])?;
            eprintln!("virtual-pad: {axis:?} recentered");
            std::thread::sleep(Duration::from_millis(80));
        }
        _ => {
            let btn = match name {
                "south" => button(Key::BTN_SOUTH),
                "east" => button(Key::BTN_EAST),
                "north" => button(Key::BTN_NORTH),
                "west" => button(Key::BTN_WEST),
                "start" => button(Key::BTN_START),
                "select" => button(Key::BTN_SELECT),
                "guide" | "guide-hold" => button(Key::BTN_MODE),
                _ => None,
            };
            let Some(btn) = btn else {
                eprintln!("virtual-pad: unknown step '{name}'");
                std::process::exit(2);
            };
            let d = ms.unwrap_or(120);
            dev.emit(&[InputEvent::new(key, btn.code(), 1)])?;
            eprintln!("virtual-pad: {btn:?} down ({d} ms)");
            std::thread::sleep(Duration::from_millis(d));
            dev.emit(&[InputEvent::new(key, btn.code(), 0)])?;
            eprintln!("virtual-pad: {btn:?} up");
            std::thread::sleep(Duration::from_millis(80));
        }
    }
    Ok(())
}
