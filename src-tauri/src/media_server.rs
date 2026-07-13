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
use std::path::PathBuf;
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
    /// Artwork disk-cache budget in MB (artwork_cache.rs LRU sweep). 0 (default) = 200 MB.
    pub art_cache_mb: u64,
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
            art_cache_mb: 0,
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
    async fn get(&self, path: &str) -> Result<serde_json::Value, String> {
        let url = format!("{}{path}", self.base);
        let resp = crate::http::client()
            .get(&url)
            .header("X-Emby-Token", &self.token)
            .send()
            .await
            .map_err(|e| format!("media server unreachable: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("media server: HTTP {} on {path}", resp.status()));
        }
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
        let _ = self.user_id.set(id.clone());
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
            .get(&format!("/Users/{user}/Items/Resume?Limit=12&MediaTypes=Video&Fields=Overview"))
            .await
            .map(|v| items_of(&v["Items"]))
            .unwrap_or_default();
        let latest = self
            .get(&format!("/Users/{user}/Items/Latest?Limit=16&IncludeItemTypes=Movie,Series"))
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

    /// Children of a library or series/season — one call covers every drill-down level.
    pub async fn browse(&self, parent: &str) -> Result<Vec<MediaItem>, String> {
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

    /// The server base URL (scheme://host[:port], no trailing slash) — the trust anchor
    /// for `commands::get_artwork`'s URL allowlist (artwork_cache::url_within_base).
    pub fn base(&self) -> &str {
        &self.base
    }

    /// The primary-poster URL for an item (the artwork_cache key for it).
    pub fn poster_url(&self, id: &str) -> String {
        format!("{}/Items/{id}/Images/Primary?maxWidth=480&quality=90", self.base)
    }

    /// Disk-cached primary poster; returns the local path for omnideck://. All the cache
    /// mechanics (ETag revalidation, atomic writes, LRU budget) live in artwork_cache.rs.
    pub async fn poster(&self, id: &str) -> Option<PathBuf> {
        crate::artwork_cache::get(&self.poster_url(id), Some(("X-Emby-Token", &self.token))).await
    }

    /// Warm the poster cache for `ids` in the background (artwork_cache::prefetch, 4
    /// workers): fired after the landing sections load so rail art is on disk before the
    /// tiles scroll into view — no pop-in on the next cold boot either.
    pub fn prefetch_posters(self: &Arc<Self>, ids: impl IntoIterator<Item = String>) {
        let urls: Vec<String> = ids.into_iter().map(|id| self.poster_url(&id)).collect();
        let auth = Some(("X-Emby-Token".to_string(), self.token.clone()));
        crate::artwork_cache::prefetch(urls, auth, 4);
    }
}

fn items_of(v: &serde_json::Value) -> Vec<MediaItem> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|i| {
                    Some(MediaItem {
                        id: i["Id"].as_str()?.to_string(),
                        name: i["Name"].as_str()?.to_string(),
                        kind: i["Type"].as_str().unwrap_or("").to_string(),
                        overview: i["Overview"].as_str().map(str::to_string),
                        played_pct: i["UserData"]["PlayedPercentage"].as_f64(),
                        runtime_mins: i["RunTimeTicks"].as_u64().map(|t| t / 600_000_000),
                        series: i["SeriesName"].as_str().map(str::to_string),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::MediaServerConfig;

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

    #[test]
    fn art_cache_mb_defaults_zero_and_parses() {
        // Absent → 0 = "use artwork_cache's 200 MB default".
        let ms: MediaServerConfig = toml::from_str("").unwrap();
        assert_eq!(ms.art_cache_mb, 0);
        let ms: MediaServerConfig = toml::from_str("art_cache_mb = 512").unwrap();
        assert_eq!(ms.art_cache_mb, 512);
    }
}
