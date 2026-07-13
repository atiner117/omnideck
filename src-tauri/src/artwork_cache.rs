// OmniDeck — artwork disk cache (Jellyfin posters + any media-server image URL).
//
// Cold boots used to re-fetch every poster over the network (art pop-in on the rail).
// This module makes remote artwork a disk-first resource:
//
//   · Layout: $XDG_CACHE_HOME/omnideck/artwork/<fnv1a64(url)>.<jpg|png|webp> plus a
//     `<hash>.meta` JSON sidecar holding the URL, the server's ETag/Last-Modified
//     validators, and the fetch time.
//   · Freshness: a hit younger than REVALIDATE_AFTER is served with ZERO network. Older
//     hits revalidate with If-None-Match/If-Modified-Since — a 304 costs headers only.
//     Network errors serve the stale copy (a launcher on flaky wifi still shows art).
//   · Writes are atomic (fsutil::write_atomic, temp-sibling + rename) so a power cut
//     mid-write can never leave a truncated image for the asset protocol to serve.
//   · Budget: size-capped LRU sweep after every store. Serving a hit bumps the data
//     file's mtime, so eviction order is true access recency (unlike the fetch-recency
//     pruning in steamgriddb.rs — posters re-show every boot, art fetched once and
//     watched daily must outlive art fetched yesterday and never opened).
//
// Security: this module fetches whatever URL it's given; the URL-trust decision lives at
// the boundary (`url_within_base` — commands::get_artwork only accepts URLs under the
// configured media server's base, so the webview can't turn the cache into an open proxy).
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Serve without any network for this long after a fetch/revalidation. Posters rarely
/// change; one conditional GET per item per day keeps edits visible without pop-in.
const REVALIDATE_AFTER_SECS: u64 = 24 * 60 * 60;

/// Default LRU budget when `[media_server] art_cache_mb` is 0/absent.
const DEFAULT_BUDGET_MB: u64 = 200;

/// Refuse to cache bodies larger than this (matches asset.rs's serve cap — a bigger file
/// could never be served anyway, so caching it would only burn budget).
const MAX_IMAGE_BYTES: u64 = 32 * 1024 * 1024;

const IMAGE_EXTS: [&str; 3] = ["jpg", "png", "webp"];

/// Cache root — allowlisted in asset.rs so `omnideck://` can serve from it.
pub fn cache_dir() -> Option<PathBuf> {
    // XDG: prefer $XDG_CACHE_HOME (when absolute), else ~/.cache (steamgriddb.rs pattern).
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("omnideck/artwork"))
}

/// FNV-1a 64-bit over the URL bytes, as 16 lowercase hex chars. Implemented inline (the
/// project keeps deps lean — no hash crate) and pinned by a unit test: cache file names
/// must be STABLE across builds/releases or every upgrade cold-starts the cache.
/// (std's DefaultHasher is explicitly unspecified across releases, so it can't be used.)
pub fn hash_url(url: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in url.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// True when `url` points inside the configured server (`base` = scheme://host[:port],
/// no trailing slash). Prefix-match on `base` + a delimiter, so neither a host that merely
/// starts with ours (`https://jf.example.evil.com`) nor a userinfo trick
/// (`https://jf.example@evil.com/…`) passes.
pub fn url_within_base(url: &str, base: &str) -> bool {
    let base = base.trim_end_matches('/');
    if base.is_empty() {
        return false;
    }
    url == base
        || url
            .strip_prefix(base)
            .is_some_and(|rest| rest.starts_with('/') || rest.starts_with('?'))
}

/// `<hash>.meta` sidecar: the URL it caches (for debugging/`doctor`), the server's
/// validators, and when we last confirmed freshness.
#[derive(Serialize, Deserialize)]
struct Meta {
    url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    etag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_modified: Option<String>,
    fetched_unix: u64,
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn meta_path(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{key}.meta"))
}

fn read_meta(dir: &Path, key: &str) -> Option<Meta> {
    serde_json::from_str(&std::fs::read_to_string(meta_path(dir, key)).ok()?).ok()
}

/// The cached data file for `key`, whichever image extension it landed with.
fn find_data(dir: &Path, key: &str) -> Option<PathBuf> {
    IMAGE_EXTS.iter().map(|e| dir.join(format!("{key}.{e}"))).find(|p| p.exists())
}

/// Bump the data file's mtime so the LRU sweep sees real access recency. Best-effort:
/// a read-only cache filesystem just degrades eviction to fetch order.
fn touch(path: &Path) {
    if let Ok(f) = std::fs::File::options().append(true).open(path) {
        let _ = f.set_modified(std::time::SystemTime::now());
    }
}

/// File extension for the sniffed image type, or None for anything that isn't a real
/// image (servers can 200 an HTML error page; never serve that as art).
fn sniff_ext(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\xFF\xD8\xFF") {
        Some("jpg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// Write data + sidecar atomically. Returns the data path. Rejects non-image bytes.
/// A re-fetch that changed image type (jpg → webp) removes the old-extension file so
/// `find_data` can't resolve to stale bytes.
fn store(dir: &Path, key: &str, bytes: &[u8], meta: &Meta) -> Option<PathBuf> {
    let ext = sniff_ext(bytes)?;
    let path = dir.join(format!("{key}.{ext}"));
    crate::fsutil::write_atomic(&path, bytes).ok()?;
    for other in IMAGE_EXTS.iter().filter(|e| **e != ext) {
        let _ = std::fs::remove_file(dir.join(format!("{key}.{other}")));
    }
    let json = serde_json::to_vec(meta).ok()?;
    let _ = crate::fsutil::write_atomic(&meta_path(dir, key), &json);
    Some(path)
}

/// Effective LRU budget: `[media_server] art_cache_mb` when set (floored so a typo'd
/// tiny value can't thrash-evict the whole cache), else 200 MB.
fn budget_bytes() -> u64 {
    let mb = crate::config::load_or_create().media_server.art_cache_mb;
    let mb = if mb == 0 { DEFAULT_BUDGET_MB } else { mb.clamp(16, 4096) };
    mb * 1024 * 1024
}

/// LRU sweep: while the data files exceed `max_bytes`, evict least-recently-used first
/// (mtime — `touch` on every hit makes that access order, not fetch order), sidecars
/// included. Called after each store, so the just-written file is newest and survives.
fn sweep(dir: &Path, max_bytes: u64) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = rd
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            // Only data files count toward (and are evicted from) the budget; each
            // eviction removes its sidecar alongside. Skips temp siblings too.
            p.extension().and_then(|x| x.to_str()).filter(|x| IMAGE_EXTS.contains(x))?;
            let md = e.metadata().ok()?;
            if !md.is_file() {
                return None;
            }
            Some((md.modified().ok()?, md.len(), p))
        })
        .collect();
    let total: u64 = files.iter().map(|(_, len, _)| len).sum();
    if total <= max_bytes {
        return;
    }
    files.sort_by_key(|(mtime, _, _)| *mtime); // least recently used first
    let mut excess = total - max_bytes;
    for (_, len, path) in files {
        if excess == 0 {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            excess = excess.saturating_sub(len);
            let _ = std::fs::remove_file(path.with_extension("meta"));
        }
    }
}

/// The cached local file for `url`, fetching/revalidating as needed. `auth` is an extra
/// request header (Jellyfin's `X-Emby-Token`); None for unauthenticated art.
///
/// Callers are trusted to have validated `url` (see module comment / `url_within_base`).
pub async fn get(url: &str, auth: Option<(&str, &str)>) -> Option<PathBuf> {
    let dir = cache_dir()?;
    let _ = std::fs::create_dir_all(&dir);
    remove_legacy_dir_once();
    get_in(&dir, url, auth).await
}

/// `get` against an explicit cache dir — the seam the loopback end-to-end test drives
/// without touching the real $XDG_CACHE_HOME (env mutation races parallel tests).
async fn get_in(dir: &Path, url: &str, auth: Option<(&str, &str)>) -> Option<PathBuf> {
    let dir = dir.to_path_buf();
    let key = hash_url(url);
    let data = find_data(&dir, &key);
    let meta = read_meta(&dir, &key);

    if let (Some(data), Some(meta)) = (&data, meta) {
        if now_unix().saturating_sub(meta.fetched_unix) < REVALIDATE_AFTER_SECS {
            touch(data); // pure disk hit — the common cold-boot path, zero network
            return Some(data.clone());
        }
        // Stale: conditional GET. 304 = headers only; errors serve the stale copy.
        return match fetch(url, auth, Some(&meta)).await {
            FetchResult::NotModified => {
                let refreshed = Meta { fetched_unix: now_unix(), ..meta };
                if let Ok(json) = serde_json::to_vec(&refreshed) {
                    let _ = crate::fsutil::write_atomic(&meta_path(&dir, &key), &json);
                }
                touch(data);
                Some(data.clone())
            }
            FetchResult::Fetched(bytes, m) => {
                let stored = store(&dir, &key, &bytes, &m);
                sweep(&dir, budget_bytes());
                stored.or_else(|| Some(data.clone())) // non-image body: keep what we had
            }
            FetchResult::Failed => {
                touch(data);
                Some(data.clone())
            }
        };
    }

    // Miss (or sidecar lost): plain fetch.
    match fetch(url, auth, None).await {
        FetchResult::Fetched(bytes, m) => {
            let stored = store(&dir, &key, &bytes, &m);
            sweep(&dir, budget_bytes());
            stored
        }
        // A 304 without a local copy can't happen (no validators were sent) — treat
        // anything else as a miss the frontend renders its fallback glyph for.
        _ => data,
    }
}

enum FetchResult {
    Fetched(Vec<u8>, Meta),
    NotModified,
    Failed,
}

async fn fetch(url: &str, auth: Option<(&str, &str)>, validators: Option<&Meta>) -> FetchResult {
    let mut req = crate::http::client().get(url);
    if let Some((name, value)) = auth {
        req = req.header(name, value);
    }
    if let Some(m) = validators {
        if let Some(etag) = &m.etag {
            req = req.header("If-None-Match", etag);
        }
        if let Some(lm) = &m.last_modified {
            req = req.header("If-Modified-Since", lm);
        }
    }
    let resp = match req.send().await {
        Ok(r) => r,
        Err(_) => return FetchResult::Failed,
    };
    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        return FetchResult::NotModified;
    }
    if !resp.status().is_success() {
        return FetchResult::Failed;
    }
    let header = |name: &str| {
        resp.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string)
    };
    let meta = Meta {
        url: url.to_string(),
        etag: header("etag"),
        last_modified: header("last-modified"),
        fetched_unix: now_unix(),
    };
    match read_body_capped(resp, MAX_IMAGE_BYTES).await {
        Some(b) => FetchResult::Fetched(b, meta),
        None => FetchResult::Failed,
    }
}

/// Read a response body, enforcing `cap` WHILE streaming: a declared Content-Length over
/// the cap is rejected before the first byte, and an over-long (or lying/chunked) body is
/// abandoned at the moment it crosses the cap — never buffered whole and checked after.
async fn read_body_capped(mut resp: reqwest::Response, cap: u64) -> Option<Vec<u8>> {
    if resp.content_length().is_some_and(|l| l > cap) {
        return None;
    }
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = resp.chunk().await.ok()? {
        if buf.len() as u64 + chunk.len() as u64 > cap {
            return None;
        }
        buf.extend_from_slice(&chunk);
    }
    Some(buf)
}

/// Warm the cache for `urls` in the background with `concurrency` parallel workers.
/// Fire-and-forget: fresh entries cost a stat(), stale ones a conditional GET. Bounded
/// so a big rail can't fire dozens of simultaneous fetches at the server.
pub fn prefetch(urls: Vec<String>, auth: Option<(String, String)>, concurrency: usize) {
    if urls.is_empty() {
        return;
    }
    let next = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let urls = std::sync::Arc::new(urls);
    let auth = std::sync::Arc::new(auth);
    for _ in 0..concurrency.clamp(1, 8).min(urls.len()) {
        let (next, urls, auth) = (next.clone(), urls.clone(), auth.clone());
        tauri::async_runtime::spawn(async move {
            loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(url) = urls.get(i) else { break };
                let hdr = auth.as_ref().as_ref().map(|(n, v)| (n.as_str(), v.as_str()));
                let _ = get(url, hdr).await;
            }
        });
    }
}

/// (entries, bytes) of cached artwork, for `omnideck doctor`.
pub struct CacheStats {
    pub dir: Option<PathBuf>,
    pub entries: u64,
    pub bytes: u64,
    pub budget_bytes: u64,
}

pub fn stats() -> CacheStats {
    let dir = cache_dir();
    let (mut entries, mut bytes) = (0u64, 0u64);
    if let Some(d) = &dir {
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.flatten() {
                let p = e.path();
                let is_data =
                    p.extension().and_then(|x| x.to_str()).is_some_and(|x| IMAGE_EXTS.contains(&x));
                if !is_data {
                    continue;
                }
                if let Ok(md) = e.metadata() {
                    if md.is_file() {
                        entries += 1;
                        bytes += md.len();
                    }
                }
            }
        }
    }
    CacheStats { dir, entries, bytes, budget_bytes: budget_bytes() }
}

/// Delete the whole cache (`omnideck doctor --clear-art-cache`). Everything here is
/// regenerable from the server on the next boot.
pub fn clear() -> std::io::Result<()> {
    if let Some(dir) = cache_dir() {
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
    }
    if let Some(legacy) = legacy_dir() {
        if legacy.exists() {
            std::fs::remove_dir_all(&legacy)?;
        }
    }
    Ok(())
}

/// The pre-artwork_cache poster cache (id-keyed files under omnideck/media, no
/// validators/sidecars). Nothing reads it anymore; leave it and every upgrade carries up
/// to 100 MB of dead art forever.
fn legacy_dir() -> Option<PathBuf> {
    cache_dir().map(|d| d.with_file_name("media"))
}

fn remove_legacy_dir_once() {
    static ONCE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        if let Some(legacy) = legacy_dir() {
            if legacy.exists() {
                let _ = std::fs::remove_dir_all(&legacy);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("omnideck-artcache-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn hash_is_stable_across_builds() {
        // Pinned vectors: if this test ever fails, cache file names changed and every
        // user's cache cold-starts on upgrade. Don't "fix" the constants — fix the hash.
        assert_eq!(hash_url(""), "cbf29ce484222325");
        assert_eq!(hash_url("a"), "af63dc4c8601ec8c");
        assert_eq!(
            hash_url("http://jf.local:8096/Items/abc123/Images/Primary?maxWidth=480&quality=90"),
            hash_url("http://jf.local:8096/Items/abc123/Images/Primary?maxWidth=480&quality=90"),
        );
        assert_ne!(hash_url("http://jf/a"), hash_url("http://jf/b"));
        assert_eq!(hash_url("x").len(), 16);
    }

    #[test]
    fn url_within_base_rejects_lookalikes() {
        let base = "http://jf.local:8096";
        assert!(url_within_base("http://jf.local:8096/Items/1/Images/Primary", base));
        assert!(url_within_base("http://jf.local:8096?x=1", base));
        assert!(url_within_base("http://jf.local:8096", base));
        assert!(url_within_base("http://jf.local:8096/x", "http://jf.local:8096/")); // trailing slash in config
        assert!(!url_within_base("http://jf.local:80960/Items/1", base)); // port prefix trick
        assert!(!url_within_base("http://jf.local.evil.com/Items/1", "http://jf.local")); // host suffix trick
        assert!(!url_within_base("http://jf.local:8096@evil.com/", base)); // userinfo trick
        assert!(!url_within_base("http://evil.com/http://jf.local:8096/", base));
        assert!(!url_within_base("http://jf.local:8096/x", "")); // unconfigured = nothing allowed
    }

    #[test]
    fn store_writes_data_and_sidecar_atomically() {
        let dir = scratch("store");
        let meta = Meta {
            url: "http://jf/Items/1/Images/Primary".into(),
            etag: Some("\"abc\"".into()),
            last_modified: None,
            fetched_unix: 12345,
        };
        // JPEG magic → .jpg file; sidecar parses back; no temp droppings.
        let p = store(&dir, "00000000deadbeef", b"\xFF\xD8\xFFrest-of-jpeg", &meta).unwrap();
        assert_eq!(p, dir.join("00000000deadbeef.jpg"));
        assert!(p.exists());
        let m = read_meta(&dir, "00000000deadbeef").unwrap();
        assert_eq!(m.etag.as_deref(), Some("\"abc\""));
        assert_eq!(m.fetched_unix, 12345);
        let names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 2, "exactly data + sidecar, no temp files: {names:?}");

        // Type change on re-fetch: webp replaces the jpg so find_data can't go stale.
        let p2 = store(&dir, "00000000deadbeef", b"RIFF\x00\x00\x00\x00WEBPVP8 ", &meta).unwrap();
        assert_eq!(p2, dir.join("00000000deadbeef.webp"));
        assert!(!dir.join("00000000deadbeef.jpg").exists());
        assert_eq!(find_data(&dir, "00000000deadbeef").unwrap(), p2);

        // Non-image bodies (server error pages) are never cached.
        assert!(store(&dir, "1111111111111111", b"<html>502</html>", &meta).is_none());
        assert!(find_data(&dir, "1111111111111111").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sweep_evicts_least_recently_used_with_sidecars() {
        let dir = scratch("sweep");
        let day = std::time::Duration::from_secs(86_400);
        let now = std::time::SystemTime::now();
        // Three 10-byte entries, staged mtimes: c (oldest access) < a < b (newest).
        for (key, age_days) in [("aaaa", 1u64), ("bbbb", 0), ("cccc", 3)] {
            let data = dir.join(format!("{key}.jpg"));
            std::fs::write(&data, b"0123456789").unwrap();
            std::fs::write(dir.join(format!("{key}.meta")), b"{}").unwrap();
            let f = std::fs::File::options().append(true).open(&data).unwrap();
            f.set_modified(now - day * (age_days as u32)).unwrap();
        }
        sweep(&dir, 20); // 30 bytes of data > 20: exactly the LRU entry (cccc) must go
        assert!(!dir.join("cccc.jpg").exists(), "least-recently-used data evicted");
        assert!(!dir.join("cccc.meta").exists(), "sidecar evicted alongside");
        assert!(dir.join("aaaa.jpg").exists());
        assert!(dir.join("bbbb.jpg").exists());

        sweep(&dir, 20); // now under budget: no-op
        assert!(dir.join("aaaa.jpg").exists() && dir.join("bbbb.jpg").exists());

        // Meta files never count toward the budget and are never evicted on their own.
        sweep(&dir, 15); // 20 bytes of data > 15: aaaa (older than bbbb) goes
        assert!(!dir.join("aaaa.jpg").exists() && !dir.join("aaaa.meta").exists());
        assert!(dir.join("bbbb.jpg").exists() && dir.join("bbbb.meta").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn touch_makes_hits_survive_the_sweep() {
        let dir = scratch("touch");
        let day = std::time::Duration::from_secs(86_400);
        let now = std::time::SystemTime::now();
        for key in ["old1", "old2"] {
            let data = dir.join(format!("{key}.jpg"));
            std::fs::write(&data, b"0123456789").unwrap();
            let f = std::fs::File::options().append(true).open(&data).unwrap();
            f.set_modified(now - day * 5).unwrap();
        }
        // A hit on old1 (the touch() path) must re-rank it above old2.
        touch(&dir.join("old1.jpg"));
        sweep(&dir, 10);
        assert!(dir.join("old1.jpg").exists(), "recently-served art survives");
        assert!(!dir.join("old2.jpg").exists(), "untouched art is the eviction victim");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn end_to_end_fetch_hit_and_304_revalidate_over_loopback() {
        use std::io::{Read, Write};
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Minimal HTTP/1.1 image server on a loopback ephemeral port: 200 + ETag on a
        // plain GET, 304 when the request carries our validator. Counts requests so the
        // assertions below can prove which paths hit the network.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let hits = std::sync::Arc::new(AtomicUsize::new(0));
        let server_hits = hits.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { break };
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_ascii_lowercase();
                server_hits.fetch_add(1, Ordering::SeqCst);
                let resp: Vec<u8> = if req.contains("if-none-match: \"v1\"") {
                    b"HTTP/1.1 304 Not Modified\r\netag: \"v1\"\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_vec()
                } else {
                    let body = b"\xFF\xD8\xFFjpeg-body";
                    let mut r = format!(
                        "HTTP/1.1 200 OK\r\netag: \"v1\"\r\nlast-modified: Mon, 01 Jan 2024 00:00:00 GMT\r\ncontent-type: image/jpeg\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                        body.len()
                    )
                    .into_bytes();
                    r.extend_from_slice(body);
                    r
                };
                let _ = s.write_all(&resp);
            }
        });

        let dir = scratch("e2e");
        let url = format!("http://127.0.0.1:{port}/Items/1/Images/Primary");
        let key = hash_url(&url);

        // 1. Miss → network fetch → cached with validators.
        let p = tauri::async_runtime::block_on(get_in(&dir, &url, None)).unwrap();
        assert_eq!(p, dir.join(format!("{key}.jpg")));
        assert_eq!(std::fs::read(&p).unwrap(), b"\xFF\xD8\xFFjpeg-body");
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        let m = read_meta(&dir, &key).unwrap();
        assert_eq!(m.etag.as_deref(), Some("\"v1\""));
        assert!(m.last_modified.is_some());

        // 2. Fresh hit → served from disk, ZERO network (the cold-boot rail path).
        let p2 = tauri::async_runtime::block_on(get_in(&dir, &url, None)).unwrap();
        assert_eq!(p2, p);
        assert_eq!(hits.load(Ordering::SeqCst), 1, "fresh hit must not touch the network");

        // 3. Stale (backdate fetched_unix) → conditional GET → 304 → same file served,
        //    freshness clock reset.
        let backdated = Meta { fetched_unix: 1, ..m };
        std::fs::write(meta_path(&dir, &key), serde_json::to_vec(&backdated).unwrap()).unwrap();
        let p3 = tauri::async_runtime::block_on(get_in(&dir, &url, None)).unwrap();
        assert_eq!(p3, p);
        assert_eq!(hits.load(Ordering::SeqCst), 2, "stale hit revalidates once");
        assert!(read_meta(&dir, &key).unwrap().fetched_unix > 1, "304 must reset freshness");

        // 4. …and the reset means the very next lookup is a pure disk hit again.
        let p4 = tauri::async_runtime::block_on(get_in(&dir, &url, None)).unwrap();
        assert_eq!(p4, p);
        assert_eq!(hits.load(Ordering::SeqCst), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn body_cap_is_enforced_while_streaming() {
        use std::io::{Read, Write};

        // Loopback server sending a 20-byte body with an honest Content-Length.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { break };
                let mut buf = [0u8; 2048];
                let _ = s.read(&mut buf);
                let body = b"\xFF\xD8\xFF17-more-jpeg-byte";
                let mut r = format!(
                    "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                )
                .into_bytes();
                r.extend_from_slice(body);
                let _ = s.write_all(&r);
            }
        });

        let url = format!("http://127.0.0.1:{port}/art");
        let fetch_with_cap = |cap: u64| {
            tauri::async_runtime::block_on(async {
                let resp = crate::http::client().get(&url).send().await.unwrap();
                read_body_capped(resp, cap).await
            })
        };
        // Over the cap → rejected up front off the declared length, nothing buffered.
        assert_eq!(fetch_with_cap(10), None);
        // At/over the body size → full body comes through.
        assert_eq!(fetch_with_cap(20).as_deref(), Some(&b"\xFF\xD8\xFF17-more-jpeg-byte"[..]));
        assert_eq!(fetch_with_cap(1024).map(|b| b.len()), Some(20));
    }

    #[test]
    fn sniff_rejects_non_images() {
        assert_eq!(sniff_ext(b"\xFF\xD8\xFFxx"), Some("jpg"));
        assert_eq!(sniff_ext(b"\x89PNG\r\n\x1a\nxx"), Some("png"));
        assert_eq!(sniff_ext(b"RIFF\x00\x00\x00\x00WEBP"), Some("webp"));
        assert_eq!(sniff_ext(b"<html>nope</html>"), None);
        assert_eq!(sniff_ext(b""), None);
    }
}
