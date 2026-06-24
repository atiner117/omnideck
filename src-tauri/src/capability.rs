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
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    GamescopeSession,
    MediaKiosk,
    PlainWindow,
}

#[derive(Clone, Serialize, Debug)]
pub struct Gpu {
    pub pci: String,
    pub vendor: String,
    pub vendor_id: String,
    pub device_id: String,
    pub driver: String,
    pub class: String,
}

#[derive(Clone, Serialize, Debug)]
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

pub fn probe() -> Capability {
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
        if !gamescope {
            diagnostics.push("Tier-1 capable, but gamescope is not installed.".into());
        }
        if !gamescope_session_plus {
            diagnostics.push(
                "Tier-1 capable, but gamescope-session-plus is missing — install with \
                 `paru -S gamescope-session-git` to enable the session."
                    .into(),
            );
        }
        if gamescope && gamescope_session_plus {
            diagnostics.push("Tier-1 ready: a gamescope session can launch.".into());
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
