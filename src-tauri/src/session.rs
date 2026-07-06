// OmniDeck — the one place that answers "are we running as the gamescope session?".
//
// Everything session-gated (the STEAM_GAME atom, the app switcher, the hotkey grabs, the
// GDK backend pin, fullscreening) keys off this. It used to be ~10 inlined env checks; a
// second session signal (a future gamescope exporting something else, a nested-session
// marker) now means editing exactly one function.
pub fn in_session() -> bool {
    std::env::var_os("GAMESCOPE_WAYLAND_DISPLAY").is_some()
}
