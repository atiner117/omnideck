// OmniDeck — auto-generated, display-aware mpv playback profiles (media direct-play).
//
// Renders an embedded profile set (mpv.conf + input.conf + VapourSynth .vpy filters)
// into ~/.config/omnideck/mpv-profiles/ and hands media_play the --include path, so
// interpolation/denoise/tone-mapping work out of the box on any host with mpv built
// against VapourSynth. Policy v1, kept honest to measurements on a 14700K + RTX 3070
// driving 1440p @ 165 Hz:
//   * BASIC interpolation (MVTools BlockFPS) targets the full display rate.
//   * ULTRA (FlowFPS, optical flow) targets display/2 above 100 Hz — full-rate FlowFPS
//     starves the CPU over a long movie and audio walks away from video — AND stays
//     inside a pixel-rate budget of cpu_threads × 12 Mpx/s (src_px × target_fps): a 60 Hz
//     display never trips the halving, but a 4K source targeting 60 measured 13.5 of 16
//     cores on a 7800X3D with easy synthetic motion (2026-07-08 two-host bench) — real
//     film desyncs there. Under-budget targets pass; over-budget ones are lowered, and
//     below 2× the source rate the .vpy passes through instead.
//   * The GPU side (upscale, tone-map, deband, present at display rate) is cheap by
//     comparison — mpv.conf `profile=high-quality` + `vo=gpu-next` own it.
// The display rate itself reaches the .vpy scripts at runtime via mpv's injected
// `display_fps`, made deterministic by the `--display-fps-override` media_play passes
// from the session's RandR mode — the profiles never bake a refresh rate in.
use std::path::{Path, PathBuf};

/// What the profile set is generated FOR — recorded in every rendered header so a
/// support bundle (or the user) can see what OmniDeck detected, and so the set is
/// re-rendered when the hardware story changes.
pub struct Tier {
    pub display: Option<crate::gpu::DisplayMode>,
    pub cpu_threads: usize,
    pub gpu: String,
    pub usable: bool, // a real (non-lavapipe) GPU — software render can't feed gpu-next
    /// Explicit refresh rate (Hz) from `[media_server] display_fps`; 0 = none. Overrides the
    /// baked hint so daily desktop use (no session RandR) still targets the real panel rate.
    pub display_fps_override: f64,
    /// Forced audio samplerate (Hz) from `[media_server] audio_samplerate`; 0 = leave native.
    pub audio_samplerate: u32,
}

/// Build the tier from an already-resolved display mode plus the two config knobs. The caller
/// passes the mode it already probed (media_play needs it for `--display-fps-override` anyway)
/// so playback doesn't open a second X11/RandR connection for the same answer.
pub fn probe_tier(
    display: Option<crate::gpu::DisplayMode>,
    display_fps_override: f64,
    audio_samplerate: u32,
) -> Tier {
    let cap = crate::capability::probe();
    let gpu = cap
        .gpus
        .first()
        .map(|g| format!("{} ({})", g.vendor, g.driver))
        .unwrap_or_else(|| "none".into());
    Tier {
        display,
        cpu_threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
        gpu,
        usable: cap.has_real_gpu,
        display_fps_override,
        audio_samplerate,
    }
}

/// The refresh rate to bake into the profiles: the explicit config override wins, else the
/// detected session mode, else 0 (unknown). Floored at 30 to match the `.vpy` consumers.
fn effective_hz(t: &Tier) -> f64 {
    if t.display_fps_override > 30.0 {
        return t.display_fps_override;
    }
    t.display.map(|(_, _, hz)| hz).filter(|hz| *hz > 30.0).unwrap_or(0.0)
}

fn tier_info(t: &Tier) -> String {
    let disp = match (t.display, effective_hz(t)) {
        // Override present: show it, keep the detected geometry if we have it.
        (Some((w, h, _)), hz) if t.display_fps_override > 30.0 => format!("{w}x{h} @ {hz:.1} Hz (config)"),
        (None, hz) if hz > 30.0 => format!("{hz:.1} Hz (config)"),
        (Some((w, h, hz)), _) => format!("{w}x{h} @ {hz:.1} Hz"),
        (None, _) => "unknown (not in session; profiles use mpv's own display_fps)".into(),
    };
    let audio = if t.audio_samplerate > 0 {
        format!(" | audio {} Hz", t.audio_samplerate)
    } else {
        String::new()
    };
    format!("generated for: display {disp} | cpu threads {} | gpu {}{audio}", t.cpu_threads, t.gpu)
}

/// How long a NEGATIVE VapourSynth probe stays cached before we ask mpv again.
const VS_RECHECK: std::time::Duration = std::time::Duration::from_secs(60);

/// mpv built with the VapourSynth filter? Shells out to `mpv --no-config --vf=help`.
/// `true` is cached for the process lifetime (a built-in filter doesn't vanish), but
/// `false` is only cached for VS_RECHECK: OmniDeck is an always-on launcher, and a
/// forever-negative cache meant installing a VapourSynth-enabled mpv mid-run left
/// interpolation silently off until a restart — possibly days later. The re-probe is
/// gated on the timestamp, so this still never forks mpv on every call.
pub fn vapoursynth_available() -> bool {
    static PROBE: std::sync::Mutex<Option<(bool, std::time::Instant)>> = std::sync::Mutex::new(None);
    let mut cached = crate::sync::lock_or_recover(&PROBE, "media_profiles.PROBE");
    if let Some((available, probed_at)) = *cached {
        if available || probed_at.elapsed() < VS_RECHECK {
            return available;
        }
    }
    let out = std::process::Command::new("mpv")
        .args(["--no-config", "--vf=help"])
        .output();
    let available =
        matches!(out, Ok(o) if String::from_utf8_lossy(&o.stdout).contains("vapoursynth"));
    *cached = Some((available, std::time::Instant::now()));
    available
}

const TEMPLATES: &[(&str, &str)] = &[
    ("mpv.conf", include_str!("../profiles/mpv.conf")),
    ("input.conf", include_str!("../profiles/input.conf")),
    ("omnideck-toggles.lua", include_str!("../profiles/omnideck-toggles.lua")),
    ("interpolate-basic.vpy", include_str!("../profiles/interpolate-basic.vpy")),
    ("interpolate-ultra.vpy", include_str!("../profiles/interpolate-ultra.vpy")),
    ("denoise.vpy", include_str!("../profiles/denoise.vpy")),
];

/// Files starting with this marker belong to OmniDeck and are rewritten freely; a file
/// whose header was removed is user-owned and never touched again. Lua uses `--` comments,
/// so the marker is accepted behind either comment leader.
const GENERATED_HEADER: &str = "# omnideck-generated";

fn is_generated(content: &str) -> bool {
    content.starts_with(GENERATED_HEADER) || content.starts_with("-- # omnideck-generated")
}

fn render(template: &str, dir: &Path, tier: &Tier) -> String {
    // The .vpy scripts can't be handed the panel rate at runtime: mpv's vapoursynth
    // filter injects display_fps from the VO only (0 at init; --display-fps-override is
    // NOT forwarded — measured on mpv 0.40). So the session's RandR rate is baked here,
    // 0.0 when unknown (desktop) — the scripts then use mpv's value or their 60 fallback.
    // Floor at 30, matching the `.vpy` consumers: a rate they'd reject as "unknown" (<=30)
    // must not be baked as if it were real, or mpv paces at it while interpolation targets 60.
    let hint = effective_hz(tier);
    // Ultra's sustainability budget: src_pixels × target_fps the CPU can be trusted to
    // FlowFPS through real film. 12 Mpx/s per thread, anchored to ares' known-good
    // (1080p→82.5 on 28t) and known-desync (1080p→165) real-world data points; 0
    // (unknown thread count) disables the cap in the script.
    let budget_px = tier.cpu_threads as f64 * 12_000_000.0;
    // Optional forced-samplerate directive; empty (no line) when 0, so the DAC's native rate
    // is left bit-perfect for everyone who doesn't opt in.
    let audio_line = if tier.audio_samplerate > 0 {
        format!("audio-samplerate={}", tier.audio_samplerate)
    } else {
        String::new()
    };
    template
        .replace("{{PROFILE_DIR}}", &dir.to_string_lossy())
        .replace("{{TIER_INFO}}", &tier_info(tier))
        .replace("{{DISPLAY_FPS_HINT}}", &format!("{hint:.3}"))
        .replace("{{ULTRA_BUDGET_PX}}", &format!("{budget_px:.0}"))
        .replace("{{AUDIO_SAMPLERATE_LINE}}", &audio_line)
}

/// Render the profile set for this hardware into ~/.config/omnideck/mpv-profiles/,
/// returning the mpv.conf path to `--include`. Idempotent and cheap: files are only
/// written when their rendered content changed, and user-owned files are skipped.
pub fn ensure_profiles(tier: &Tier) -> Option<PathBuf> {
    if !tier.usable {
        tracing::info!("mpv-profiles: no usable GPU — skipping auto-profiles");
        return None;
    }
    let dir = crate::config::config_base()?.join("omnideck/mpv-profiles");
    std::fs::create_dir_all(&dir).ok()?;
    for (name, template) in TEMPLATES {
        let path = dir.join(name);
        let content = render(template, &dir, tier);
        match std::fs::read_to_string(&path) {
            Ok(cur) if !is_generated(&cur) => {
                tracing::debug!("mpv-profiles: {name} is user-owned (header removed), keeping it");
                continue;
            }
            Ok(cur) if cur == content => continue,
            Ok(_) => {}                                    // ours and stale — rewrite below
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {} // missing — (re)create
            Err(e) => {
                // Any other read error (permissions, non-UTF-8 content a user pasted after
                // stripping the header, transient I/O): the file may be user-owned, and the
                // contract is "OmniDeck never touches it". Skip rather than clobber it.
                tracing::warn!("mpv-profiles: can't read {name} ({e}) — not overwriting");
                continue;
            }
        }
        if let Err(e) = std::fs::write(&path, &content) {
            tracing::warn!("mpv-profiles: writing {name} failed: {e}");
            return None;
        }
    }
    Some(dir.join("mpv.conf"))
}

/// The `--include=` path for media_play's auto-profile launch, or None when the host
/// can't run the set (no VapourSynth mpv, no real GPU, no writable config dir). Takes the
/// display mode the caller already resolved, so playback doesn't re-probe RandR here.
pub fn auto_include(
    display: Option<crate::gpu::DisplayMode>,
    display_fps_override: f64,
    audio_samplerate: u32,
) -> Option<PathBuf> {
    if !vapoursynth_available() {
        tracing::info!(
            "mpv-profiles: mpv lacks the vapoursynth filter — bare launch \
             (install a VapourSynth-enabled mpv for interpolation profiles)"
        );
        return None;
    }
    ensure_profiles(&probe_tier(display, display_fps_override, audio_samplerate))
}

/// Human-readable report for the `omnideck mpvprofiles` CLI.
pub fn report() -> String {
    let ms = crate::config::load_or_create().media_server;
    let tier = probe_tier(crate::gpu::session_display_mode(), ms.display_fps, ms.audio_samplerate);
    let mut s = String::from("OmniDeck mpv profile set\n");
    s.push_str(&format!("  vapoursynth mpv: {}\n", vapoursynth_available()));
    s.push_str(&format!("  {}\n", tier_info(&tier)));
    match ensure_profiles(&tier) {
        Some(conf) => {
            s.push_str(&format!("  rendered:        {}\n", conf.parent().unwrap_or(&conf).display()));
            s.push_str("  mpv usage:       mpv --include=");
            s.push_str(&conf.to_string_lossy());
            s.push_str(" <file>\n");
        }
        None => s.push_str("  rendered:        NO (no usable GPU or config dir)\n"),
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tier_165() -> Tier {
        Tier {
            display: Some((2560, 1440, 165.0)),
            cpu_threads: 28,
            gpu: "NVIDIA (nvidia)".into(),
            usable: true,
            display_fps_override: 0.0,
            audio_samplerate: 0,
        }
    }

    #[test]
    fn render_bakes_dir_tier_and_rate_and_keeps_header() {
        let dir = Path::new("/cfg/omnideck/mpv-profiles");
        let tier = tier_165();
        for (name, template) in TEMPLATES {
            let out = render(template, dir, &tier);
            assert!(is_generated(&out), "{name}: generated header missing");
            assert!(!out.contains("{{"), "{name}: unrendered placeholder");
            assert!(out.contains("2560x1440 @ 165.0 Hz"), "{name}: tier info missing");
            // Nothing host-personal may leak from the template source.
            assert!(!out.contains("/home/"), "{name}: hardcoded home path");
        }
        let conf = render(TEMPLATES[0].1, dir, &tier);
        assert!(conf.contains("input-conf=/cfg/omnideck/mpv-profiles/input.conf"));
        // The toggle script owns the filter dimensions (no preset combo-profiles anymore);
        // it must be loaded by the conf and know where the .vpy files live.
        assert!(conf.contains("scripts-append=/cfg/omnideck/mpv-profiles/omnideck-toggles.lua"));
        let lua = render(
            TEMPLATES.iter().find(|(n, _)| *n == "omnideck-toggles.lua").unwrap().1,
            dir,
            &tier,
        );
        assert!(lua.contains("VPY = \"/cfg/omnideck/mpv-profiles\""), "lua: vpy dir not baked");
        assert!(lua.contains("interpolate-ultra.vpy"), "lua: ultra path missing");
        // The session rate is baked into the interpolation scripts (mpv does not forward
        // --display-fps-override into VapourSynth) — and 0.0 when there is no session.
        for vpy in ["interpolate-basic.vpy", "interpolate-ultra.vpy"] {
            let t = TEMPLATES.iter().find(|(n, _)| *n == vpy).unwrap().1;
            assert!(render(t, dir, &tier).contains("float(\"165.000\")"), "{vpy}: hint not baked");
            let no_display = Tier {
                display: None,
                cpu_threads: 8,
                gpu: "x".into(),
                usable: true,
                display_fps_override: 0.0,
                audio_samplerate: 0,
            };
            assert!(render(t, dir, &no_display).contains("float(\"0.000\")"));
        }
    }

    #[test]
    fn ultra_profile_halves_only_above_100hz() {
        // The policy lives in the .vpy (rate resolved at runtime); pin the template
        // text so a rework can't silently drop the sustainability cap.
        let ultra = TEMPLATES
            .iter()
            .find(|(n, _)| *n == "interpolate-ultra.vpy")
            .unwrap()
            .1;
        assert!(ultra.contains("dfps / 2 if dfps > 100 else dfps"));
        // Cap #2 (2026-07-08 two-host bench): the pixel-rate budget and its 2×-source
        // floor — a 60 Hz display never trips the halving, but a 4K source can max the
        // CPU anyway. Baked as threads × 12 Mpx/s (28t → 336M), 0 disables.
        assert!(ultra.contains("w * h * target > budget"), "ultra: pixel budget dropped");
        assert!(ultra.contains("target < 2 * src_fps"), "ultra: 2x-source floor dropped");
        let dir = Path::new("/cfg/omnideck/mpv-profiles");
        assert!(render(ultra, dir, &tier_165()).contains("float(\"336000000\")"));
        let unknown_cpu = Tier {
            display: None,
            cpu_threads: 0,
            gpu: "x".into(),
            usable: true,
            display_fps_override: 0.0,
            audio_samplerate: 0,
        };
        assert!(render(ultra, dir, &unknown_cpu).contains("float(\"0\")"));
        let basic = TEMPLATES
            .iter()
            .find(|(n, _)| *n == "interpolate-basic.vpy")
            .unwrap()
            .1;
        assert!(!basic.contains("/ 2"), "basic must keep the full display target");
    }

    #[test]
    fn tier_info_without_display_says_so() {
        let t = Tier {
            display: None,
            cpu_threads: 8,
            gpu: "AMD (amdgpu)".into(),
            usable: true,
            display_fps_override: 0.0,
            audio_samplerate: 0,
        };
        assert!(tier_info(&t).contains("unknown"));
    }

    #[test]
    fn audio_samplerate_line_present_only_when_set() {
        let dir = Path::new("/cfg/omnideck/mpv-profiles");
        let mut t = tier_165();
        // 0 → no forced-samplerate directive (native rate stays bit-perfect).
        assert!(!render(TEMPLATES[0].1, dir, &t).contains("audio-samplerate"));
        t.audio_samplerate = 96000;
        assert!(render(TEMPLATES[0].1, dir, &t).contains("audio-samplerate=96000"));
    }

    #[test]
    fn display_fps_override_wins_over_detected_and_reaches_the_vpy() {
        let dir = Path::new("/cfg/omnideck/mpv-profiles");
        // Detected panel says 60, but the explicit config override is 165.08 → the .vpy hint
        // must bake the override (this is the daily-desktop / no-session path).
        let mut t = tier_165();
        t.display = Some((2560, 1440, 60.0));
        t.display_fps_override = 165.08;
        let basic = TEMPLATES.iter().find(|(n, _)| *n == "interpolate-basic.vpy").unwrap().1;
        assert!(render(basic, dir, &t).contains("float(\"165.080\")"));
        assert!(tier_info(&t).contains("165.1 Hz (config)"));
        // A sub-30 override is treated as "unknown" and does NOT win over a real detected rate.
        let mut t2 = tier_165(); // detected 165
        t2.display_fps_override = 24.0;
        let basic_out = render(basic, dir, &t2);
        assert!(basic_out.contains("float(\"165.000\")"), "sub-30 override must not clobber the detected rate");
    }
}
