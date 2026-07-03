// OmniDeck test tool — virtual Xbox-style gamepad over /dev/uinput.
//
// Used by packaging/test-session.sh to exercise the REAL controller input path: the kernel
// device it creates is indistinguishable from a physical pad to gilrs (evdev), including
// the udev hotplug event, so this tests gamepad_loop's Guide press/hold logic end to end.
//
//   cargo run --example virtual-pad -- guide-short        # press+release (< hold threshold)
//   cargo run --example virtual-pad -- guide-hold [ms]    # hold BTN_MODE (default 1000 ms)
//
// Needs write access to /dev/uinput (root:input on Arch — be in the `input` group).
use evdev::uinput::VirtualDeviceBuilder;
use evdev::{AbsInfo, AbsoluteAxisType, AttributeSet, BusType, InputEvent, InputId, Key, UinputAbsSetup};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let action = args.next().unwrap_or_default();
    let hold_ms: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1000);
    let press_ms = match action.as_str() {
        "guide-short" => 120,
        "guide-hold" => hold_ms,
        _ => {
            eprintln!("usage: virtual-pad guide-short | guide-hold [ms]");
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

    let key = evdev::EventType::KEY;
    dev.emit(&[InputEvent::new(key, Key::BTN_MODE.code(), 1)])?;
    eprintln!("virtual-pad: BTN_MODE down ({press_ms} ms)");
    std::thread::sleep(Duration::from_millis(press_ms));
    dev.emit(&[InputEvent::new(key, Key::BTN_MODE.code(), 0)])?;
    eprintln!("virtual-pad: BTN_MODE up");

    // Keep the device alive long enough for the reader to drain the release event.
    std::thread::sleep(Duration::from_millis(500));
    Ok(())
}
