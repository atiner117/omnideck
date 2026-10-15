---
title: "Ignore Steam Input's virtual pad while a physical pad is connected"
kind: walkthrough
status: draft
created: 2026-10-14
updated: 2026-10-14
thread: claude-code:implementation-lane (fix/gamepad-ignore-virtual-pads)
---

## Problem
Steam Input reads the DualSense and re-emits it as a uinput "Microsoft X-Box 360 pad 0".
gilrs opens both, so OmniDeck saw every Steam-forwarded press twice (r2d2). A doubled Guide
tap is two `guide-tap` events.

## Change (src-tauri/src/gamepad.rs)
- `is_virtual_sysfs_path`: realpath of `/sys/class/input/eventN` contains `/devices/virtual/`.
- `scan_input_devices`: name + id/vendor + id/product + virtual flag per eventN.
- `pad_is_virtual`: gilrs exposes no device path, so a pad (os_name, vendor, product) is
  virtual only if at least one sysfs device matches and every match is virtual.
- `ignore_virtual_pad`: drop a virtual pad's events only while a physical pad is connected
  and `OMNIDECK_ALLOW_VIRTUAL_PADS` is not `1`. Classification happens on startup and on
  `Connected`, never per event; the check runs before Guide handling.
- `packaging/test-session.sh` sets `OMNIDECK_ALLOW_VIRTUAL_PADS=1` (its pad is uinput).

## Decisions not obvious from the diff
- Not "drop every virtual pad": on handhelds InputPlumber hides the real device and exposes
  only a virtual one; the harness pad is virtual too.
- Ambiguity goes to "physical" so a real pad is never dropped by a name collision.

## Not in this change
The watchdog half of the original report (Steam's install-script reaper read as an exit)
is already fixed in 52de173 on PR #97 / fix/steam-guide-coexistence (`Install=1` rejected,
three consecutive missed polls before an exit), and main has no reaper detector at all
(it lands with PR #96).

## Verify
`cargo test --release gamepad::` (path predicate, matching, policy, fake sysfs tree).
On the couch: Steam running with a DualSense connected, the log shows
`gamepad: GamepadId(n) 'Microsoft X-Box 360 pad 0' is virtual (uinput)` and one Guide tap
opens the deck once.
