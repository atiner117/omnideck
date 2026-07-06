// OmniDeck — GPU-appropriate webview environment.
// WebKitGTK needs env set BEFORE it initializes, so this re-execs the process once with the
// right variables rather than setting them in-process.

/// Re-exec self once with GPU-appropriate WebKit env so the webview renders on any GPU. The
/// NVIDIA WebKitGTK workaround is *session-specific* (2026): on X11/gamescope the dmabuf
/// renderer is the bug; on Wayland the bug is a startup crash fixed by disabling explicit sync
/// (no perf cost) — and `GDK_BACKEND=x11` must NOT be forced (it reintroduces the
/// fractional-scaling/blur/input regressions Wayland users left X11 to escape). AMD/Intel
/// (Mesa) need nothing.
#[cfg(unix)]
pub fn ensure_gpu_env() {
    if std::env::var_os("OMNIDECK_ENV_READY").is_some() {
        return;
    }
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.args(std::env::args_os().skip(1));
    cmd.env("OMNIDECK_ENV_READY", "1");
    // Inside gamescope, be an X11 (Xwayland) client EXPLICITLY. Everything session-side —
    // the STEAM_GAME focus-return atom (watchdog), the app switcher's unmap/map, the
    // Ctrl+Alt+Home grabs — manages our window through X. GTK, however, prefers a Wayland
    // socket when it sees one: a parent compositor's leaked WAYLAND_DISPLAY (nested
    // gamescope on a desktop — the test harness) or a gamescope build that exports its own
    // would silently put our window where none of that machinery can reach it. On the real
    // SDDM session this is a no-op today (no parent socket); it pins the behavior we
    // already rely on. The "don't force GDK_BACKEND=x11" warning below is about *desktop
    // Wayland*, where it stays unset. Launched children inherit it, which is also right:
    // the switcher can only hide/show X windows.
    if crate::session::in_session() {
        cmd.env("GDK_BACKEND", "x11");
    }
    if crate::capability::probe().nvidia_present {
        let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default().to_ascii_lowercase();
        let in_gamescope = crate::session::in_session()
            || std::env::var_os("STEAM_GAMESCOPE").is_some();
        if in_gamescope || session == "x11" {
            // X11/gamescope: the dmabuf renderer paints blank on NVIDIA — disable it.
            cmd.env("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        } else if session == "wayland" {
            // Wayland: WebKitGTK won't start on NVIDIA without this (explicit-sync crash); keeps
            // the hardware-accelerated fast path, unlike disabling dmabuf.
            cmd.env("__NV_DISABLE_EXPLICIT_SYNC", "1");
        } else {
            // Unknown session type: take the conservative X11-style workaround.
            cmd.env("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        // Last-resort: WEBKIT_DISABLE_COMPOSITING_MODE forces SOFTWARE paint (caps animation
        // smoothness — the category-switch fps dip). Set OMNIDECK_GPU_COMPOSITING=1 to try GPU
        // compositing instead: smoother *if* driver + WebKitGTK render correctly without it.
        if std::env::var_os("OMNIDECK_GPU_COMPOSITING").is_none() {
            cmd.env("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }
    use std::os::unix::process::CommandExt;
    let _ = cmd.exec(); // replaces this process; returns only on failure
}

#[cfg(not(unix))]
pub fn ensure_gpu_env() {}

/// Log the display mode the session ACTUALLY runs at, via gamescope's Xwayland RandR
/// (gamescope reports its real output mode there). This is the ground truth for "did my
/// session.conf `-r 165` apply?" — the UI's fps meter can't answer it: WebKitGTK's
/// software-compositing frame clock paces requestAnimationFrame at ~60 regardless of the
/// panel (M2 finding: meter read ~61 on a 165 Hz mode; the 100/240 spikes were rAF burst
/// noise hitting the meter's clamp).
pub fn log_session_display_mode() {
    if !crate::session::in_session() {
        return;
    }
    if let Err(e) = try_log_mode() {
        tracing::warn!("display-mode probe failed (cosmetic): {e}");
    }
}

fn try_log_mode() -> Result<(), Box<dyn std::error::Error>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::randr::ConnectionExt as _;
    let (conn, screen_num) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen_num].root;
    let res = conn.randr_get_screen_resources_current(root)?.reply()?;
    let modes: std::collections::HashMap<u32, _> =
        res.modes.iter().map(|m| (m.id, m)).collect();
    for &crtc in &res.crtcs {
        let info = conn.randr_get_crtc_info(crtc, res.config_timestamp)?.reply()?;
        if info.mode == 0 {
            continue; // disabled crtc
        }
        if let Some(m) = modes.get(&info.mode) {
            let denom = u64::from(m.htotal) * u64::from(m.vtotal);
            let hz = if denom > 0 { m.dot_clock as f64 / denom as f64 } else { 0.0 };
            tracing::info!(
                "session display mode: {}x{} @ {hz:.0} Hz (gamescope Xwayland RandR)",
                m.width, m.height
            );
        }
    }
    Ok(())
}
