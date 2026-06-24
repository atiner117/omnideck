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
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".cache/omnideck/art"))
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

    let client = reqwest::Client::new();
    let api = format!(
        "https://www.steamgriddb.com/api/v2/grids/steam/{appid}?dimensions=600x900&types=static&limit=8&nsfw=false"
    );
    let resp = client.get(&api).bearer_auth(key).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: Resp = resp.json().await.ok()?;
    let img_url = body.data.first()?.url.clone();
    let ext = if img_url.ends_with(".png") {
        "png"
    } else if img_url.ends_with(".webp") {
        "webp"
    } else {
        "jpg"
    };
    let bytes = reqwest::get(&img_url).await.ok()?.bytes().await.ok()?;
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
