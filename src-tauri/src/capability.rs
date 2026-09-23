// OmniDeck — M1 capability probe.
//
// Decides which tier a host can run (see omnideck-build-plan.md §1):
//   Tier 1  gamescope session  — real GPU + active KMS  (gamescope hard-requires hw Vulkan)
//   Tier 2  cage media kiosk    — no usable GPU but `cage` available (software render)
//   Tier 3  plain window        — universal zero-dependency fallback
//
// Detection is filesystem-only (no Vulkan loader / vulkaninfo dependency for v1):
//   * GPUs        -> /sys/bus/pci/devices/*  (PCI class 0x03xxxx = display controller)
//   * render node -> glob /dev/dri/renderD*  (NEVER assume renderD128 / card0)
//   * KMS active  -> presence of /sys/class/drm/card*-<connector> nodes
//                    (authoritative: this is what /proc/cmdline-only checks miss on modern
//                     NVIDIA drivers that default nvidia-drm.modeset on)
//   * ICDs        -> /usr/share/vulkan/icd.d (reject a lavapipe-only host)
// Hardening note: enumerating Vulkan physical devices via `ash` (to reject
// VK_PHYSICAL_DEVICE_TYPE_CPU and prefer DISCRETE) is the planned upgrade; the
// PCI-class + render-node + non-lavapipe-ICD heuristic is the v1 stand-in.
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Serialize, Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    GamescopeSession,
    MediaKiosk,
    PlainWindow,
}

#[derive(Clone, Serialize, Debug)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Gpu {
    pub pci: String,
    pub vendor: String,
    pub vendor_id: String,
    pub device_id: String,
    pub driver: String,
    pub class: String,
}

#[derive(Clone, Serialize, Debug)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Capability {
    pub tier: Tier,
    pub gpus: Vec<Gpu>,
    pub render_nodes: Vec<String>,
    pub drm_cards: Vec<String>,
    pub kms_connectors: Vec<String>,
    pub kms_active: bool,
    pub vulkan_icds: Vec<String>,
    pub has_real_gpu: bool,
    pub nvidia_present: bool,
    pub nvidia_modeset_loaded: bool,
    pub gamescope: bool,
    pub gamescope_session_plus: bool,
    pub cage: bool,
    pub diagnostics: Vec<String>,
}

/// Cached, process-lifetime hardware probe. The result is stable for a run (hardware doesn't
/// change mid-session), and this sits on media_play's and the child-launch env setup's hot
/// paths — so the sysfs/PATH/ICD scans below run once and every later call is a clone.
///
/// Only SETTLED answers are cached: a PCI GPU with no usable render node/ICD usually means
/// the probe ran before udev/driver setup finished (session autostart) — memoizing that
/// would pin "no usable GPU" (auto-profiles off, wrong env decisions) for the whole run.
/// Re-scanning until it settles is a few sysfs dir reads; a genuinely GPU-less host still
/// caches immediately. BOUNDED, though: a GPU still unusable after several probes is a fact
/// of the host (installed card, no usable driver), not boot lag — accept and cache it
/// rather than re-walking sysfs/PATH/ICDs on every play for the whole run.
pub fn probe() -> Capability {
    static CACHE: std::sync::OnceLock<Capability> = std::sync::OnceLock::new();
    static UNSETTLED_PROBES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    if let Some(c) = CACHE.get() {
        return c.clone();
    }
    let cap = probe_uncached();
    let settled = cap.has_real_gpu || cap.gpus.is_empty();
    if settled || UNSETTLED_PROBES.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= 8 {
        let _ = CACHE.set(cap.clone());
    }
    cap
}

fn probe_uncached() -> Capability {
    let (render_nodes, drm_cards, kms_connectors) = scan_dri();
    let kms_active = !kms_connectors.is_empty();
    let gpus = scan_pci_gpus();
    let vulkan_icds = scan_icds();
    let has_non_lavapipe_icd = vulkan_icds
        .iter()
        .any(|f| !f.contains("lvp") && !f.contains("lavapipe"));
    let has_real_gpu = !gpus.is_empty() && !render_nodes.is_empty() && has_non_lavapipe_icd;
    let nvidia_present = gpus.iter().any(|g| g.vendor_id == "0x10de");
    let nvidia_modeset_loaded = Path::new("/sys/module/nvidia_modeset").exists();
    let gamescope = in_path("gamescope");
    let gamescope_session_plus = in_path("gamescope-session-plus") || in_path("gamescope-fg");
    let cage = in_path("cage");
    let webview_audio = gst_autoaudiosink_present();

    let tier1_capable = has_real_gpu && kms_active;
    let tier = if tier1_capable {
        Tier::GamescopeSession
    } else if cage {
        Tier::MediaKiosk
    } else {
        Tier::PlainWindow
    };

    let mut diagnostics = Vec::new();
    if tier1_capable {
        // The session uses *plain* gamescope (install-session.sh runs `gamescope -f …`), so
        // readiness keys on `gamescope` — NOT gamescope-session-plus, which OmniDeck never invokes.
        if gamescope {
            diagnostics.push(
                "Tier-1 ready: run packaging/install-session.sh, then pick \"OmniDeck\" at your \
                 display manager (plain gamescope — gamescope-session-plus is not required)."
                    .into(),
            );
        } else {
            diagnostics.push(
                "Tier-1 capable, but gamescope is not installed — install `gamescope`, then run \
                 packaging/install-session.sh to add the session."
                    .into(),
            );
        }
    } else if has_real_gpu && !kms_active {
        diagnostics.push(
            "GPU present but KMS/modeset is inactive — enable `nvidia-drm.modeset=1` \
             for a gamescope DRM session."
                .into(),
        );
    } else if !has_real_gpu {
        if cage {
            diagnostics
                .push("No usable GPU — media-kiosk (cage, software render) is available.".into());
        } else {
            diagnostics.push(
                "No usable GPU and cage not installed — only plain-window media mode; \
                 install `cage` for a kiosk."
                    .into(),
            );
        }
    }

    // Sound is a webview feature: WebKitGTK plays Web Audio (nav blips, ambient pad) through
    // GStreamer's `autoaudiosink`, which ships in gst-plugins-good — NOT a webkit2gtk
    // dependency on Arch. Couch box 2026-09-23: everything else worked and the menu was just
    // silent; no error anywhere, the WebProcess simply never opened an audio stream.
    if webview_audio == Some(false) {
        diagnostics.push(
            "WebKitGTK has no GStreamer audio sink (autoaudiosink missing) — navigation \
             sounds and ambient music will be silent. Install `gst-plugins-good`."
                .into(),
        );
    }

    Capability {
        tier,
        gpus,
        render_nodes,
        drm_cards,
        kms_connectors,
        kms_active,
        vulkan_icds,
        has_real_gpu,
        nvidia_present,
        nvidia_modeset_loaded,
        gamescope,
        gamescope_session_plus,
        cage,
        diagnostics,
    }
}

fn scan_dri() -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut render = Vec::new();
    let mut cards = Vec::new();
    let mut connectors = Vec::new();
    if let Ok(rd) = fs::read_dir("/dev/dri") {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with("renderD") {
                render.push(format!("/dev/dri/{n}"));
            } else if n.starts_with("card") {
                cards.push(format!("/dev/dri/{n}"));
            }
        }
    }
    if let Ok(rd) = fs::read_dir("/sys/class/drm") {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            // connector nodes look like card1-DP-1, card0-HDMI-A-1 (card<N>-<conn>)
            if n.starts_with("card") && n.contains('-') {
                connectors.push(n);
            }
        }
    }
    render.sort();
    cards.sort();
    connectors.sort();
    (render, cards, connectors)
}

fn scan_pci_gpus() -> Vec<Gpu> {
    let mut gpus = Vec::new();
    if let Ok(rd) = fs::read_dir("/sys/bus/pci/devices") {
        for e in rd.flatten() {
            let p = e.path();
            let class = read_trim(p.join("class")).unwrap_or_default();
            if !class.starts_with("0x03") {
                continue; // not a display controller
            }
            let vendor_id = read_trim(p.join("vendor")).unwrap_or_default();
            let device_id = read_trim(p.join("device")).unwrap_or_default();
            let driver = fs::read_link(p.join("driver"))
                .ok()
                .and_then(|d| d.file_name().map(|f| f.to_string_lossy().into_owned()))
                .unwrap_or_else(|| "none".into());
            let vendor = match vendor_id.as_str() {
                "0x10de" => "NVIDIA",
                "0x1002" => "AMD",
                "0x8086" => "Intel",
                _ => "unknown",
            }
            .to_string();
            gpus.push(Gpu {
                pci: e.file_name().to_string_lossy().into_owned(),
                vendor,
                vendor_id,
                device_id,
                driver,
                class,
            });
        }
    }
    gpus.sort_by(|a, b| a.pci.cmp(&b.pci));
    gpus
}

fn scan_icds() -> Vec<String> {
    let mut out = Vec::new();
    for dir in ["/usr/share/vulkan/icd.d", "/etc/vulkan/icd.d"] {
        if let Ok(rd) = fs::read_dir(dir) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().into_owned();
                if n.ends_with(".json") {
                    out.push(n);
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn read_trim<P: AsRef<Path>>(p: P) -> Option<String> {
    fs::read_to_string(p).ok().map(|s| s.trim().to_string())
}

/// Is GStreamer's `autoaudiosink` (the element WebKitGTK builds its audio output on)
/// installed? `None` when no GStreamer plugin directory exists at all — a layout this probe
/// doesn't know (Flatpak, AppImage) rather than evidence of a missing plugin.
fn gst_autoaudiosink_present() -> Option<bool> {
    let mut dirs: Vec<std::path::PathBuf> = std::env::var_os("GST_PLUGIN_PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    dirs.extend(
        [
            "/usr/lib/gstreamer-1.0",
            "/usr/lib64/gstreamer-1.0",
            "/usr/lib/x86_64-linux-gnu/gstreamer-1.0",
            "/usr/lib/aarch64-linux-gnu/gstreamer-1.0",
            "/usr/local/lib/gstreamer-1.0",
        ]
        .map(std::path::PathBuf::from),
    );
    gst_autoaudiosink_present_in(&dirs)
}

fn gst_autoaudiosink_present_in(dirs: &[std::path::PathBuf]) -> Option<bool> {
    let existing: Vec<&std::path::PathBuf> = dirs.iter().filter(|d| d.is_dir()).collect();
    if existing.is_empty() {
        return None;
    }
    // libgstautodetect.so is the plugin that registers autoaudiosink (gst-plugins-good).
    Some(existing.iter().any(|d| d.join("libgstautodetect.so").is_file()))
}

fn in_path(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(bin).is_file()))
        .unwrap_or(false)
}

pub fn report(c: &Capability) -> String {
    let mut s = String::from("OmniDeck capability probe\n");
    s.push_str(&format!("  tier:            {:?}\n", c.tier));
    s.push_str(&format!("  has_real_gpu:    {}\n", c.has_real_gpu));
    s.push_str(&format!(
        "  kms_active:      {} ({} connectors)\n",
        c.kms_active,
        c.kms_connectors.len()
    ));
    s.push_str(&format!("  render nodes:    {:?}\n", c.render_nodes));
    s.push_str(&format!("  drm cards:       {:?}\n", c.drm_cards));
    s.push_str(&format!("  vulkan ICDs:     {:?}\n", c.vulkan_icds));
    s.push_str(&format!(
        "  nvidia:          present={} modeset_module={}\n",
        c.nvidia_present, c.nvidia_modeset_loaded
    ));
    s.push_str(&format!("  gamescope:       {}\n", c.gamescope));
    s.push_str(&format!("  session-plus:    {}\n", c.gamescope_session_plus));
    s.push_str(&format!("  cage:            {}\n", c.cage));
    for g in &c.gpus {
        s.push_str(&format!(
            "  gpu:             {} {} [{}:{}] driver={} class={}\n",
            g.pci, g.vendor, g.vendor_id, g.device_id, g.driver, g.class
        ));
    }
    if !c.diagnostics.is_empty() {
        s.push_str("  diagnostics:\n");
        for d in &c.diagnostics {
            s.push_str(&format!("    - {d}\n"));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::gst_autoaudiosink_present_in;
    use std::path::PathBuf;

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("omnideck-gst-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn no_plugin_dir_at_all_is_unknown_not_missing() {
        let ghost = std::env::temp_dir().join("omnideck-gst-no-such-dir");
        assert_eq!(gst_autoaudiosink_present_in(&[ghost]), None);
    }

    #[test]
    fn plugin_dir_without_autodetect_is_missing() {
        let d = scratch("empty");
        std::fs::write(d.join("libgstpipewire.so"), b"").unwrap(); // a sink, but not autodetect
        assert_eq!(gst_autoaudiosink_present_in(std::slice::from_ref(&d)), Some(false));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn autodetect_in_any_listed_dir_is_present() {
        let empty = scratch("first");
        let good = scratch("second");
        std::fs::write(good.join("libgstautodetect.so"), b"").unwrap();
        assert_eq!(gst_autoaudiosink_present_in(&[empty.clone(), good.clone()]), Some(true));
        let _ = std::fs::remove_dir_all(empty);
        let _ = std::fs::remove_dir_all(good);
    }
}
