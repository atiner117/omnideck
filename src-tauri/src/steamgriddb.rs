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

pub fn cache_dir() -> Option<PathBuf> {
    // XDG: prefer $XDG_CACHE_HOME (when absolute), else ~/.cache (unchanged for existing installs).
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("omnideck/art"))
}

/// Size budget for the art cache. Capsules are ~0.1–1 MB, so this holds art for hundreds of
/// games; anything evicted just refetches on demand next time the tile scrolls into view.
const MAX_CACHE_BYTES: u64 = 100 * 1024 * 1024;

/// Keep `dir` under `max_bytes` by deleting oldest-modified files first (approximate LRU —
/// fetch recency, not read recency; serving art doesn't bump mtime and that's fine for a
/// regenerable cache). Called after each new write, so the just-written file is the newest
/// and survives. One readdir over a few hundred entries — cheap next to the fetch itself.
fn prune_cache(dir: &Path, max_bytes: u64) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = rd
        .flatten()
        .filter_map(|e| {
            let md = e.metadata().ok()?;
            if !md.is_file() {
                return None;
            }
            Some((md.modified().ok()?, md.len(), e.path()))
        })
        .collect();
    let total: u64 = files.iter().map(|(_, len, _)| len).sum();
    if total <= max_bytes {
        return;
    }
    files.sort_by_key(|(mtime, _, _)| *mtime); // oldest first
    let mut excess = total - max_bytes;
    for (_, len, path) in files {
        if excess == 0 {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            excess = excess.saturating_sub(len);
        }
    }
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

/// Returns the cache **file path** for the game's vertical box art (cached or freshly fetched),
/// or None if no key, no result, or a network error. The `omnideck://` asset protocol serves it.
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
            return Some(p.to_string_lossy().into_owned());
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
    prune_cache(&dir, MAX_CACHE_BYTES); // evict oldest art if the cache outgrew its budget
    Some(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::prune_cache;

    #[test]
    fn prunes_oldest_first_until_under_budget() {
        let dir = std::env::temp_dir().join(format!("omnideck-prune-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["a_oldest.jpg", "b_middle.jpg", "c_newest.jpg"] {
            std::fs::write(dir.join(name), [0u8; 10]).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20)); // distinct mtimes
        }
        prune_cache(&dir, 15); // total 30 > 15: must evict the two oldest
        assert!(!dir.join("a_oldest.jpg").exists());
        assert!(!dir.join("b_middle.jpg").exists());
        assert!(dir.join("c_newest.jpg").exists());
        prune_cache(&dir, 15); // under budget: no-op
        assert!(dir.join("c_newest.jpg").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
