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
    if crate::capability::probe().nvidia_present {
        let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default().to_ascii_lowercase();
        let in_gamescope = std::env::var_os("GAMESCOPE_WAYLAND_DISPLAY").is_some()
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
