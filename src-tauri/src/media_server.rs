// OmniDeck — media-server integration (Jellyfin-first; NOTES-DEEPDIVE-MEDIA-SERVER.md).
//
// Talks to the server's HTTP API to browse libraries and fetch posters, and hands playback
// to a real player (mpv direct-stream for 4K hwdec, or the desktop client). It does NOT
// re-implement Jellyfin's UI or scan local files — the server already solved metadata,
// resume points, and art.
//
// Configuration resolves in order:
//   1. the `[media_server]` table in config.toml (kind/url/token, Jellyfin only for now)
//   2. jellyfin-mpv-shim's pairing file (~/.config/jellyfin-mpv-shim/cred.json) — if the
//      user already paired the shim, OmniDeck adopts the same server + token, zero setup.
//
// Secrets: the token lives in config.toml (same plaintext, single-user posture as
// steamgriddb_key), is sent as an `X-Emby-Token` header (not in URLs) for API calls — the
// mpv stream included, via `--http-header-fields` — and is NEVER logged or echoed to the
// webview (get_config blanks it; posters go through the rooted omnideck:// protocol so the
// frontend never sees an authenticated URL). Keeping the token out of the URL keeps it out
// of URL-shaped surfaces (mpv's log/OSD/IPC, watch-later state, HTTP access logs); the mpv
// argv itself is still visible in the local process list, accepted for v1 like the rest of
// the single-user threat model.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct MediaLibrary {
    pub id: String,
    pub name: String,
    pub kind: String, // Jellyfin CollectionType: "movies" | "tvshows" | "music" | ...
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct MediaItem {
    pub id: String,
    pub name: String,
    pub kind: String, // "Movie" | "Series" | "Season" | "Episode" | ...
    pub overview: Option<String>,
    pub played_pct: Option<f64>,
    pub runtime_mins: Option<u64>,
    pub series: Option<String>, // parent series name for episodes
    /// Resume point in seconds (from UserData.PlaybackPositionTicks; 1 tick = 100 ns).
    /// None/0 = start from the beginning.
    pub position_secs: Option<u64>,
    /// Fully-watched flag (UserData.Played) — drives the watched checkmark + mark_unwatched.
    pub played: Option<bool>,
}

/// Jellyfin item/user ids are GUIDs — 32 hex chars, sometimes dashed. Gate every id the
/// FRONTEND supplies before it's interpolated into a URL path, so a crafted id can't smuggle
/// path segments (`../`) or query text into a request the token authenticates.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Jellyfin ticks (100 ns) → whole seconds, rounding down.
pub fn ticks_to_secs(ticks: u64) -> u64 {
    ticks / 10_000_000
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct MediaSections {
    pub server_name: String,
    pub resume: Vec<MediaItem>,
    pub latest: Vec<MediaItem>,
    pub libraries: Vec<MediaLibrary>,
}

/// `[media_server]` in config.toml. Empty kind/url = unconfigured (shim fallback applies).
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct MediaServerConfig {
    pub kind: String,  // "" | "jellyfin"  (emby/plex are future impls of the same shape)
    pub url: String,   // http(s)://host[:port]
    pub token: String, // Jellyfin API key or user access token; blanked over IPC
    pub prefer_mpv: bool,
    /// Extra mpv flags for direct-play, e.g. `["--include=~/.config/jellyfin-mpv-shim/mpv.conf"]`
    /// to reuse an existing profile set (VapourSynth interpolation/denoise, keybinds).
    /// When set, OmniDeck stops passing its own `--hwdec` so the config's choice rules
    /// (VapourSynth filters need `hwdec=auto-copy`; a CLI `--hwdec` would override it),
    /// and the auto-generated profile set below is not used.
    pub mpv_args: Vec<String>,
    /// Use OmniDeck's generated display-aware profile set (media_profiles.rs) when
    /// `mpv_args` is empty and mpv has VapourSynth. Default true; false = bare launch.
    pub auto_profiles: bool,
    /// Force mpv's audio output samplerate (Hz) in the generated profile set — e.g. 96000 for
    /// a fixed-rate DAC or LDAC headphones. 0 (default) leaves mpv's native rate (bit-perfect;
    /// forcing a rate resamples everything, so only set it when your gear wants a fixed rate).
    pub audio_samplerate: u32,
    /// Display refresh rate (Hz) to bake into the generated profiles and pass as
    /// `--display-fps-override`, for when OmniDeck can't detect it — i.e. daily use *outside*
    /// the gamescope session, where the RandR probe is unavailable and the profiles would
    /// otherwise fall back to 60. 0 (default) = auto-detect from the session's RandR mode.
    pub display_fps: f64,
}

/// Manual impl (not derived): `auto_profiles` must default ON — the derive would pick
/// `false`, silently disabling the feature for every config that doesn't mention it.
/// (`audio_samplerate`/`display_fps` default to 0 = "leave it alone", which the derive
/// would also give, but they ride along here to keep the whole default in one place.)
impl Default for MediaServerConfig {
    fn default() -> Self {
        Self {
            kind: String::new(),
            url: String::new(),
            token: String::new(),
            prefer_mpv: false,
            mpv_args: Vec::new(),
            auto_profiles: true,
            audio_samplerate: 0,
            display_fps: 0.0,
        }
    }
}

impl MediaServerConfig {
    /// Same posture as Settings::normalize — enum-check + URL-scheme-check hand-edited values.
    pub fn normalize(&mut self) {
        if !matches!(self.kind.as_str(), "" | "jellyfin") {
            self.kind.clear();
        }
        if !self.url.is_empty()
            && !self.url.starts_with("https://")
            && !self.url.starts_with("http://")
        {
            self.url.clear();
        }
        // Flags only — a bare word here would be handed to mpv as a filename/URL.
        self.mpv_args.retain(|a| a.starts_with("--"));
    }
}

pub struct JellyfinServer {
    base: String,
    token: String,
    user_id: OnceLock<Option<String>>, // resolved lazily via /Users/Me when not pre-known
    preknown_user: Option<String>,
}

/// Cached resolution: outer `None` = "not resolved yet / invalidated", inner `None` =
/// "resolved to unconfigured". A `RwLock` (not `OnceLock`) so `invalidate()` can drop the
/// cache when config.toml is saved — the frontend probes `media_available` at mount, and a
/// `OnceLock` would pin that first `None` until restart even after the user configures
/// Jellyfin in the UI.
static SERVER: RwLock<Option<Option<Arc<JellyfinServer>>>> = RwLock::new(None);

/// The currently-resolved server (config first, then shim pairing), or None. Re-resolves
/// lazily after `invalidate()`.
pub fn server() -> Option<Arc<JellyfinServer>> {
    if let Some(cached) = SERVER.read().unwrap().clone() {
        return cached;
    }
    // Resolve outside the lock (config + shim file I/O); a racing thread may resolve too —
    // get_or_insert keeps whichever landed first, both read the same config.
    let resolved = resolve();
    SERVER.write().unwrap().get_or_insert(resolved).clone()
}

/// Drop the cached resolution so the next `server()` call re-reads config.toml / the shim
/// pairing. Called after every config save (config::mutate_and_save).
pub fn invalidate() {
    *SERVER.write().unwrap() = None;
}

fn resolve() -> Option<Arc<JellyfinServer>> {
    let ms = crate::config::load_or_create().media_server;
    if ms.kind == "jellyfin" && !ms.url.is_empty() && !ms.token.is_empty() {
        return Some(Arc::new(JellyfinServer {
            base: ms.url.trim_end_matches('/').to_string(),
            token: ms.token,
            user_id: OnceLock::new(),
            preknown_user: None,
        }));
    }
    shim_pairing().map(Arc::new)
}

/// Adopt jellyfin-mpv-shim's pairing (address + user AccessToken + UserId). The file is a
/// JSON array of servers; we take the first one marked connected (or just the first).
fn shim_pairing() -> Option<JellyfinServer> {
    let path = crate::config::config_base()?.join("jellyfin-mpv-shim/cred.json");
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let servers = v.as_array()?;
    let s = servers
        .iter()
        .find(|s| s.get("connected").and_then(|c| c.as_bool()).unwrap_or(false))
        .or_else(|| servers.first())?;
    let base = s.get("address")?.as_str()?.trim_end_matches('/').to_string();
    let token = s.get("AccessToken")?.as_str()?.to_string();
    if !base.starts_with("http") || token.is_empty() {
        return None;
    }
    tracing::info!("media: adopted jellyfin-mpv-shim pairing for {base}");
    Some(JellyfinServer {
        base,
        token,
        user_id: OnceLock::new(),
        preknown_user: s.get("UserId").and_then(|u| u.as_str()).map(str::to_string),
    })
}

impl JellyfinServer {
    /// One authenticated request with a bounded retry: a single re-send after 250 ms on
    /// TRANSIENT transport errors only (connect refused mid-restart, timeout on a sleepy
    /// NAS spin-up). HTTP error statuses are returned immediately — a 401/404 won't get
    /// better by asking again. Both call sites (GET reads, PlayedItems POST/DELETE) are
    /// idempotent, so the retry can never double-apply anything.
    async fn request(&self, method: reqwest::Method, path: &str) -> Result<reqwest::Response, String> {
        let url = format!("{}{path}", self.base);
        for attempt in 0..2 {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
            let sent = crate::http::client()
                .request(method.clone(), &url)
                .header("X-Emby-Token", &self.token)
                .send()
                .await;
            match sent {
                Ok(resp) if resp.status().is_success() => return Ok(resp),
                Ok(resp) => return Err(format!("media server: HTTP {} on {path}", resp.status())),
                Err(e) if attempt == 0 && (e.is_connect() || e.is_timeout()) => {
                    tracing::debug!("media: transient error on {path}, retrying once: {e}");
                }
                Err(e) => return Err(format!("media server unreachable: {e}")),
            }
        }
        unreachable!("retry loop always returns")
    }

    async fn get(&self, path: &str) -> Result<serde_json::Value, String> {
        let resp = self.request(reqwest::Method::GET, path).await?;
        resp.json().await.map_err(|e| format!("media server: bad JSON: {e}"))
    }

    async fn user(&self) -> Result<String, String> {
        if let Some(u) = &self.preknown_user {
            return Ok(u.clone());
        }
        if let Some(u) = self.user_id.get() {
            return u.clone().ok_or_else(|| "no user".into());
        }
        let me = self.get("/Users/Me").await?;
        let id = me.get("Id").and_then(|v| v.as_str()).map(str::to_string);
        // Cache only a SUCCESSFUL resolve: a 200 with no Id (reverse proxy served an error
        // page as JSON, wrong endpoint behind a rewrite) must not poison the OnceLock until
        // restart — leave it unset so the next call re-asks.
        if id.is_some() {
            let _ = self.user_id.set(id.clone());
        }
        id.ok_or_else(|| "media server: couldn't resolve user".into())
    }

    pub async fn sections(&self) -> Result<MediaSections, String> {
        let user = self.user().await?;
        let views = self.get(&format!("/Users/{user}/Views")).await?;
        let libraries = views["Items"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| {
                        let kind = v["CollectionType"].as_str().unwrap_or("").to_string();
                        // v1 is video-shaped: skip music/playlist views (their tiles still exist).
                        if !matches!(kind.as_str(), "movies" | "tvshows" | "homevideos" | "") {
                            return None;
                        }
                        Some(MediaLibrary {
                            id: v["Id"].as_str()?.to_string(),
                            name: v["Name"].as_str()?.to_string(),
                            kind,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let resume = self
            .get(&resume_path(&user))
            .await
            .map(|v| items_of(&v["Items"]))
            .unwrap_or_default();
        let latest = self
            .get(&latest_path(&user))
            .await
            .map(|v| items_of(&v)) // /Latest returns a bare array
            .unwrap_or_default();
        let name = self
            .get("/System/Info/Public")
            .await
            .ok()
            .and_then(|v| v["ServerName"].as_str().map(str::to_string))
            .unwrap_or_else(|| "Jellyfin".into());
        Ok(MediaSections { server_name: name, resume, latest, libraries })
    }

    /// The Continue Watching rail: in-progress video items with their resume positions
    /// (`position_secs` from UserData.PlaybackPositionTicks).
    pub async fn continue_watching(&self) -> Result<Vec<MediaItem>, String> {
        let user = self.user().await?;
        let v = self.get(&resume_path(&user)).await?;
        Ok(items_of(&v["Items"]))
    }

    /// Recently-added movies/series (the "Latest" shelf). `/Items/Latest` returns a bare
    /// array, not an `{ Items: [...] }` envelope like the other listing endpoints.
    pub async fn recently_added(&self) -> Result<Vec<MediaItem>, String> {
        let user = self.user().await?;
        let v = self.get(&latest_path(&user)).await?;
        Ok(items_of(&v))
    }

    /// Set/clear the fully-watched flag: POST marks played, DELETE marks unplayed
    /// (`/Users/{user}/PlayedItems/{id}` — both are idempotent, safe under the retry).
    pub async fn set_played(&self, id: &str, played: bool) -> Result<(), String> {
        if !valid_id(id) {
            return Err("invalid media item id".into());
        }
        let user = self.user().await?;
        let method = if played { reqwest::Method::POST } else { reqwest::Method::DELETE };
        self.request(method, &played_path(&user, id)).await.map(|_| ())
    }

    /// Children of a library or series/season — one call covers every drill-down level.
    pub async fn browse(&self, parent: &str) -> Result<Vec<MediaItem>, String> {
        if !valid_id(parent) {
            return Err("invalid media item id".into());
        }
        let user = self.user().await?;
        // Non-recursive keeps the natural hierarchy (Series → Seasons → Episodes) and
        // matches how Jellyfin's own clients browse.
        let v = self
            .get(&format!(
                "/Users/{user}/Items?ParentId={parent}&SortBy=SortName&Fields=Overview&Limit=500"
            ))
            .await?;
        Ok(items_of(&v["Items"]))
    }

    /// Direct-play URL for mpv. `static=true` asks the server for the untranscoded file —
    /// the whole point: mpv + hwdec does the 4K work, not a server transcode. Carries NO
    /// credential: callers must authenticate with the `X-Emby-Token` header (see `token()`),
    /// which keeps the token out of URL-shaped surfaces (module comment).
    pub fn stream_url(&self, id: &str) -> String {
        format!("{}/Videos/{id}/stream?static=true", self.base)
    }

    /// The raw token, for callers that hand `stream_url()` to an external player and must
    /// pass the `X-Emby-Token` auth alongside it (mpv `--http-header-fields`, the CLI's
    /// debug probe). Don't log it, don't put it in a URL.
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Fetch + disk-cache the primary poster; returns the cached path for omnideck://.
    /// Extension comes from sniffing the bytes — the asset protocol serves by extension,
    /// so the cache file must carry a real one.
    pub async fn poster(&self, id: &str) -> Option<PathBuf> {
        if !valid_id(id) {
            return None;
        }
        let dir = poster_cache_dir()?;
        let _ = std::fs::create_dir_all(&dir);
        let safe = id.replace(['/', '.'], "_");
        for ext in ["jpg", "png", "webp"] {
            let p = dir.join(format!("{safe}.{ext}"));
            if p.exists() {
                return Some(p);
            }
        }
        let url = format!("{}/Items/{id}/Images/Primary?maxWidth=480&quality=90", self.base);
        let resp = crate::http::client()
            .get(&url)
            .header("X-Emby-Token", &self.token)
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let bytes = resp.bytes().await.ok()?;
        // Only cache real images (sniff like the icon path does — servers can 200 an error page).
        let ext = if bytes.starts_with(b"\xFF\xD8\xFF") {
            "jpg"
        } else if bytes.starts_with(b"\x89PNG") {
            "png"
        } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            "webp"
        } else {
            return None;
        };
        let path = dir.join(format!("{safe}.{ext}"));
        std::fs::write(&path, &bytes).ok()?;
        prune(&dir, 100 * 1024 * 1024);
        Some(path)
    }
}

// URL-path builders for the user-scoped endpoints, split out so tests can pin the exact
// request shapes without a live server. `user` comes from the server's own /Users/Me (or the
// shim pairing), never the frontend.
fn resume_path(user: &str) -> String {
    format!("/Users/{user}/Items/Resume?Limit=12&MediaTypes=Video&Fields=Overview")
}

fn latest_path(user: &str) -> String {
    format!("/Users/{user}/Items/Latest?Limit=16&IncludeItemTypes=Movie,Series")
}

fn played_path(user: &str, id: &str) -> String {
    format!("/Users/{user}/PlayedItems/{id}")
}

fn items_of(v: &serde_json::Value) -> Vec<MediaItem> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|i| {
                    // A 0-tick position means "no resume point" — map it to None so the
                    // frontend's `position_secs != null` check is the whole resume test.
                    let position_secs = i["UserData"]["PlaybackPositionTicks"]
                        .as_u64()
                        .map(ticks_to_secs)
                        .filter(|s| *s > 0);
                    Some(MediaItem {
                        id: i["Id"].as_str()?.to_string(),
                        name: i["Name"].as_str()?.to_string(),
                        kind: i["Type"].as_str().unwrap_or("").to_string(),
                        overview: i["Overview"].as_str().map(str::to_string),
                        played_pct: i["UserData"]["PlayedPercentage"].as_f64(),
                        runtime_mins: i["RunTimeTicks"].as_u64().map(|t| t / 600_000_000),
                        series: i["SeriesName"].as_str().map(str::to_string),
                        position_secs,
                        played: i["UserData"]["Played"].as_bool(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Poster cache root — also allowlisted in asset.rs so omnideck:// can serve it.
pub fn poster_cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("omnideck/media"))
}

/// Oldest-first eviction once the cache outgrows its budget (steamgriddb.rs pattern).
fn prune(dir: &Path, max_bytes: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            if !m.is_file() {
                return None;
            }
            Some((m.modified().ok()?, m.len(), e.path()))
        })
        .collect();
    let total: u64 = files.iter().map(|(_, len, _)| len).sum();
    if total <= max_bytes {
        return;
    }
    files.sort_by_key(|(t, _, _)| *t);
    let mut excess = total - max_bytes;
    for (_, len, path) in files {
        if std::fs::remove_file(&path).is_ok() {
            excess = excess.saturating_sub(len);
            if excess == 0 {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        items_of, latest_path, played_path, resume_path, ticks_to_secs, valid_id,
        MediaServerConfig,
    };

    #[test]
    fn ticks_convert_to_whole_seconds() {
        assert_eq!(ticks_to_secs(0), 0);
        assert_eq!(ticks_to_secs(10_000_000), 1); // 1 s exactly
        assert_eq!(ticks_to_secs(9_999_999), 0); // sub-second rounds down
        assert_eq!(ticks_to_secs(600_000_000), 60); // 1 min
        assert_eq!(ticks_to_secs(36_000_000_000), 3600); // 1 h
        // A real Jellyfin resume point: 47 min 30 s into a movie.
        assert_eq!(ticks_to_secs(28_500_000_000), 2850);
    }

    #[test]
    fn id_validation_accepts_guids_and_rejects_url_metacharacters() {
        assert!(valid_id("f137a2dd21bbc1b99aa5c0f6bf02a805")); // undashed GUID (Jellyfin's usual)
        assert!(valid_id("f137a2dd-21bb-c1b9-9aa5-c0f6bf02a805")); // dashed form
        assert!(!valid_id("")); // empty
        assert!(!valid_id("../Users/admin")); // path traversal
        assert!(!valid_id("abc?api_key=steal")); // query injection
        assert!(!valid_id("abc/def")); // extra path segment
        assert!(!valid_id("abc def")); // whitespace
        assert!(!valid_id(&"a".repeat(65))); // over the GUID-shaped budget
    }

    #[test]
    fn user_scoped_paths_have_the_documented_shapes() {
        let u = "f137a2dd21bbc1b99aa5c0f6bf02a805";
        assert_eq!(
            resume_path(u),
            format!("/Users/{u}/Items/Resume?Limit=12&MediaTypes=Video&Fields=Overview")
        );
        assert_eq!(
            latest_path(u),
            format!("/Users/{u}/Items/Latest?Limit=16&IncludeItemTypes=Movie,Series")
        );
        assert_eq!(played_path(u, "abc123"), format!("/Users/{u}/PlayedItems/abc123"));
    }

    #[test]
    fn items_parse_resume_position_and_played_flag() {
        let v = serde_json::json!([
            {
                "Id": "aaa", "Name": "Halfway Movie", "Type": "Movie",
                "RunTimeTicks": 72_000_000_000u64, // 2 h
                "UserData": {
                    "PlaybackPositionTicks": 36_000_000_000u64, // 1 h in
                    "PlayedPercentage": 50.0,
                    "Played": false
                }
            },
            {
                "Id": "bbb", "Name": "Finished Episode", "Type": "Episode",
                "SeriesName": "Some Show",
                "UserData": { "PlaybackPositionTicks": 0, "Played": true }
            },
            { "Id": "ccc", "Name": "Untouched", "Type": "Movie" }
        ]);
        let items = items_of(&v);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].position_secs, Some(3600));
        assert_eq!(items[0].played_pct, Some(50.0));
        assert_eq!(items[0].played, Some(false));
        assert_eq!(items[0].runtime_mins, Some(120));
        // 0 ticks = no resume point, not "resume at 0:00".
        assert_eq!(items[1].position_secs, None);
        assert_eq!(items[1].played, Some(true));
        assert_eq!(items[1].series.as_deref(), Some("Some Show"));
        // No UserData at all → all watch-state fields None.
        assert_eq!(items[2].position_secs, None);
        assert_eq!(items[2].played, None);
        assert_eq!(items[2].played_pct, None);
    }

    #[test]
    fn auto_profiles_defaults_on_for_configs_that_dont_mention_it() {
        // Every pre-0.2.0 config.toml lacks the key; the serde(default) container
        // fallback must come from OUR Default impl (true), not the derive's false.
        let ms: MediaServerConfig = toml::from_str("").unwrap();
        assert!(ms.auto_profiles);
        assert!(MediaServerConfig::default().auto_profiles);

        let ms: MediaServerConfig = toml::from_str("auto_profiles = false").unwrap();
        assert!(!ms.auto_profiles);
    }

    #[test]
    fn audio_and_display_fps_default_off_and_parse() {
        // Absent → 0 = "leave it alone" (no forced samplerate, auto-detect the refresh).
        let ms: MediaServerConfig = toml::from_str("").unwrap();
        assert_eq!(ms.audio_samplerate, 0);
        assert_eq!(ms.display_fps, 0.0);

        let ms: MediaServerConfig =
            toml::from_str("audio_samplerate = 96000\ndisplay_fps = 165.08").unwrap();
        assert_eq!(ms.audio_samplerate, 96000);
        assert!((ms.display_fps - 165.08).abs() < 1e-9);
    }
}
