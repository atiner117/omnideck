// OmniDeck — optional SteamGridDB box-art fetcher.
// Only runs when the user sets `steamgriddb_key` in config. Fills in the vertical
// capsule (600x900) art that's often missing from Steam's local cache. Results are
// cached on disk so each game is fetched at most once.
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct Resp {
    #[serde(default)]
    data: Vec<Grid>,
}

#[derive(Deserialize)]
struct Grid {
    url: String,
}

fn cache_dir() -> Option<PathBuf> {
    // XDG: prefer $XDG_CACHE_HOME (when absolute), else ~/.cache (unchanged for existing installs).
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("omnideck/art"))
}

/// GET a URL, buffering at most `max` bytes — guards against an OOM from a huge or buggy
/// response (content-length can be absent or lie, so we cap the actual byte stream).
async fn fetch_capped(url: &str, max: usize) -> Option<Vec<u8>> {
    let mut resp = crate::http::client().get(url).send().await.ok()?;
    let mut buf = Vec::new();
    loop {
        match resp.chunk().await {
            Ok(Some(chunk)) => {
                if buf.len() + chunk.len() > max {
                    return None;
                }
                buf.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(_) => return None,
        }
    }
    Some(buf)
}

/// Returns a data URL for the game's vertical box art (cached or freshly fetched),
/// or None if no key, no result, or a network error.
pub async fn box_art(appid: &str, key: &str) -> Option<String> {
    if key.is_empty() {
        return None;
    }
    let dir = cache_dir()?;
    let _ = std::fs::create_dir_all(&dir);

    // Already cached?
    for ext in ["jpg", "png", "webp"] {
        let p = dir.join(format!("{appid}_box.{ext}"));
        if p.exists() {
            return to_data_url(&p);
        }
    }

    let api = format!(
        "https://www.steamgriddb.com/api/v2/grids/steam/{appid}?dimensions=600x900&types=static&limit=8&nsfw=false"
    );
    let resp = crate::http::client().get(&api).bearer_auth(key).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: Resp = resp.json().await.ok()?;
    let img_url = body.data.first()?.url.clone();
    // The asset URL comes straight from the API JSON — require https so a compromised/spoofed
    // response can't redirect the fetch to an internal service or a non-TLS endpoint.
    if !img_url.starts_with("https://") {
        return None;
    }
    let ext = if img_url.ends_with(".png") {
        "png"
    } else if img_url.ends_with(".webp") {
        "webp"
    } else {
        "jpg"
    };
    let bytes = fetch_capped(&img_url, 16 * 1024 * 1024).await?; // box art is ~KB–low MB; 16 MiB cap
    let path = dir.join(format!("{appid}_box.{ext}"));
    std::fs::write(&path, &bytes).ok()?;
    to_data_url(&path)
}

fn to_data_url(p: &Path) -> Option<String> {
    use base64::Engine;
    let bytes = std::fs::read(p).ok()?;
    let mime = match p.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    };
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}
