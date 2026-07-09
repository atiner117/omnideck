// OmniDeck test tool — virtual Xbox-style gamepad over /dev/uinput.
//
// Used by packaging/test-session.sh to exercise the REAL controller input path: the kernel
// device it creates is indistinguishable from a physical pad to gilrs (evdev), including
// the udev hotplug event, so this tests gamepad_loop's Guide press/hold logic end to end.
//
//   cargo run --example virtual-pad -- guide-short        # press+release (< hold threshold)
//   cargo run --example virtual-pad -- guide-hold [ms]    # hold BTN_MODE (default 1000 ms)
//   cargo run --example virtual-pad -- stick-up [ms]      # left stick full up (default 300 ms)
//   cargo run --example virtual-pad -- stick-down [ms]    # left stick full down
//
// Needs write access to /dev/uinput (root:input on Arch — be in the `input` group).
use evdev::uinput::VirtualDeviceBuilder;
use evdev::{AbsInfo, AbsoluteAxisType, AttributeSet, BusType, InputEvent, InputId, Key, UinputAbsSetup};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let action = args.next().unwrap_or_default();
    let arg_ms: Option<u64> = args.next().and_then(|s| s.parse().ok());
    let press_ms = match action.as_str() {
        "guide-short" => 120,
        "guide-hold" => arg_ms.unwrap_or(1000),
        "press-south" => 120, // A button: deck "open card" / activate
        "stick-up" | "stick-down" => arg_ms.unwrap_or(300),
        _ => {
            eprintln!("usage: virtual-pad guide-short | guide-hold [ms] | press-south | stick-up [ms] | stick-down [ms]");
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

    if action.starts_with("stick-") {
        // evdev Y axis: NEGATIVE raw = stick pushed up (gilrs normalizes to LeftStickY +1).
        let raw: i32 = if action == "stick-up" { -32768 } else { 32767 };
        let abs = evdev::EventType::ABSOLUTE;
        dev.emit(&[InputEvent::new(abs, AbsoluteAxisType::ABS_Y.0, raw)])?;
        eprintln!("virtual-pad: ABS_Y {raw} ({press_ms} ms)");
        std::thread::sleep(Duration::from_millis(press_ms));
        dev.emit(&[InputEvent::new(abs, AbsoluteAxisType::ABS_Y.0, 0)])?;
        eprintln!("virtual-pad: ABS_Y recentered");
    } else {
        let key = evdev::EventType::KEY;
        // guide-*  → BTN_MODE (Guide); press-south → BTN_SOUTH (A).
        let btn = if action == "press-south" { Key::BTN_SOUTH } else { Key::BTN_MODE };
        dev.emit(&[InputEvent::new(key, btn.code(), 1)])?;
        eprintln!("virtual-pad: {btn:?} down ({press_ms} ms)");
        std::thread::sleep(Duration::from_millis(press_ms));
        dev.emit(&[InputEvent::new(key, btn.code(), 0)])?;
        eprintln!("virtual-pad: {btn:?} up");
    }

    // Keep the device alive long enough for the reader to drain the release event.
    std::thread::sleep(Duration::from_millis(500));
    Ok(())
}
