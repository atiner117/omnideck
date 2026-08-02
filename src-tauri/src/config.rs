// OmniDeck — user config ("config is king").
// A hand-editable TOML at ~/.config/omnideck/config.toml drives the app/media
// tiles, grid columns, and sort order. Generated with sensible detected defaults
// on first run; never overwritten afterward (a parse error falls back to defaults
// in memory without clobbering the user's file).
use crate::apps;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

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
    pub live_wallpaper: String, // animated background: "off" | "waves" (PSP-style ribbon)
    pub ambient: bool, // synthesized ambient background music (subtle, off by default)
    pub ambient_volume: f64, // ambient music volume multiplier (0.0–1.0)
    // Parental controls (pin.rs). Deterrence, not access control — see pin.rs header.
    pub pin_hash: String, // argon2 PHC hash of the parental PIN; empty = no lock. Only set_pin writes it; masked over IPC (see Config::has_pin).
    pub locked_categories: Vec<String>, // category ids the UI gates behind the PIN. Only set_locked_categories writes it (PIN-verified).
    pub check_updates: bool, // boot-time release check (update.rs); manual check always works
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
            live_wallpaper: "waves".into(),
            ambient: false,
            ambient_volume: 0.35,
            pin_hash: String::new(),
            locked_categories: Vec::new(),
            check_updates: true,
        }
    }
}

/// `[screensaver]` — OLED burn-in protection (roadmap Appendix C #1). The gamepad thread
/// (gamepad.rs) emits a single `idle` event after `idle_dim_secs` without pad input and
/// `active` on the next input; the frontend owns the staged presentation (dim at
/// `idle_dim_secs`, Ken-Burns pan at `ken_burns_secs`, true black at `blank_secs`) and
/// honors `enabled` — the backend always emits the transitions.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct ScreensaverConfig {
    pub enabled: bool,       // default true (frontend overlay gate)
    pub idle_dim_secs: u64,  // stage a: dim the whole UI
    pub ken_burns_secs: u64, // stage b: slow pan/zoom on cover art, hide the rails/clock
    pub blank_secs: u64,     // stage c: true black + small moving clock
}

/// Manual impl (not derived): `enabled` must default ON — the derive would pick `false`,
/// silently disabling burn-in protection for every config that doesn't mention it.
impl Default for ScreensaverConfig {
    fn default() -> Self {
        Self { enabled: true, idle_dim_secs: 60, ken_burns_secs: 180, blank_secs: 600 }
    }
}

impl ScreensaverConfig {
    /// Same posture as Settings::normalize — clamp hand-edited numerics into sane ranges
    /// and keep the stages monotonic (dim <= ken-burns <= blank) so the frontend never
    /// sees a later stage scheduled before an earlier one.
    pub fn normalize(&mut self) {
        const MAX_SECS: u64 = 86_400; // a day — effectively "never" without overflow risk
        self.idle_dim_secs = self.idle_dim_secs.clamp(5, MAX_SECS);
        self.ken_burns_secs = self.ken_burns_secs.clamp(self.idle_dim_secs, MAX_SECS);
        self.blank_secs = self.blank_secs.clamp(self.ken_burns_secs, MAX_SECS);
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
        if !matches!(self.live_wallpaper.as_str(), "off" | "waves") {
            self.live_wallpaper = "waves".into();
        }
        self.ambient_volume = self.ambient_volume.clamp(0.0, 1.0);
    }
}

/// `[launch_overrides.<tile-id>]` — per-tile launch tuning (roadmap parking-lot item:
/// "per-game launch options"), hand-editable ("config is king"):
///
/// ```toml
/// [launch_overrides."app:retroarch"]
/// env = { MANGOHUD = "1" }
/// args = ["--verbose"]
/// ```
///
/// `env` is applied to the spawned process, `args` are appended to the tile's argv —
/// custom launchers and catalog apps only (launch_command). Steam titles don't go through
/// our spawn (`steam://rungameid` hands off to the running client), so per-game overrides
/// for them belong in Steam's own Launch Options. Threat model: a hand-edited config can
/// already put arbitrary argv in a custom launcher's `exec`, so env/args add no capability
/// it didn't have; browser (BROWSER-token) tiles still pass the same URL-only argv guard,
/// override args included.
#[derive(Clone, Serialize, Deserialize, Default)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct LaunchOverride {
    /// Extra environment for the spawned process (e.g. MANGOHUD=1, WINEESYNC=1).
    pub env: std::collections::BTreeMap<String, String>,
    /// Extra argv appended after the tile's own exec args.
    pub args: Vec<String>,
}

impl LaunchOverride {
    /// Drop entries `Command::env` can't represent (empty key, '=' or NUL in the key,
    /// NUL in the value) instead of failing the whole launch at spawn time.
    fn normalize(&mut self) {
        self.env.retain(|k, v| !k.is_empty() && !k.contains(['=', '\0']) && !v.contains('\0'));
        self.args.retain(|a| !a.contains('\0'));
    }
}

/// `[input]` — controller/hotkey tuning (parking-lot item, NOTES-DEEPDIVE-ROADMAP.md #6).
/// Read once at startup by the gamepad and hotkey threads — changes need a relaunch (both
/// threads park in blocking waits; re-reading per event would be config I/O at 125 Hz).
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct InputConfig {
    /// Guide-button long-hold threshold in ms: hold this long to close the current app
    /// (short press switches). Clamped 200–5000 — below 200 every tap "closes", above 5000
    /// the hold reads as broken.
    pub guide_hold_ms: u64,
    /// Grab Ctrl+Alt+Home/End inside the gamescope session (keyboard escape hatch —
    /// hotkey.rs). False = no global grabs, for users whose media keyboard needs the chord.
    pub session_hotkeys: bool,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self { guide_hold_ms: 800, session_hotkeys: true }
    }
}

impl InputConfig {
    fn normalize(&mut self) {
        self.guide_hold_ms = self.guide_hold_ms.clamp(200, 5000);
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct Config {
    pub settings: Settings,
    /// `[launch_overrides]` — per-tile env/args tuning, keyed by tile id. Absent from the
    /// generated default config (empty map serializes to nothing) — purely opt-in.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub launch_overrides: std::collections::BTreeMap<String, LaunchOverride>,
    /// `[input]` — guide-button hold threshold + session hotkey toggle.
    #[serde(default)]
    pub input: InputConfig,
    /// `[media_server]` — Jellyfin browse/play integration (media_server.rs). Empty =
    /// unconfigured; the jellyfin-mpv-shim pairing is adopted as a fallback at runtime.
    #[serde(default)]
    pub media_server: crate::media_server::MediaServerConfig,
    /// `[screensaver]` — OLED burn-in protection timings (gamepad.rs idle events +
    /// frontend overlay).
    #[serde(default)]
    pub screensaver: ScreensaverConfig,
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
    /// Whether a parental PIN is set. IPC-only (set by `get_config`, never written to disk):
    /// the webview needs presence, not the hash — `settings.pin_hash` is masked over IPC
    /// like `media_server.token`, since a 4–6 digit PIN is offline-crackable from its hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional = nullable))] // absent over IPC unless get_config sets it
    pub has_pin: Option<bool>,
}

/// The XDG config base: $XDG_CONFIG_HOME (when absolute), else ~/.config. Shared with
/// callers that peek at OTHER apps' config (e.g. jellyfin-mpv-shim's server address).
pub fn config_base() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

fn config_path() -> Option<PathBuf> {
    Some(config_base()?.join("omnideck/config.toml"))
}

fn defaults() -> Config {
    Config {
        settings: Settings {
            onboarded: false, // a fresh install runs the onboarding wizard
            ..Default::default()
        },
        media_server: Default::default(),
        screensaver: Default::default(),
        launch_overrides: Default::default(),
        input: Default::default(),
        apps: apps::list(),
        favorites: Vec::new(),
        recent_apps: Vec::new(),
        config_path: String::new(),
        config_error: None,
        has_pin: None,
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
                    tracing::warn!("config.toml parse error — using defaults:\n{e}");
                    let first = e.to_string().lines().next().unwrap_or("parse error").to_string();
                    error_defaults(format!("config.toml: {first} — using defaults until fixed"))
                }
            },
            Err(e) => {
                tracing::warn!("could not read config.toml: {e} — using defaults");
                error_defaults(format!("Couldn't read config.toml ({e}) — using defaults"))
            }
        };
        cfg.settings.normalize(); // defend against out-of-range values in a hand-edited config
        cfg.media_server.normalize();
        cfg.screensaver.normalize();
        for ov in cfg.launch_overrides.values_mut() {
            ov.normalize();
        }
        cfg.input.normalize();
        cfg.config_path = path_str;
        return cfg;
    }

    // First run: write a default config the user can edit (atomically — a crash mid-write
    // must not leave a truncated TOML the load path then refuses to overwrite).
    let mut cfg = defaults();
    if let Ok(text) = toml::to_string_pretty(&cfg) {
        let _ = write_atomic(&path, text.as_bytes());
    }
    cfg.config_path = path_str;
    cfg
}

/// Serializes the whole load→mutate→write sequence. Settings, favorites, apps, and recents
/// are separate IPC calls that each reload-and-rewrite the ENTIRE file; without this, two
/// concurrent saves could both read the same old state and the later completion would drop
/// the other's change. Also covers the first-run/default write path via `mutate_and_save`.
static SAVE_LOCK: Mutex<()> = Mutex::new(());
/// Per-process counter so concurrent atomic writes get distinct temp names.
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Write `bytes` to `path` atomically: fully write a unique temp sibling, fsync it, then
/// rename over the destination (rename is atomic within a filesystem). A crash, power loss,
/// I/O error, or full disk during the write leaves the PREVIOUS file intact rather than a
/// half-written one — which matters here because the load path deliberately refuses to
/// overwrite an unparseable config, so one truncated write would otherwise wedge all future
/// saves until the user repaired the file by hand.
///
/// Rename-replace has two sharp edges the plain `fs::write` it replaced didn't, both
/// handled here: a symlinked destination is resolved first (write through to the TARGET —
/// renaming over the link would sever a dotfiles-managed config), and the destination's
/// permissions are copied onto the temp before the swap (a chmod-600 config holding the
/// media-server token must not come back as umask 0644 after every auto-save).
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dest = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let parent = dest
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no parent"))?;
    fs::create_dir_all(parent)?;
    let stem = dest.file_name().and_then(|n| n.to_str()).unwrap_or("config");
    let tmp = parent.join(format!(
        ".{stem}.tmp-{}-{}",
        std::process::id(),
        TMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    // Scope the file so it's closed before the rename.
    let write = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all() // contents durable before we swap it in
    })();
    if let Err(e) = write {
        let _ = fs::remove_file(&tmp); // don't leave the partial temp behind
        return Err(e);
    }
    if let Ok(meta) = fs::metadata(&dest) {
        let _ = fs::set_permissions(&tmp, meta.permissions());
    }
    fs::rename(&tmp, &dest)?;
    // Best-effort: fsync the directory so the rename itself survives power loss.
    if let Ok(dir) = fs::File::open(parent) {
        let _ = dir.sync_all();
    }
    Ok(())
}

/// Shared save path: reload the on-disk config, apply `mutate`, write it back. Refuses to
/// write while the file is in the parse/read-error state — otherwise any save (recent apps
/// fire automatically on every launch) would replace the user's config with the in-memory
/// defaults, exactly the clobber the load path promises not to do. Internal IPC-only fields
/// (`config_path`, `config_error`) are stripped before serializing.
fn mutate_and_save(mutate: impl FnOnce(&mut Config)) -> Result<(), String> {
    // Hold the lock across the whole read-modify-write so concurrent saves serialize instead
    // of clobbering each other. Poison-tolerant: a panic in another saver must not wedge saves.
    let _guard = SAVE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = config_path().ok_or("no config path")?;
    let mut cfg = load_or_create();
    if let Some(err) = cfg.config_error.take() {
        return Err(format!("not saving over a broken config.toml — {err}"));
    }
    mutate(&mut cfg);
    cfg.config_path = String::new(); // never written to disk
    cfg.has_pin = None; // IPC-only, never written to disk
    let text = toml::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    write_atomic(&path, text.as_bytes()).map_err(|e| e.to_string())
}

/// Persist new settings, preserving the apps list and not writing internal fields.
/// The parental-control fields are deliberately preserved from disk (server-authoritative):
/// - `pin_hash` is only written by `save_pin_hash` (via the `set_pin` command, which
///   verifies the current PIN first);
/// - `locked_categories` is only written by `save_locked_categories` (via the
///   `set_locked_categories` command, PIN-verified when a PIN is set) — otherwise a plain
///   settings save could empty the locked list and unlock everything without the PIN.
///
/// A settings payload can therefore neither clear/replace the PIN nor change the locks.
pub fn save_settings(settings: Settings) -> Result<(), String> {
    mutate_and_save(|cfg| {
        let pin_hash = std::mem::take(&mut cfg.settings.pin_hash);
        let locked = std::mem::take(&mut cfg.settings.locked_categories);
        cfg.settings = settings;
        cfg.settings.pin_hash = pin_hash;
        cfg.settings.locked_categories = locked;
    })
}

/// Persist a new PIN hash (empty = lock removed). Only pin.rs::set_pin calls this,
/// after verifying the current PIN.
pub fn save_pin_hash(pin_hash: String) -> Result<(), String> {
    mutate_and_save(|cfg| cfg.settings.pin_hash = pin_hash)
}

/// Persist the PIN-locked category ids. Only pin.rs::set_locked_categories calls this,
/// after verifying the PIN (when one is set).
pub fn save_locked_categories(locked_categories: Vec<String>) -> Result<(), String> {
    mutate_and_save(|cfg| cfg.settings.locked_categories = locked_categories)
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

// ---- Backup / restore (roadmap #5) ----
//
// config.toml accumulates curated state (custom launchers, favorites, recents, media-server
// pairing). Backup is a sanitized TOML snapshot the user can carry between machines; restore
// funnels the file through the same `normalize()` gates as a hand-edited config, so a crafted
// backup can't inject anything a hand-edit couldn't.

/// Prepare an in-memory config for writing to a backup file: strip the IPC-only fields and,
/// unless the user opts in, blank the credential-shaped fields (the threat model is "the user
/// emails themselves the zip" — see NOTES-DEEPDIVE-ROADMAP.md #5).
fn sanitize_for_backup(mut cfg: Config, include_credentials: bool) -> Config {
    cfg.config_path = String::new();
    cfg.config_error = None;
    if !include_credentials {
        cfg.media_server.token = String::new();
        cfg.settings.steamgriddb_key = String::new();
    }
    cfg
}

/// Parse + sanitize backup text. Every field goes through the same `normalize()` pass as a
/// hand-edited config.toml, so restore is exactly as safe as editing the file by hand.
fn parse_backup(text: &str) -> Result<Config, String> {
    let mut cfg: Config =
        toml::from_str(text).map_err(|e| {
            let first = e.to_string().lines().next().unwrap_or("parse error").to_string();
            format!("not a valid OmniDeck backup: {first}")
        })?;
    cfg.settings.normalize();
    cfg.media_server.normalize();
    cfg.config_path = String::new();
    cfg.config_error = None;
    Ok(cfg)
}

/// Write a backup of the current config.toml to `dest`. Refuses while the on-disk config is in
/// the parse-error state (the in-memory defaults are NOT the user's config — backing them up
/// would launder the broken state into a "good" backup). Returns the path written.
pub fn backup_to(dest: &std::path::Path, include_credentials: bool) -> Result<String, String> {
    if !dest.is_absolute() {
        return Err("backup destination must be an absolute path".into());
    }
    let mut cfg = load_or_create();
    if let Some(err) = cfg.config_error.take() {
        return Err(format!("not backing up a broken config.toml — {err}"));
    }
    let cfg = sanitize_for_backup(cfg, include_credentials);
    let text = toml::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    // write_atomic creates the parent dir and never leaves a truncated backup.
    write_atomic(dest, text.as_bytes()).map_err(|e| e.to_string())?;
    Ok(dest.to_string_lossy().into_owned())
}

/// Replace config.toml with the (sanitized) contents of a backup file, then return the freshly
/// loaded config so the UI can re-render. Unlike `mutate_and_save` this intentionally works
/// while the current config is broken — restoring a backup is exactly how the user fixes that.
/// Credential fields left empty in the backup (the default) keep their current on-disk values,
/// so a sanitized backup never wipes a working pairing.
pub fn restore_from(src: &std::path::Path) -> Result<Config, String> {
    let text = fs::read_to_string(src).map_err(|e| format!("couldn't read backup: {e}"))?;
    let mut cfg = parse_backup(&text)?;

    // Serialize with every other config write: an auto-save (recents fire on launch) racing
    // the restore must not interleave with it. Poison-tolerant like mutate_and_save.
    let _guard = SAVE_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // Keep current credentials when the backup carries none (sanitized backups blank them).
    let current = load_or_create();
    if current.config_error.is_none() {
        if cfg.media_server.token.is_empty() {
            cfg.media_server.token = current.media_server.token;
        }
        if cfg.settings.steamgriddb_key.is_empty() {
            cfg.settings.steamgriddb_key = current.settings.steamgriddb_key;
        }
    }

    let path = config_path().ok_or("no config path")?;
    let out = toml::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    // Atomic replace: a crash mid-restore must not leave a truncated config.toml that the
    // load path then refuses to overwrite (the exact clobber-protection deadlock).
    write_atomic(&path, out.as_bytes()).map_err(|e| e.to_string())?;
    Ok(load_or_create())
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
    use super::{parse_backup, sanitize_for_backup, write_atomic, Config, InputConfig, LaunchOverride, ScreensaverConfig, Settings};

    #[test]
    fn write_atomic_creates_overwrites_and_leaves_no_temp() {
        let dir = std::env::temp_dir().join(format!("omnideck-cfgtest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("config.toml");

        // Fresh write (dir doesn't exist yet — write_atomic must create it).
        write_atomic(&path, b"first").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"first");

        // Overwrite with different-length content: no truncation artifacts, full replace.
        write_atomic(&path, b"second-and-longer").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second-and-longer");

        // The temp sibling was renamed away, not left behind — only config.toml remains.
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(leftovers, vec!["config.toml".to_string()], "stray temp file left: {leftovers:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn launch_override_normalize_drops_unspawnable_entries() {
        let mut ov = LaunchOverride::default();
        ov.env.insert("MANGOHUD".into(), "1".into());
        ov.env.insert("".into(), "x".into()); // empty key
        ov.env.insert("A=B".into(), "x".into()); // '=' in key
        ov.env.insert("NULKEY\0".into(), "x".into()); // NUL in key
        ov.env.insert("NULVAL".into(), "x\0y".into()); // NUL in value
        ov.args = vec!["--verbose".into(), "bad\0arg".into()];
        ov.normalize();
        assert_eq!(ov.env.len(), 1);
        assert_eq!(ov.env.get("MANGOHUD").map(String::as_str), Some("1"));
        assert_eq!(ov.args, vec!["--verbose".to_string()]);
    }

    #[test]
    fn launch_overrides_parse_from_toml() {
        let text = r#"
[launch_overrides."app:retroarch"]
env = { MANGOHUD = "1" }
args = ["--verbose"]
"#;
        let cfg: super::Config = toml::from_str(text).unwrap();
        let ov = cfg.launch_overrides.get("app:retroarch").expect("override parsed");
        assert_eq!(ov.env.get("MANGOHUD").map(String::as_str), Some("1"));
        assert_eq!(ov.args, vec!["--verbose".to_string()]);
        // Empty map stays out of the serialized default config (opt-in table).
        let out = toml::to_string_pretty(&super::Config::default()).unwrap();
        assert!(!out.contains("launch_overrides"), "{out}");
    }

    #[test]
    fn backup_strips_credentials_by_default() {
        let mut cfg = Config::default();
        cfg.settings.steamgriddb_key = "sgdb-secret".into();
        cfg.media_server.token = "jf-secret".into();
        cfg.config_path = "/home/x/.config/omnideck/config.toml".into();
        cfg.config_error = Some("boom".into());

        let clean = sanitize_for_backup(cfg.clone(), false);
        assert_eq!(clean.settings.steamgriddb_key, "");
        assert_eq!(clean.media_server.token, "");
        assert_eq!(clean.config_path, "");
        assert!(clean.config_error.is_none());

        let keep = sanitize_for_backup(cfg, true);
        assert_eq!(keep.settings.steamgriddb_key, "sgdb-secret");
        assert_eq!(keep.media_server.token, "jf-secret");
        assert_eq!(keep.config_path, ""); // IPC-only fields always stripped
    }

    #[test]
    fn backup_roundtrips_through_parse() {
        let mut cfg = Config::default();
        cfg.settings.grid_columns = 8;
        cfg.favorites = vec!["steam:123".into()];
        cfg.recent_apps = vec!["app:firefox".into()];
        let text = toml::to_string_pretty(&sanitize_for_backup(cfg, false)).unwrap();
        let back = parse_backup(&text).unwrap();
        assert_eq!(back.settings.grid_columns, 8);
        assert_eq!(back.favorites, vec!["steam:123".to_string()]);
        assert_eq!(back.recent_apps, vec!["app:firefox".to_string()]);
    }

    #[test]
    fn restore_normalizes_hostile_backup() {
        // A crafted backup goes through the same normalize() gates as a hand-edited config.
        let text = r##"
[settings]
accent = "red; background:url(http://evil)"
search_provider = "javascript:alert(1)"
ui_scale_custom = 99.0
grid_columns = 0
"##;
        let cfg = parse_backup(text).unwrap();
        assert_eq!(cfg.settings.accent, "#4cc2ff");
        assert_eq!(cfg.settings.search_provider, "");
        assert_eq!(cfg.settings.ui_scale_custom, 3.5);
        assert_eq!(cfg.settings.grid_columns, 1);
    }

    #[test]
    fn restore_rejects_garbage_with_first_line() {
        let err = match parse_backup("this is { not toml") {
            Ok(_) => panic!("garbage should not parse"),
            Err(e) => e,
        };
        assert!(err.starts_with("not a valid OmniDeck backup:"), "{err}");
        assert_eq!(err.lines().count(), 1, "error should be toast-sized: {err}");
    }

    #[test]
    fn input_config_clamps_hold_threshold() {
        let mut i = InputConfig { guide_hold_ms: 50, ..Default::default() };
        i.normalize();
        assert_eq!(i.guide_hold_ms, 200); // a tap must never read as a "close" hold

        let mut i = InputConfig { guide_hold_ms: 60_000, ..Default::default() };
        i.normalize();
        assert_eq!(i.guide_hold_ms, 5000);

        let mut i = InputConfig::default();
        i.normalize();
        assert_eq!(i.guide_hold_ms, 800); // default untouched
        assert!(i.session_hotkeys);
    }

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

    #[test]
    fn screensaver_defaults_enabled_with_staged_timings() {
        let ss = ScreensaverConfig::default();
        assert!(ss.enabled);
        assert_eq!(ss.idle_dim_secs, 60);
        assert_eq!(ss.ken_burns_secs, 180);
        assert_eq!(ss.blank_secs, 600);
        // An empty `[screensaver]` table (or a missing one) must land on the same defaults —
        // in particular `enabled` must be true, not the bool-derive false.
        let ss: ScreensaverConfig = toml::from_str("").unwrap();
        assert!(ss.enabled);
        assert_eq!(ss.blank_secs, 600);
    }

    #[test]
    fn screensaver_normalize_clamps_and_orders_stages() {
        // Too-small / overflowing values clamp into range.
        let mut ss = ScreensaverConfig { idle_dim_secs: 0, ken_burns_secs: 0, blank_secs: u64::MAX, ..Default::default() };
        ss.normalize();
        assert_eq!(ss.idle_dim_secs, 5);
        assert_eq!(ss.ken_burns_secs, 5); // floored to idle_dim
        assert_eq!(ss.blank_secs, 86_400);

        // Stages hand-edited out of order are re-floored to stay monotonic.
        let mut ss = ScreensaverConfig { idle_dim_secs: 300, ken_burns_secs: 30, blank_secs: 60, ..Default::default() };
        ss.normalize();
        assert_eq!(ss.idle_dim_secs, 300);
        assert_eq!(ss.ken_burns_secs, 300);
        assert_eq!(ss.blank_secs, 300);

        // In-range, ordered values pass through untouched.
        let mut ss = ScreensaverConfig { idle_dim_secs: 30, ken_burns_secs: 120, blank_secs: 300, enabled: false };
        ss.normalize();
        assert_eq!((ss.idle_dim_secs, ss.ken_burns_secs, ss.blank_secs), (30, 120, 300));
        assert!(!ss.enabled); // normalize never touches the toggle
    }
}
