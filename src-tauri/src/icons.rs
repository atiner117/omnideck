// OmniDeck — site-icon (favicon) fetcher for browser/web app tiles.
// Streaming/web tiles (Netflix, Spotify, …) ship with an emoji placeholder; this can fill
// in the real brand icon at runtime from DuckDuckGo's favicon service (normalized PNGs),
// falling back to the site's own /favicon.ico. Results are cached on disk so each domain is
// fetched at most once. We never bundle trademarked logos — icons are fetched on the user's
// machine on demand.
//
// Privacy: fetching tells DuckDuckGo (and the site) which web tiles the user has, so it is
// OPT-IN — `settings.fetch_remote_icons`, default false. With it off, only icons already in
// the on-disk cache are shown and no request (not even the DNS lookup behind the SSRF check)
// is made. There is deliberately no Google s2 fallback any more.
use crate::http::is_blocked_host_resolved;
use std::path::{Path, PathBuf};

fn cache_dir() -> Option<PathBuf> {
    // XDG: prefer $XDG_CACHE_HOME (when absolute), else ~/.cache (unchanged for existing installs).
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("omnideck/icons"))
}

/// Pull the host out of a URL (handles a leading `--app=` from our browser-PWA execs).
pub fn domain_of(url: &str) -> Option<String> {
    let s = url.strip_prefix("--app=").unwrap_or(url);
    let s = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s);
    let host = s.split('/').next()?.split('?').next()?;
    if host.is_empty() || !host.contains('.') {
        None
    } else {
        Some(host.to_string())
    }
}

/// last-two-labels fallback (open.spotify.com -> spotify.com, listen.tidal.com -> tidal.com).
/// Naive on multi-part TLDs (.co.uk) but fine for the mainstream services we ship.
fn root_domain(host: &str) -> Option<String> {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() > 2 {
        Some(parts[parts.len() - 2..].join("."))
    } else {
        None
    }
}

/// What the cache alone can answer for a tile URL — computing it touches no network.
#[derive(Debug, PartialEq)]
enum Local {
    /// No usable domain (bare binary, `localhost`, …): nothing to fetch.
    NoDomain,
    /// Already cached: the data URL.
    Hit(String),
    /// Not cached: the host to look up and the cache file to write it to.
    Miss { host: String, path: PathBuf },
}

fn local_lookup(dir: &Path, url: &str) -> Local {
    let Some(host) = domain_of(url) else { return Local::NoDomain };
    let safe = host.replace(['/', ':'], "_");
    let path = dir.join(format!("{safe}.img"));
    if path.exists() {
        if let Some(d) = to_data_url(&path) {
            return Local::Hit(d);
        }
    }
    Local::Miss { host, path }
}

/// Remote sources to try for a host, in order: DuckDuckGo, then the site's own
/// /favicon.ico (recovers real logos DDG lacks, e.g. VLC's cone); host first, then root.
/// Only the site itself and DuckDuckGo ever see the domain — no Google s2 fallback.
fn candidate_urls(host: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    for d in [Some(host.to_string()), root_domain(host)].into_iter().flatten() {
        candidates.push(ddg_url(&d));
        candidates.push(format!("https://{d}/favicon.ico"));
    }
    candidates
}

/// Fetch a site icon (cached) as a data URL, or None on no-domain / network error / no
/// real image. `allow_network` is `settings.fetch_remote_icons`: when false only the
/// on-disk cache is consulted. DuckDuckGo sometimes returns junk for a subdomain, so we
/// also try the root.
pub async fn favicon(url: &str, allow_network: bool) -> Option<String> {
    favicon_in(&cache_dir()?, url, allow_network).await
}

async fn favicon_in(dir: &Path, url: &str, allow_network: bool) -> Option<String> {
    let (host, path) = match local_lookup(dir, url) {
        Local::Hit(d) => return Some(d),
        Local::NoDomain => return None,
        Local::Miss { host, path } => (host, path),
    };
    if !allow_network {
        return None; // opt-in only: no request, no DNS lookup
    }
    if is_blocked_host_resolved(&host) {
        // Don't probe internal/loopback services from a crafted tile URL (SSRF) — including
        // a public-looking domain that RESOLVES to an internal address (DNS rebinding).
        return None;
    }
    let _ = std::fs::create_dir_all(dir);
    let mut bytes = None;
    for url in candidate_urls(&host) {
        if let Some(b) = fetch_image(&url).await {
            bytes = Some(b);
            break;
        }
    }
    let bytes = bytes?;
    std::fs::write(&path, &bytes).ok()?;
    to_data_url(&path)
}

fn ddg_url(domain: &str) -> String {
    format!("https://icons.duckduckgo.com/ip3/{domain}.ico")
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

/// GET an icon URL; Some(bytes) only if the body is a recognized image that isn't a known
/// placeholder. Tiny 16x16 favicons are caught by too_small, and DuckDuckGo serves a fixed
/// 1478-byte 48x48 generic for domains it doesn't have — rejected here so unknown apps fall
/// through to their real favicon or the emoji instead.
async fn fetch_image(url: &str) -> Option<Vec<u8>> {
    let bytes = fetch_capped(url, 4 * 1024 * 1024).await?; // favicons are KB; 4 MiB is plenty
    let is_ddg = url.contains("icons.duckduckgo.com");
    if is_ddg && bytes.len() == 1478 {
        return None; // DuckDuckGo's generic "unknown domain" fallback
    }
    if bytes.len() >= 100 && sniff(&bytes).is_some() && !too_small(&bytes) {
        Some(bytes)
    } else {
        None
    }
}

/// True if a PNG/ICO is <24px on a side (placeholder-sized). JPEG/GIF/SVG always pass.
fn too_small(b: &[u8]) -> bool {
    if b.len() >= 24 && b.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        let w = u32::from_be_bytes([b[16], b[17], b[18], b[19]]);
        let h = u32::from_be_bytes([b[20], b[21], b[22], b[23]]);
        return w < 24 || h < 24;
    }
    if b.len() >= 8 && b.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        let w = if b[6] == 0 { 256 } else { b[6] as u32 };
        let h = if b[7] == 0 { 256 } else { b[7] as u32 };
        return w < 24 || h < 24;
    }
    false
}

/// Recognized image content type, or None if the bytes aren't an image we can show.
fn sniff(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        Some("image/png")
    } else if b.starts_with(&[0xFF, 0xD8]) {
        Some("image/jpeg")
    } else if b.starts_with(b"GIF8") {
        Some("image/gif")
    } else if b.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        Some("image/x-icon")
    } else if b.starts_with(b"<svg") || b.starts_with(b"<?xml") {
        Some("image/svg+xml")
    } else {
        None
    }
}

fn to_data_url(p: &Path) -> Option<String> {
    use base64::Engine;
    let bytes = std::fs::read(p).ok()?;
    let mime = sniff(&bytes)?;
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[cfg(test)]
mod tests {
    use super::{candidate_urls, domain_of, favicon_in, local_lookup, Local};
    use crate::http::is_blocked_host;

    fn temp_cache(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("omnideck-icontest-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A 32x32 PNG header — enough for sniff/too_small/to_data_url (they read the IHDR only).
    fn png_32() -> Vec<u8> {
        let mut b = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, b'I', b'H', b'D', b'R'];
        b.extend_from_slice(&32u32.to_be_bytes());
        b.extend_from_slice(&32u32.to_be_bytes());
        b.resize(128, 0);
        b
    }

    #[test]
    fn candidates_never_include_google() {
        let c = candidate_urls("open.spotify.com");
        assert_eq!(
            c,
            vec![
                "https://icons.duckduckgo.com/ip3/open.spotify.com.ico",
                "https://open.spotify.com/favicon.ico",
                "https://icons.duckduckgo.com/ip3/spotify.com.ico",
                "https://spotify.com/favicon.ico",
            ]
        );
        assert!(c.iter().all(|u| !u.contains("google")), "{c:?}");
    }

    #[test]
    fn local_lookup_serves_cache_and_reports_misses() {
        let dir = temp_cache("lookup");
        assert_eq!(local_lookup(&dir, "brave"), Local::NoDomain);
        match local_lookup(&dir, "--app=https://www.netflix.com") {
            Local::Miss { host, path } => {
                assert_eq!(host, "www.netflix.com");
                assert_eq!(path, dir.join("www.netflix.com.img"));
            }
            other => panic!("expected a miss, got {other:?}"),
        }
        std::fs::write(dir.join("www.netflix.com.img"), png_32()).unwrap();
        match local_lookup(&dir, "--app=https://www.netflix.com") {
            Local::Hit(d) => assert!(d.starts_with("data:image/png;base64,"), "{d}"),
            other => panic!("expected a hit, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn network_disabled_uses_cache_only() {
        let dir = temp_cache("gate");
        // Uncached + network off: None without any request (a domain that cannot resolve,
        // so a broken gate fails here rather than silently succeeding).
        let miss = tauri::async_runtime::block_on(favicon_in(&dir, "https://omnideck-test.invalid", false));
        assert_eq!(miss, None);
        assert!(std::fs::read_dir(&dir).unwrap().next().is_none(), "nothing may be written");
        // Cached + network off: the cached icon is still shown.
        std::fs::write(dir.join("www.netflix.com.img"), png_32()).unwrap();
        let hit = tauri::async_runtime::block_on(favicon_in(&dir, "https://www.netflix.com/", false));
        assert!(hit.is_some_and(|d| d.starts_with("data:image/png;base64,")));
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn extracts_domain() {
        assert_eq!(domain_of("--app=https://www.netflix.com").as_deref(), Some("www.netflix.com"));
        assert_eq!(domain_of("https://open.spotify.com/").as_deref(), Some("open.spotify.com"));
        assert_eq!(domain_of("https://app.plex.tv/desktop?foo=1").as_deref(), Some("app.plex.tv"));
        assert_eq!(domain_of("brave"), None); // bare binary, no domain
        assert_eq!(domain_of("--app=https://localhost"), None); // no dot
    }

    #[test]
    fn blocks_private_and_loopback_hosts() {
        assert!(is_blocked_host("169.254.169.254")); // cloud metadata
        assert!(is_blocked_host("127.0.0.1"));
        assert!(is_blocked_host("10.0.0.5:8080")); // with port
        assert!(is_blocked_host("192.168.1.1"));
        assert!(is_blocked_host("172.16.4.2"));
        assert!(is_blocked_host("0.0.0.0"));
        assert!(is_blocked_host("localhost"));
        assert!(!is_blocked_host("spotify.com"));
        assert!(!is_blocked_host("icons.duckduckgo.com"));
        assert!(!is_blocked_host("8.8.8.8")); // public IP is fine
    }
}
