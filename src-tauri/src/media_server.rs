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
// steamgriddb_key), is sent as an `X-Emby-Token` header (not in URLs) for API calls, and is
// NEVER logged or echoed to the webview (get_config blanks it; posters go through the
// rooted omnideck:// protocol so the frontend never sees an authenticated URL). The one
// exception is the mpv stream URL (api_key query param) — visible in the local process
// list, accepted for v1 like the rest of the single-user threat model.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

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
#[derive(Clone, Serialize, Deserialize, Default)]
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
    /// (VapourSynth filters need `hwdec=auto-copy`; a CLI `--hwdec` would override it).
    pub mpv_args: Vec<String>,
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

/// The resolved server for this run (config first, then shim pairing), or None.
pub fn server() -> Option<&'static JellyfinServer> {
    static SERVER: OnceLock<Option<JellyfinServer>> = OnceLock::new();
    SERVER
        .get_or_init(|| {
            let ms = crate::config::load_or_create().media_server;
            if ms.kind == "jellyfin" && !ms.url.is_empty() && !ms.token.is_empty() {
                return Some(JellyfinServer {
                    base: ms.url.trim_end_matches('/').to_string(),
                    token: ms.token,
                    user_id: OnceLock::new(),
                    preknown_user: None,
                });
            }
            shim_pairing()
        })
        .as_ref()
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
    /// the whole point: mpv + hwdec does the 4K work, not a server transcode.
    pub fn stream_url(&self, id: &str) -> String {
        format!("{}/Videos/{id}/stream?static=true&api_key={}", self.base, self.token)
    }

    /// Fetch + disk-cache the primary poster; returns the cached path for omnideck://.
    /// Extension comes from sniffing the bytes — the asset protocol serves by extension,
    /// so the cache file must carry a real one.
    pub async fn poster(&self, id: &str) -> Option<PathBuf> {
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
