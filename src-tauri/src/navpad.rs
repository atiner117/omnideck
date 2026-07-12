// OmniDeck — navpad: gamepad → virtual keyboard/mouse bridge for launched apps.
//
// The reason OmniDeck exists is "navigate everything from the couch" — but a launched
// PWA/browser only understands keyboard and mouse. This module creates a virtual
// keyboard+mouse over /dev/uinput (same mechanism as examples/virtual-pad.rs, in the
// other direction) and, WHILE A LAUNCHED APP IS IN FRONT, translates the pad:
//
//   dpad / left stick   → arrow-key pulses with console-style repeat (TV-UI navigation;
//                          Jellyfin-web, YouTube TV, etc. are fully arrow-driven)
//   right stick         → mouse pointer (squared response curve) — the primary way to
//                          navigate arbitrary web UIs, the PlayStation-browser answer
//   A (South)           → left click   (select where the pointer is — the main action)
//   B (East)            → Escape       (back in every TV-style web UI)
//   X (West)            → Enter        (activate the focused element in keyboard/spatial UIs)
//   Y (North)           → Space        (play/pause in players)
//   R2 / L2             → left / right mouse button (R2 held = press-and-hold / drag)
//   L1 / R1             → scroll wheel up / down (with repeat)
//
// Activation is the switcher's ground truth (an owned window is viewable ⇒ gamescope has
// it focused ⇒ input belongs to the app), cached briefly; when OmniDeck itself is in
// front the bridge is inert and the webview keeps its own gamepad handling. The Guide
// button never reaches here (gamepad_loop consumes it for switch/close).
//
// Input delivery is via the kernel, so it works for any X11 client under gamescope's
// Xwayland — Chromium, Firefox, mpv, Qt — with zero per-app integration.
use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{AttributeSet, EventType, InputEvent, Key, RelativeAxisType};
use std::time::{Duration, Instant};

const ACTIVE_CACHE: Duration = Duration::from_millis(300);
const REPEAT_FIRST: Duration = Duration::from_millis(400);
const REPEAT_NEXT: Duration = Duration::from_millis(90);
const WHEEL_REPEAT: Duration = Duration::from_millis(120);
/// Stick deflection that engages / releases a direction (hysteresis so a stick resting
/// near the threshold doesn't machine-gun arrows).
const ENGAGE: f32 = 0.55;
const RELEASE: f32 = 0.35;
/// Right-stick pointer: deadzone and full-deflection speed in px per 8 ms tick (~1600 px/s).
const PTR_DEADZONE: f32 = 0.15;
const PTR_MAX_PER_TICK: f32 = 13.0;

/// Disarm the uinput bridge after this many *consecutive* emit failures, logging once,
/// instead of warning on every failed tick (~125 Hz spam if the device is revoked mid-session).
const EMIT_ERROR_LIMIT: u32 = 10;

const UP: usize = 0;
const DOWN: usize = 1;
const LEFT: usize = 2;
const RIGHT: usize = 3;
const DIR_KEYS: [Key; 4] = [Key::KEY_UP, Key::KEY_DOWN, Key::KEY_LEFT, Key::KEY_RIGHT];

#[derive(Default, Clone, Copy)]
struct Dir {
    dpad: bool,
    stick: bool,
    /// Next auto-repeat deadline while engaged.
    next: Option<Instant>,
}

impl Dir {
    fn engaged(&self) -> bool {
        self.dpad || self.stick
    }
}

pub struct NavPad {
    dev: VirtualDevice,
    // Activation gate (cached — an X round-trip per input event would be silly).
    cached_active: bool,
    last_check: Instant,
    logged_active: bool, // last active-state we logged, so transitions log once each
    emitted_since_active: bool, // did we deliver anything this activation? (diagnostic)
    emit_errors: u32,           // consecutive emit() failures; resets on any success
    emit_disarmed: bool,        // stop emitting after EMIT_ERROR_LIMIT consecutive failures
    // Navigation state.
    dirs: [Dir; 4],
    ptr_x: f32,
    ptr_y: f32,
    // Fractional pointer remainder so slow deflections still move.
    ptr_rem: (f32, f32),
    wheel: i32, // -1, 0, +1 currently held
    wheel_next: Option<Instant>,
    /// Keys/buttons currently held down through the bridge (released in bulk when the
    /// app goes away mid-press, so nothing stays stuck).
    held: Vec<Key>,
}

impl NavPad {
    /// Build the virtual device; None (with one log line) when /dev/uinput isn't writable —
    /// the rest of the app works fine without the bridge.
    pub fn new() -> Option<NavPad> {
        let mut keys = AttributeSet::<Key>::new();
        for k in [
            Key::KEY_UP, Key::KEY_DOWN, Key::KEY_LEFT, Key::KEY_RIGHT,
            Key::KEY_ENTER, Key::KEY_ESC, Key::KEY_SPACE,
            Key::BTN_LEFT, Key::BTN_RIGHT,
        ] {
            keys.insert(k);
        }
        let mut rel = AttributeSet::<RelativeAxisType>::new();
        rel.insert(RelativeAxisType::REL_X);
        rel.insert(RelativeAxisType::REL_Y);
        rel.insert(RelativeAxisType::REL_WHEEL);
        let dev = VirtualDeviceBuilder::new()
            .and_then(|b| b.name("OmniDeck Navigation Bridge").with_keys(&keys)?.with_relative_axes(&rel)?.build());
        match dev {
            Ok(dev) => {
                tracing::info!("navpad: virtual keyboard/mouse ready (/dev/uinput)");
                Some(NavPad {
                    dev,
                    cached_active: false,
                    last_check: Instant::now() - ACTIVE_CACHE,
                    logged_active: false,
                    emitted_since_active: false,
                    emit_errors: 0,
                    emit_disarmed: false,
                    dirs: Default::default(),
                    ptr_x: 0.0,
                    ptr_y: 0.0,
                    ptr_rem: (0.0, 0.0),
                    wheel: 0,
                    wheel_next: None,
                    held: Vec::new(),
                })
            }
            Err(e) => {
                tracing::warn!(
                    "navpad: /dev/uinput unavailable ({e}) — controller won't drive launched \
                     apps (add your user to the `input` group)"
                );
                None
            }
        }
    }

    /// Is a launched app in front? Same gate policy as the switcher (session-only unless
    /// OMNIDECK_FORCE_HOTKEY), refreshed at most every ACTIVE_CACHE.
    fn active(&mut self) -> bool {
        if !crate::session::in_session() && std::env::var_os("OMNIDECK_FORCE_HOTKEY").is_none() {
            return false;
        }
        if self.last_check.elapsed() >= ACTIVE_CACHE {
            self.cached_active = crate::switcher::any_app_visible();
            self.last_check = Instant::now();
            // Log each transition once so a session log answers "did the bridge engage?"
            // (the couch test couldn't tell whether the gate or the delivery was the issue).
            if self.cached_active != self.logged_active {
                if self.cached_active {
                    tracing::info!("navpad: ACTIVE — launched app in front, pad now drives it");
                    self.emitted_since_active = false;
                } else {
                    tracing::info!(
                        "navpad: inactive — OmniDeck in front (delivered input this activation: {})",
                        self.emitted_since_active
                    );
                }
                self.logged_active = self.cached_active;
            }
        }
        self.cached_active
    }

    fn emit(&mut self, events: &[InputEvent]) {
        if self.emit_disarmed {
            return;
        }
        if let Err(e) = self.dev.emit(events) {
            self.emit_errors += 1;
            if self.emit_errors >= EMIT_ERROR_LIMIT {
                self.emit_disarmed = true;
                tracing::warn!(
                    "navpad: emit failed {} times in a row ({e}) — disarming input bridge",
                    self.emit_errors
                );
            }
            // Individual failures are swallowed on purpose: a revoked uinput device fails
            // ~125x/s and would flood the rotating log. The disarm line is the one signal.
        } else {
            self.emit_errors = 0;
            if !self.emitted_since_active {
                // First delivery of an activation — proves the uinput device is being read.
                tracing::info!("navpad: delivered first input to the focused app");
                self.emitted_since_active = true;
            }
        }
    }

    /// One key pulse (down+up). TV-style web UIs act on keydown; the immediate keyup keeps
    /// autorepeat OURS (deterministic across X/Xwayland clients) and nothing can stick.
    fn pulse(&mut self, key: Key) {
        self.emit(&[
            InputEvent::new(EventType::KEY, key.code(), 1),
            InputEvent::new(EventType::KEY, key.code(), 0),
        ]);
    }

    fn key_down(&mut self, key: Key) {
        self.emit(&[InputEvent::new(EventType::KEY, key.code(), 1)]);
        if !self.held.contains(&key) {
            self.held.push(key);
        }
    }

    fn key_up(&mut self, key: Key) {
        self.emit(&[InputEvent::new(EventType::KEY, key.code(), 0)]);
        self.held.retain(|&k| k != key);
    }

    fn set_dir(&mut self, dir: usize, source_stick: bool, on: bool) {
        let d = &mut self.dirs[dir];
        let was = d.engaged();
        if source_stick {
            d.stick = on;
        } else {
            d.dpad = on;
        }
        let now_engaged = self.dirs[dir].engaged();
        if now_engaged && !was {
            self.pulse(DIR_KEYS[dir]);
            self.dirs[dir].next = Some(Instant::now() + REPEAT_FIRST);
        } else if !now_engaged {
            self.dirs[dir].next = None;
        }
    }

    /// Map a stick axis to its two directions with hysteresis.
    fn stick_axis(&mut self, v: f32, neg_dir: usize, pos_dir: usize) {
        for (dir, dv) in [(neg_dir, -v), (pos_dir, v)] {
            let engaged = self.dirs[dir].stick;
            if !engaged && dv > ENGAGE {
                self.set_dir(dir, true, true);
            } else if engaged && dv < RELEASE {
                self.set_dir(dir, true, false);
            }
        }
    }

    /// Translate one gilrs event. Call AFTER the Guide handling; cheap no-op when the
    /// bridge isn't active.
    pub fn handle(&mut self, event: &gilrs::EventType) {
        use gilrs::{Axis, Button, EventType as G};
        if !self.active() {
            return;
        }
        match *event {
            G::ButtonPressed(b, _) | G::ButtonReleased(b, _) => {
                let down = matches!(*event, G::ButtonPressed(..));
                match b {
                    Button::DPadUp => self.set_dir(UP, false, down),
                    Button::DPadDown => self.set_dir(DOWN, false, down),
                    Button::DPadLeft => self.set_dir(LEFT, false, down),
                    Button::DPadRight => self.set_dir(RIGHT, false, down),
                    // A/South = the primary "select": a LEFT CLICK where the pointer is —
                    // the user navigates with the right-stick pointer and expects cross/A to
                    // click (couch test 2026-07-09). Enter (for keyboard/spatial-nav apps like
                    // Jellyfin-web) is on X/West so the two never double-fire on toggles.
                    Button::South => if down { self.key_down(Key::BTN_LEFT) } else { self.key_up(Key::BTN_LEFT) },
                    Button::East => if down { self.key_down(Key::KEY_ESC) } else { self.key_up(Key::KEY_ESC) },
                    Button::West => if down { self.key_down(Key::KEY_ENTER) } else { self.key_up(Key::KEY_ENTER) },
                    Button::North => if down { self.key_down(Key::KEY_SPACE) } else { self.key_up(Key::KEY_SPACE) },
                    Button::RightTrigger2 => if down { self.key_down(Key::BTN_LEFT) } else { self.key_up(Key::BTN_LEFT) },
                    Button::LeftTrigger2 => if down { self.key_down(Key::BTN_RIGHT) } else { self.key_up(Key::BTN_RIGHT) },
                    Button::RightTrigger => {
                        self.wheel = if down { -1 } else { 0 }; // wheel: negative = scroll down
                        self.wheel_next = down.then(Instant::now);
                    }
                    Button::LeftTrigger => {
                        self.wheel = if down { 1 } else { 0 };
                        self.wheel_next = down.then(Instant::now);
                    }
                    _ => {}
                }
            }
            G::AxisChanged(axis, v, _) => match axis {
                // gilrs: +1 = stick up; screen up = UP direction / negative REL_Y.
                Axis::LeftStickX | Axis::DPadX => self.stick_axis(v, LEFT, RIGHT),
                Axis::LeftStickY | Axis::DPadY => self.stick_axis(-v, UP, DOWN),
                Axis::RightStickX => self.ptr_x = v,
                Axis::RightStickY => self.ptr_y = v,
                _ => {}
            },
            _ => {}
        }
    }

    /// Periodic work: auto-repeat, pointer motion, wheel repeat, and stuck-state cleanup
    /// when the app disappears mid-press. Call every gamepad-loop tick (~8 ms).
    pub fn tick(&mut self) {
        if !self.active() {
            // App went away (closed, hidden, Guide) — release everything we hold so no
            // key arrives stuck when focus lands somewhere else.
            if !self.held.is_empty() || self.dirs.iter().any(|d| d.engaged()) || self.wheel != 0 {
                for key in std::mem::take(&mut self.held) {
                    self.emit(&[InputEvent::new(EventType::KEY, key.code(), 0)]);
                }
                self.dirs = Default::default();
                self.wheel = 0;
                self.wheel_next = None;
                self.ptr_x = 0.0;
                self.ptr_y = 0.0;
            }
            return;
        }
        let now = Instant::now();
        // Direction auto-repeat (which are due is decided first — pulse() needs &mut self).
        let due: Vec<usize> = (0..self.dirs.len())
            .filter(|&d| self.dirs[d].engaged() && self.dirs[d].next.is_some_and(|t| now >= t))
            .collect();
        for dir in due {
            self.pulse(DIR_KEYS[dir]);
            self.dirs[dir].next = Some(now + REPEAT_NEXT);
        }
        // Wheel repeat.
        if self.wheel != 0 && self.wheel_next.is_some_and(|t| now >= t) {
            let w = self.wheel;
            self.emit(&[InputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_WHEEL.0, w)]);
            self.wheel_next = Some(now + WHEEL_REPEAT);
        }
        // Right-stick pointer: squared response for fine control near center.
        let speed = |v: f32| -> f32 {
            if v.abs() < PTR_DEADZONE { 0.0 } else { v.abs() * v.abs() * v.signum() * PTR_MAX_PER_TICK }
        };
        let (dx, dy) = (speed(self.ptr_x), -speed(self.ptr_y)); // gilrs +Y=up, screen +Y=down
        if dx != 0.0 || dy != 0.0 {
            let fx = dx + self.ptr_rem.0;
            let fy = dy + self.ptr_rem.1;
            let (ix, iy) = (fx.trunc() as i32, fy.trunc() as i32);
            self.ptr_rem = (fx.fract(), fy.fract());
            if ix != 0 || iy != 0 {
                self.emit(&[
                    InputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_X.0, ix),
                    InputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_Y.0, iy),
                ]);
            }
        } else {
            self.ptr_rem = (0.0, 0.0);
        }
    }
}
