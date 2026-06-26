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

#[derive(Clone, Serialize, Deserialize, Default)]
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
    }
}

pub fn load_or_create() -> Config {
    let path = match config_path() {
        Some(p) => p,
        None => return defaults(),
    };
    let path_str = path.to_string_lossy().into_owned();

    if path.exists() {
        let mut cfg = match fs::read_to_string(&path) {
            Ok(text) => toml::from_str::<Config>(&text).unwrap_or_else(|_| defaults()),
            Err(_) => defaults(),
        };
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

/// Persist new settings, preserving the apps list and not writing internal fields.
pub fn save_settings(settings: Settings) -> Result<(), String> {
    let path = config_path().ok_or("no config path")?;
    let mut cfg = load_or_create();
    cfg.settings = settings;
    cfg.config_path = String::new(); // never written to disk
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let text = toml::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| e.to_string())
}

/// Persist a new apps list (used by the in-app "Add apps" catalog screen).
pub fn save_apps(new_apps: Vec<apps::App>) -> Result<(), String> {
    let path = config_path().ok_or("no config path")?;
    let mut cfg = load_or_create();
    cfg.apps = new_apps;
    cfg.config_path = String::new();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let text = toml::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| e.to_string())
}

/// Persist the favorites list (used by the Home ⭐ toggle).
pub fn save_favorites(favorites: Vec<String>) -> Result<(), String> {
    let path = config_path().ok_or("no config path")?;
    let mut cfg = load_or_create();
    cfg.favorites = favorites;
    cfg.config_path = String::new();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let text = toml::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| e.to_string())
}

/// Persist the recently-launched app ids (Home "recent apps").
pub fn save_recent_apps(recent_apps: Vec<String>) -> Result<(), String> {
    let path = config_path().ok_or("no config path")?;
    let mut cfg = load_or_create();
    cfg.recent_apps = recent_apps;
    cfg.config_path = String::new();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let text = toml::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| e.to_string())
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
