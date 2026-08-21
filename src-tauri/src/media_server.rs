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
    /// Resume point in whole seconds (UserData.PlaybackPositionTicks). None = start from the
    /// top, so "does this have a resume point" is one null-check on the frontend.
    pub position_secs: Option<u64>,
    /// UserData.Played — the server's fully-watched flag, for the ✓ marker on browse rows.
    pub played: Option<bool>,
}

/// Jellyfin ticks (100 ns each) → whole seconds, rounding down. Sub-second precision is
/// noise for a resume point and mpv's `--start` is happier with an integer.
fn ticks_to_secs(ticks: u64) -> u64 {
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
    user_id: OnceLock<String>, // resolved lazily via /Users/Me; only SUCCESSES are cached
    preknown_user: Option<String>,
}

/// A syntactically-valid Jellyfin item id: non-empty, length-bounded, and only the characters
/// real ids use (hex GUIDs, occasionally dashed). Rejects anything that could alter the request
/// path or query (`/`, `?`, `&`, `.`) — the Tauri media commands accept arbitrary frontend
/// strings, and these ids are interpolated straight into URLs.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
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
    // Poison-tolerant (`into_inner`), like every other lock in the codebase: one panic
    // inside a holder must degrade to a re-resolve, not wedge the media subsystem with
    // a poisoned-lock panic for the rest of the process lifetime.
    if let Some(cached) = SERVER.read().unwrap_or_else(|e| e.into_inner()).clone() {
        return cached;
    }
    // Resolve outside the lock (config + shim file I/O); a racing thread may resolve too —
    // get_or_insert keeps whichever landed first, both read the same config.
    let resolved = resolve();
    SERVER.write().unwrap_or_else(|e| e.into_inner()).get_or_insert(resolved).clone()
}

/// Drop the cached resolution so the next `server()` call re-reads config.toml / the shim
/// pairing. Called after every config save (config::mutate_and_save).
pub fn invalidate() {
    *SERVER.write().unwrap_or_else(|e| e.into_inner()) = None;
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
        let resp = self.send(reqwest::Method::GET, &url).await?;
        if !resp.status().is_success() {
            return Err(format!("media server: HTTP {} on {path}", resp.status()));
        }
        resp.json().await.map_err(|e| format!("media server: bad JSON: {e}"))
    }

    /// A request with one retry on a *transient* network error (connect/timeout) — a living-room
    /// box's wifi/DNS can blip, and a single retry turns a spurious empty section into a normal
    /// load. HTTP status errors are deterministic and are NOT retried.
    ///
    /// Retrying is only safe because every verb this sends is idempotent: GET reads, and
    /// `PlayedItems` POST/DELETE set an *absolute* watched state rather than incrementing a
    /// counter, so a replayed request lands on the same result. Keep that true of anything
    /// added here — a non-idempotent verb would be double-applied by the retry.
    async fn send(&self, method: reqwest::Method, url: &str) -> Result<reqwest::Response, String> {
        let once = || {
            crate::http::client()
                .request(method.clone(), url)
                .header("X-Emby-Token", &self.token)
                .send()
        };
        let resp = match once().await {
            Ok(r) => r,
            Err(e) if e.is_timeout() || e.is_connect() => {
                tracing::debug!("media server: transient error on {url}, retrying once: {e}");
                once().await.map_err(|e| format!("media server unreachable: {e}"))?
            }
            Err(e) => return Err(format!("media server unreachable: {e}")),
        };
        // The shared client follows redirects, and a 301/302/303 hop rewrites the method
        // to GET (reqwest's standard redirect behaviour). Reads tolerate that — an
        // http→https reverse proxy keeps every browse working — but it silently turns a
        // mutating verb into a no-op read of the redirect target. Fail loudly instead of
        // letting the read path and the write path diverge exactly where the couch can't
        // see it: the base URL is a configured known host, so a redirect here means the
        // config points at the wrong address.
        if method != reqwest::Method::GET {
            let requested = reqwest::Url::parse(url)
                .map_err(|e| format!("media server: bad URL {url}: {e}"))?;
            if *resp.url() != requested {
                return Err(format!(
                    "media server: {method} was redirected to {} (redirects downgrade it to GET, so the change was NOT applied) — point media_server.url at the server's canonical address",
                    resp.url()
                ));
            }
        }
        Ok(resp)
    }

    async fn user(&self) -> Result<String, String> {
        // The user id is interpolated into request paths exactly like item ids, and it
        // arrives from outside (config.toml / the shim's cred.json / the server's own
        // /Users/Me answer) — gate it with the same charset check as valid_id so a value
        // containing `/` or `?` can't reshape a path, least of all PlayedItems' mutating one.
        if let Some(u) = &self.preknown_user {
            if !valid_id(u) {
                return Err("media server: configured user id is not a valid Jellyfin id".into());
            }
            return Ok(u.clone());
        }
        if let Some(u) = self.user_id.get() {
            return Ok(u.clone()); // only ever cached on success, so this is never a poisoned None
        }
        let me = self.get("/Users/Me").await?;
        let id = me
            .get("Id")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| "media server: couldn't resolve user".to_string())?;
        if !valid_id(&id) {
            return Err("media server: /Users/Me returned an invalid user id".into());
        }
        let _ = self.user_id.set(id.clone()); // cache the success; a transient failure isn't sticky
        Ok(id)
    }

    pub async fn sections(&self) -> Result<MediaSections, String> {
        let user = self.user().await?;
        // The four fetches are independent — run them concurrently instead of stacking
        // four serial LAN round-trips on every media-screen open. join! (not try_join!):
        // resume/latest/server-name degrade per-section below; only Views failing fails
        // the whole screen.
        let views_url = format!("/Users/{user}/Views");
        let resume_url =
            format!("/Users/{user}/Items/Resume?Limit=12&MediaTypes=Video&Fields=Overview");
        let latest_url =
            format!("/Users/{user}/Items/Latest?Limit=16&IncludeItemTypes=Movie,Series");
        let (views, resume_res, latest_res, info_res) = tokio::join!(
            self.get(&views_url),
            self.get(&resume_url),
            self.get(&latest_url),
            self.get("/System/Info/Public"),
        );
        let views = views?;
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
        // A failed row degrades to empty rather than failing the whole screen, but it's LOGGED
        // now (was silently indistinguishable from a genuinely empty library) and the get()
        // retry above already absorbs a single transient blip.
        let resume = match resume_res {
            Ok(v) => items_of(&v["Items"]),
            Err(e) => {
                tracing::warn!("media: Continue Watching unavailable ({e}) — showing none");
                Vec::new()
            }
        };
        let latest = match latest_res {
            Ok(v) => items_of(&v), // /Latest returns a bare array
            Err(e) => {
                tracing::warn!("media: Latest unavailable ({e}) — showing none");
                Vec::new()
            }
        };
        let name = info_res
            .ok()
            .and_then(|v| v["ServerName"].as_str().map(str::to_string))
            .unwrap_or_else(|| "Jellyfin".into());
        Ok(MediaSections { server_name: name, resume, latest, libraries })
    }

    /// Children of a library or series/season — one call covers every drill-down level.
    pub async fn browse(&self, parent: &str) -> Result<Vec<MediaItem>, String> {
        if !valid_id(parent) {
            return Err("invalid item id".into());
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

    /// Mark an item watched (`played = true`) or unwatched on the server — Jellyfin's
    /// `PlayedItems`: POST sets the flag, DELETE clears it. Both set an absolute state, so
    /// the transport retry above can't double-apply and a double press from the couch lands
    /// exactly where the label promised.
    ///
    /// Marking watched also clears the item's server-side resume point (this is Jellyfin's
    /// behaviour, not ours) — that's the expected meaning of "I'm done with this", but it is
    /// why the caller gates this to a single playable item and not a whole series: un-marking
    /// restores the flag, never the positions.
    pub async fn set_played(&self, id: &str, played: bool) -> Result<(), String> {
        if !valid_id(id) {
            return Err("invalid item id".into());
        }
        let user = self.user().await?;
        let (method, path) = played_request(&user, id, played);
        let resp = self.send(method, &format!("{}{path}", self.base)).await?;
        if !resp.status().is_success() {
            return Err(format!("media server: HTTP {} marking watched", resp.status()));
        }
        // Jellyfin answers PlayedItems with the item's UserItemDataDto, whose `Played`
        // field is the authoritative outcome — and deployments exist where the route
        // answers 200 without applying (jellyfin#8168). The optimistic ✓ upstream can
        // only roll back on an Err, so when the body is parseable, hold the server to
        // its word; an empty or non-JSON body still counts as success (be lenient with
        // Emby-lineage servers that answer 204).
        if let Ok(v) = resp.json::<serde_json::Value>().await {
            if let Some(got) = v.get("Played").and_then(|p| p.as_bool()) {
                if got != played {
                    return Err(format!(
                        "media server: acknowledged the request but reports Played={got} (expected {played})"
                    ));
                }
            }
        }
        Ok(())
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
        // valid_id landed on main after the pick was authored: the id is interpolated into
        // poster_url, so the injection gate stays even though the fetch moved to the cache.
        if !valid_id(id) {
            return None;
        }
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

/// The verb + path for a watch-state change: Jellyfin's `PlayedItems` uses the SAME path in
/// both directions and only flips the method (POST sets, DELETE clears). Split out of
/// `set_played` so that choice is unit-testable without a live server — the same reason
/// `mpv_start_flag` lives outside the launch path.
fn played_request(user: &str, id: &str, played: bool) -> (reqwest::Method, String) {
    let method = if played { reqwest::Method::POST } else { reqwest::Method::DELETE };
    (method, format!("/Users/{user}/PlayedItems/{id}"))
}

fn items_of(v: &serde_json::Value) -> Vec<MediaItem> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|i| {
                    let ud = &i["UserData"]; // one lookup; missing UserData indexes as Null
                    Some(MediaItem {
                        id: i["Id"].as_str()?.to_string(),
                        name: i["Name"].as_str()?.to_string(),
                        kind: i["Type"].as_str().unwrap_or("").to_string(),
                        overview: i["Overview"].as_str().map(str::to_string),
                        played_pct: ud["PlayedPercentage"].as_f64(),
                        runtime_mins: i["RunTimeTicks"].as_u64().map(|t| ticks_to_secs(t) / 60),
                        series: i["SeriesName"].as_str().map(str::to_string),
                        // 0 ticks means "no resume point", NOT "resume at 0:00" — collapse it
                        // to None so a never-started item can't ask mpv to seek.
                        position_secs: ud["PlaybackPositionTicks"]
                            .as_u64()
                            .map(ticks_to_secs)
                            .filter(|s| *s > 0),
                        played: ud["Played"].as_bool(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{items_of, played_request, ticks_to_secs, valid_id, JellyfinServer, MediaServerConfig};

    #[test]
    fn set_played_guards_reject_before_any_network_io() {
        // base points at a loopback port nothing listens on: if either guard failed to
        // short-circuit, these would surface "unreachable" instead of the guard's message.
        let srv = |user: &str| JellyfinServer {
            base: "http://127.0.0.1:9".into(),
            token: "t".into(),
            user_id: std::sync::OnceLock::new(),
            preknown_user: Some(user.into()),
        };
        let bad_id =
            tauri::async_runtime::block_on(srv("u1").set_played("../etc", true)).unwrap_err();
        assert_eq!(bad_id, "invalid item id");
        // The user id reaches the same path interpolation as the item id — same gate.
        let bad_user =
            tauri::async_runtime::block_on(srv("u1/../admin").set_played("abc", true)).unwrap_err();
        assert!(bad_user.contains("user id"), "{bad_user}");
    }

    #[test]
    fn played_request_flips_only_the_verb() {
        // Same path both ways, and both verbs set an ABSOLUTE state — that's what makes the
        // transport's blind single retry safe, so pin it.
        let (set, set_path) = played_request("u1", "abc", true);
        let (clear, clear_path) = played_request("u1", "abc", false);
        assert_eq!(set, reqwest::Method::POST);
        assert_eq!(clear, reqwest::Method::DELETE);
        assert_eq!(set_path, "/Users/u1/PlayedItems/abc");
        assert_eq!(clear_path, set_path);
    }

    #[test]
    fn ticks_convert_to_whole_seconds() {
        assert_eq!(ticks_to_secs(0), 0);
        assert_eq!(ticks_to_secs(10_000_000), 1); // exactly 1 s
        assert_eq!(ticks_to_secs(9_999_999), 0); // sub-second rounds down
        assert_eq!(ticks_to_secs(28_500_000_000), 2850); // 47 min 30 s into a film
    }

    #[test]
    fn items_carry_the_resume_point_and_watched_flag() {
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
        assert_eq!(items[0].runtime_mins, Some(120));
        assert_eq!(items[0].played, Some(false));
        // 0 ticks = never started, so there is nothing to seek to.
        assert_eq!(items[1].position_secs, None);
        assert_eq!(items[1].played, Some(true));
        // No UserData at all → every watch-state field stays None.
        assert_eq!(items[2].position_secs, None);
        assert_eq!(items[2].played, None);
        assert_eq!(items[2].played_pct, None);
    }

    #[test]
    fn valid_id_accepts_jellyfin_ids_and_rejects_injection() {
        assert!(valid_id("f137a2dd21bbc1b99aa5c0f6bf02a805")); // 32-char hex GUID
        assert!(valid_id("a1b2-c3d4")); // occasionally dashed
        assert!(!valid_id("")); // empty
        assert!(!valid_id("../../System/Info")); // path traversal
        assert!(!valid_id("id&EnableTranscoding=true")); // query injection
        assert!(!valid_id("id/Images/Primary")); // path injection
        assert!(!valid_id(&"a".repeat(65))); // over the length bound
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

    #[test]
    fn art_cache_mb_defaults_zero_and_parses() {
        // Absent → 0 = "use artwork_cache's 200 MB default".
        let ms: MediaServerConfig = toml::from_str("").unwrap();
        assert_eq!(ms.art_cache_mb, 0);
        let ms: MediaServerConfig = toml::from_str("art_cache_mb = 512").unwrap();
        assert_eq!(ms.art_cache_mb, 512);
    }
}
