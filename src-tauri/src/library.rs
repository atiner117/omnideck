// OmniDeck — M4 Steam library scan.
//
// Parses Steam's VDF/ACF files (no Steam running required):
//   <root>/steamapps/libraryfolders.vdf   -> the set of library paths + their appids
//   <lib>/steamapps/appmanifest_<id>.acf   -> per-game name / installdir / StateFlags
// "installed" = StateFlags & 4. Libraries on unmounted paths (e.g. /mnt/r2d2) are
// reported as unavailable and their games are skipped rather than shown as launchable.
//
// Art uses the new per-appid hashed-dir layout:
//   <root>/appcache/librarycache/<appid>/<hash>/library_capsule.jpg  (box / portrait)
//                                               library_hero.jpg     (wide background)
//                                        <appid>/logo.png            (transparent logo)
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize, Debug)]
pub struct Game {
    pub appid: String,
    pub name: String,
    pub installdir: String,
    pub library_path: String,
    pub installed: bool,
    pub is_tool: bool,
    pub last_played: u64,
    pub art_box: Option<String>,
    pub art_header: Option<String>,
    pub art_hero: Option<String>,
    pub art_logo: Option<String>,
}

#[derive(Clone, Serialize, Debug)]
pub struct LibrarySummary {
    pub path: String,
    pub label: String,
    pub available: bool,
    pub app_count: usize,
}

#[derive(Clone, Serialize, Debug, Default)]
pub struct Library {
    pub steam_root: Option<String>,
    pub games: Vec<Game>,
    pub libraries: Vec<LibrarySummary>,
    pub errors: Vec<String>,
}

// ---- VDF deserialization targets (keyvalues-serde discards the root key) ----
#[derive(Deserialize)]
struct LibFolder {
    path: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    apps: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct AppManifest {
    appid: String,
    name: String,
    #[serde(rename = "StateFlags", default)]
    state_flags: String,
    #[serde(default)]
    installdir: String,
    #[serde(rename = "LastPlayed", default)]
    last_played: String,
}

pub fn scan() -> Library {
    let mut lib = Library::default();

    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => {
            lib.errors.push("HOME is not set".into());
            return lib;
        }
    };

    let steam_root = [
        format!("{home}/.steam/steam"),
        format!("{home}/.local/share/Steam"),
        format!("{home}/.steam/root"),
    ]
    .into_iter()
    .find(|p| Path::new(&format!("{p}/steamapps/libraryfolders.vdf")).exists());

    let steam_root = match steam_root {
        Some(r) => r,
        None => {
            lib.errors
                .push("Steam not found (no steamapps/libraryfolders.vdf)".into());
            return lib;
        }
    };
    lib.steam_root = Some(steam_root.clone());

    let lf_path = format!("{steam_root}/steamapps/libraryfolders.vdf");
    let lf_text = match fs::read_to_string(&lf_path) {
        Ok(t) => t,
        Err(e) => {
            lib.errors.push(format!("read {lf_path}: {e}"));
            return lib;
        }
    };
    let folders: BTreeMap<String, LibFolder> = match keyvalues_serde::from_str(&lf_text) {
        Ok(f) => f,
        Err(e) => {
            lib.errors.push(format!("parse libraryfolders.vdf: {e}"));
            return lib;
        }
    };

    let art_root = format!("{steam_root}/appcache/librarycache");

    for folder in folders.values() {
        let steamapps = format!("{}/steamapps", folder.path);
        let available = Path::new(&steamapps).is_dir();
        lib.libraries.push(LibrarySummary {
            path: folder.path.clone(),
            label: folder.label.clone(),
            available,
            app_count: folder.apps.len(),
        });
        if !available {
            lib.errors.push(format!(
                "library '{}' at {} is unavailable (not mounted?) — {} games skipped",
                folder.label,
                folder.path,
                folder.apps.len()
            ));
            continue;
        }
        for appid in folder.apps.keys() {
            let manifest = format!("{steamapps}/appmanifest_{appid}.acf");
            let text = match fs::read_to_string(&manifest) {
                Ok(t) => t,
                Err(e) => {
                    lib.errors.push(format!("read {manifest}: {e}"));
                    continue;
                }
            };
            let m: AppManifest = match keyvalues_serde::from_str(&text) {
                Ok(m) => m,
                Err(e) => {
                    lib.errors.push(format!("parse {manifest}: {e}"));
                    continue;
                }
            };
            let installed = m
                .state_flags
                .parse::<u32>()
                .map(|f| f & 4 != 0)
                .unwrap_or(false);
            let is_tool = is_tool_name(&m.name);
            let last_played = m.last_played.parse::<u64>().unwrap_or(0);
            let (art_box, art_header, art_hero, art_logo) = resolve_art(&art_root, appid);
            lib.games.push(Game {
                appid: m.appid,
                name: m.name,
                installdir: m.installdir,
                library_path: folder.path.clone(),
                installed,
                is_tool,
                last_played,
                art_box,
                art_header,
                art_hero,
                art_logo,
            });
        }
    }

    lib.games.sort_by_key(|g| g.name.to_lowercase());
    lib
}

/// Walk `<art_root>/<appid>` (a few levels deep) and pick out the named art files.
/// Returns (box/capsule, header, hero, logo).
fn resolve_art(
    art_root: &str,
    appid: &str,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let mut files = Vec::new();
    collect_files(Path::new(&format!("{art_root}/{appid}")), &mut files, 0);
    let pick = |name: &str| {
        files
            .iter()
            .find(|p| p.file_name().map(|f| f == name).unwrap_or(false))
            .map(|p| p.to_string_lossy().into_owned())
    };
    (
        pick("library_capsule.jpg"),
        pick("library_header.jpg"),
        pick("library_hero.jpg"),
        pick("logo.png"),
    )
}

/// Steam runtimes / redistributables are listed as "installed" but aren't games.
fn is_tool_name(name: &str) -> bool {
    const PATTERNS: [&str; 5] = [
        "Proton",
        "Steam Linux Runtime",
        "Steamworks Common Redistributables",
        "EasyAntiCheat Runtime",
        "Steam Runtime",
    ];
    PATTERNS.iter().any(|p| name.contains(p))
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>, depth: u8) {
    if depth > 3 {
        return;
    }
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_files(&p, out, depth + 1);
            } else {
                out.push(p);
            }
        }
    }
}

pub fn report(lib: &Library) -> String {
    let mut s = String::from("OmniDeck Steam library scan\n");
    s.push_str(&format!(
        "  steam root:  {}\n",
        lib.steam_root.as_deref().unwrap_or("(not found)")
    ));
    s.push_str("  libraries:\n");
    for l in &lib.libraries {
        s.push_str(&format!(
            "    - [{}] {} ({} apps)  {}\n",
            if l.available { "ok " } else { "N/A" },
            l.path,
            l.app_count,
            if l.label.is_empty() {
                String::new()
            } else {
                format!("\"{}\"", l.label)
            }
        ));
    }
    s.push_str(&format!("  games found: {}\n", lib.games.len()));
    for g in &lib.games {
        let (b, h, l) = (&g.art_box, &g.art_hero, &g.art_logo);
        let art = format!(
            "box:{} hero:{} logo:{}",
            if b.is_some() { "✓" } else { "·" },
            if h.is_some() { "✓" } else { "·" },
            if l.is_some() { "✓" } else { "·" },
        );
        s.push_str(&format!(
            "    - {:>8}  {:<40} {}\n",
            g.appid,
            truncate(&g.name, 40),
            art
        ));
    }
    if !lib.errors.is_empty() {
        s.push_str("  notes:\n");
        for e in &lib.errors {
            s.push_str(&format!("    - {e}\n"));
        }
    }
    s
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(n - 1).collect();
        t.push('…');
        t
    }
}
