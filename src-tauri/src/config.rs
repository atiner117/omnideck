// OmniDeck — user config ("config is king").
// A hand-editable TOML at ~/.config/omnideck/config.toml drives the app/media
// tiles, grid columns, and sort order. Generated with sensible detected defaults
// on first run; never overwritten afterward (a parse error falls back to defaults
// in memory without clobbering the user's file).
use crate::apps;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct Settings {
    pub grid_columns: usize,
    pub sort: String, // "alpha" | "recent"
    pub show_runtimes: bool,
    pub accent: String, // hex, e.g. "#4cc2ff"
    pub steamgriddb_key: String, // optional; fills in missing box art
    pub onboarded: bool, // false -> show the first-run wizard
    pub ui_scale: String, // legacy; size is now the smooth ui_scale_custom multiplier
    pub ui_scale_custom: f64, // UI size multiplier
    pub bg_blur: f64, // background cover-art blur in px (0 = sharp)
    pub bg_brightness: f64, // background cover-art brightness (0.3–1.0)
    pub search_provider: String, // web-search URL prefix for the global search (e.g. a SearXNG instance)
    pub search_mode: String, // "duckduckgo" | "google" | "brave" | "bing" | "searxng" | "custom"
    pub sound: bool, // subtle navigation/confirm sounds
    pub sound_volume: f64, // navigation sound volume multiplier (0.0–1.0)
    pub dashboard_recents: usize, // recently-played items shown on Home (0 = off)
    pub recents_show: String, // "games" | "apps" | "both"
    pub background_default: String, // base background: "color" | "image"
    pub background_color: String, // hex used when background_default = "color"
    pub background_image: String, // file path used when background_default = "image"
    pub game_backgrounds: bool, // overlay the focused game's cover art
    pub app_backgrounds: bool, // overlay a color wash from the focused app's icon
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            grid_columns: 6,
            sort: "alpha".into(),
            show_runtimes: false,
            accent: "#4cc2ff".into(),
            steamgriddb_key: String::new(),
            // Default TRUE so an existing config (missing this field) does NOT re-trigger
            // the wizard; a freshly generated config overrides this to false (see defaults()).
            onboarded: true,
            ui_scale: "medium".into(),
            ui_scale_custom: 1.6,
            bg_blur: 0.0,
            bg_brightness: 0.82,
            search_provider: "https://duckduckgo.com/?q=".into(),
            search_mode: "duckduckgo".into(),
            sound: true,
            sound_volume: 0.5,
            dashboard_recents: 8,
            recents_show: "both".into(),
            background_default: "color".into(),
            background_color: "#05070b".into(),
            background_image: String::new(),
            game_backgrounds: true,
            app_backgrounds: true,
        }
    }
}

/// True for a `#rrggbb` hex color — the only form the UI emits and CSS needs.
fn is_hex6(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 7 && b[0] == b'#' && b[1..].iter().all(u8::is_ascii_hexdigit)
}

impl Settings {
    /// Sanitize the hand-editable fields on load so a bad value in config.toml can't break the
    /// UI or inject anything. Numerics are clamped; colors/URLs/enums that don't match are reset
    /// to a safe default (the colors flow straight into CSS on `<main>`; `search_provider` into a
    /// browser launch — also defended at the use-site, this is belt-and-suspenders).
    fn normalize(&mut self) {
        self.ui_scale_custom = self.ui_scale_custom.clamp(0.8, 3.5);
        self.bg_blur = self.bg_blur.clamp(0.0, 24.0);
        self.bg_brightness = self.bg_brightness.clamp(0.3, 1.0);
        self.sound_volume = self.sound_volume.clamp(0.0, 1.0);
        self.grid_columns = self.grid_columns.clamp(1, 12);
        self.dashboard_recents = self.dashboard_recents.min(50);

        if !is_hex6(&self.accent) {
            self.accent = "#4cc2ff".into();
        }
        if !is_hex6(&self.background_color) {
            self.background_color = "#05070b".into();
        }
        // A non-empty provider must be an http(s) URL; clear anything else (empty = "not set",
        // and the UI/launch path falls back to DuckDuckGo).
        if !self.search_provider.is_empty()
            && !self.search_provider.starts_with("https://")
            && !self.search_provider.starts_with("http://")
        {
            self.search_provider.clear();
        }
        if !matches!(self.sort.as_str(), "alpha" | "recent") {
            self.sort = "alpha".into();
        }
        if !matches!(
            self.search_mode.as_str(),
            "duckduckgo" | "google" | "brave" | "bing" | "searxng" | "custom"
        ) {
            self.search_mode = "duckduckgo".into();
        }
        if !matches!(self.recents_show.as_str(), "both" | "games" | "apps") {
            self.recents_show = "both".into();
        }
        if !matches!(self.background_default.as_str(), "color" | "image") {
            self.background_default = "color".into();
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct Config {
    pub settings: Settings,
    pub apps: Vec<apps::App>,
    /// Favorited tile ids (shown on the Home category).
    pub favorites: Vec<String>,
    /// Recently-launched app ids, most-recent-first (Home "recent apps").
    #[serde(default)]
    pub recent_apps: Vec<String>,
    /// Echoed back so the UI can show the user where to edit. Sent over IPC (non-empty)
    /// but never written into the TOML file (empty when serializing the on-disk default).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub config_path: String,
    /// Set when config.toml exists but couldn't be parsed/read, so the UI can warn the user
    /// ("syntax error — using defaults") instead of silently reverting. Never written to disk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional = nullable))] // absent over IPC when None
    pub config_error: Option<String>,
}

fn config_path() -> Option<PathBuf> {
    // XDG: prefer $XDG_CONFIG_HOME (when absolute), else ~/.config (unchanged for existing installs).
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("omnideck/config.toml"))
}

fn defaults() -> Config {
    Config {
        settings: Settings {
            onboarded: false, // a fresh install runs the onboarding wizard
            ..Default::default()
        },
        apps: apps::list(),
        favorites: Vec::new(),
        recent_apps: Vec::new(),
        config_path: String::new(),
        config_error: None,
    }
}

/// Defaults for when an existing config.toml can't be parsed/read: same as `defaults()`, but the
/// user clearly already had a config, so keep them out of the onboarding wizard and flag the error.
fn error_defaults(msg: String) -> Config {
    let mut c = defaults();
    c.settings.onboarded = true;
    c.config_error = Some(msg);
    c
}

pub fn load_or_create() -> Config {
    let path = match config_path() {
        Some(p) => p,
        None => return defaults(),
    };
    let path_str = path.to_string_lossy().into_owned();

    if path.exists() {
        let mut cfg = match fs::read_to_string(&path) {
            Ok(text) => match toml::from_str::<Config>(&text) {
                Ok(c) => c,
                Err(e) => {
                    // Don't clobber the user's file; report the error so the UI can warn instead
                    // of looking like it ignored their edit. Full detail to the log, first line
                    // (TOML's "parse error at line N, column M") to the toast.
                    eprintln!("[omnideck] config.toml parse error — using defaults:\n{e}");
                    let first = e.to_string().lines().next().unwrap_or("parse error").to_string();
                    error_defaults(format!("config.toml: {first} — using defaults until fixed"))
                }
            },
            Err(e) => {
                eprintln!("[omnideck] could not read config.toml: {e} — using defaults");
                error_defaults(format!("Couldn't read config.toml ({e}) — using defaults"))
            }
        };
        cfg.settings.normalize(); // defend against out-of-range values in a hand-edited config
        cfg.config_path = path_str;
        return cfg;
    }

    // First run: write a default config the user can edit.
    let mut cfg = defaults();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(text) = toml::to_string_pretty(&cfg) {
        let _ = fs::write(&path, text);
    }
    cfg.config_path = path_str;
    cfg
}

/// Shared save path: reload the on-disk config, apply `mutate`, write it back. Refuses to
/// write while the file is in the parse/read-error state — otherwise any save (recent apps
/// fire automatically on every launch) would replace the user's config with the in-memory
/// defaults, exactly the clobber the load path promises not to do. Internal IPC-only fields
/// (`config_path`, `config_error`) are stripped before serializing.
fn mutate_and_save(mutate: impl FnOnce(&mut Config)) -> Result<(), String> {
    let path = config_path().ok_or("no config path")?;
    let mut cfg = load_or_create();
    if let Some(err) = cfg.config_error.take() {
        return Err(format!("not saving over a broken config.toml — {err}"));
    }
    mutate(&mut cfg);
    cfg.config_path = String::new(); // never written to disk
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let text = toml::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| e.to_string())
}

/// Persist new settings, preserving the apps list and not writing internal fields.
pub fn save_settings(settings: Settings) -> Result<(), String> {
    mutate_and_save(|cfg| cfg.settings = settings)
}

/// Persist a new apps list (used by the in-app "Add apps" catalog screen).
pub fn save_apps(new_apps: Vec<apps::App>) -> Result<(), String> {
    mutate_and_save(|cfg| cfg.apps = new_apps)
}

/// Persist the favorites list (used by the Home ⭐ toggle).
pub fn save_favorites(favorites: Vec<String>) -> Result<(), String> {
    mutate_and_save(|cfg| cfg.favorites = favorites)
}

/// Persist the recently-launched app ids (Home "recent apps").
pub fn save_recent_apps(recent_apps: Vec<String>) -> Result<(), String> {
    mutate_and_save(|cfg| cfg.recent_apps = recent_apps)
}

pub fn report(cfg: &Config) -> String {
    let mut s = String::from("OmniDeck config\n");
    s.push_str(&format!("  path:    {}\n", cfg.config_path));
    s.push_str(&format!("  columns: {}\n", cfg.settings.grid_columns));
    s.push_str(&format!("  sort:    {}\n", cfg.settings.sort));
    s.push_str(&format!("  runtimes:{}\n", cfg.settings.show_runtimes));
    s.push_str(&format!("  apps:    {}\n", cfg.apps.len()));
    for a in &cfg.apps {
        s.push_str(&format!("    - {} {}  ({})\n", a.icon, a.name, a.exec.join(" ")));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test]
    fn normalize_clamps_out_of_range() {
        let mut s = Settings {
            ui_scale_custom: 99.0,
            bg_brightness: -1.0,
            sound_volume: 5.0,
            grid_columns: 0,
            dashboard_recents: 999,
            ..Default::default()
        };
        s.normalize();
        assert_eq!(s.ui_scale_custom, 3.5);
        assert_eq!(s.bg_brightness, 0.3);
        assert_eq!(s.sound_volume, 1.0);
        assert_eq!(s.grid_columns, 1);
        assert_eq!(s.dashboard_recents, 50);
    }

    #[test]
    fn normalize_sanitizes_bad_strings() {
        let mut s = Settings {
            accent: "red; background:url(http://evil)".into(), // CSS-injection attempt
            background_color: "#zzz".into(),
            search_provider: "javascript:alert(1)".into(), // non-http scheme
            sort: "bogus".into(),
            search_mode: "hax".into(),
            recents_show: "nope".into(),
            background_default: "weird".into(),
            ..Default::default()
        };
        s.normalize();
        assert_eq!(s.accent, "#4cc2ff");
        assert_eq!(s.background_color, "#05070b");
        assert_eq!(s.search_provider, ""); // cleared -> UI falls back to DuckDuckGo
        assert_eq!(s.sort, "alpha");
        assert_eq!(s.search_mode, "duckduckgo");
        assert_eq!(s.recents_show, "both");
        assert_eq!(s.background_default, "color");
    }

    #[test]
    fn normalize_keeps_valid_strings() {
        let mut s = Settings {
            accent: "#AABBCC".into(),
            background_color: "#000000".into(),
            search_provider: "https://searx.example/search?q=".into(),
            sort: "recent".into(),
            search_mode: "searxng".into(),
            ..Default::default()
        };
        s.normalize();
        assert_eq!(s.accent, "#AABBCC");
        assert_eq!(s.background_color, "#000000");
        assert_eq!(s.search_provider, "https://searx.example/search?q=");
        assert_eq!(s.sort, "recent");
        assert_eq!(s.search_mode, "searxng");
    }
}
