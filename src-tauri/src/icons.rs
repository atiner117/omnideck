// OmniDeck — site-icon (favicon) fetcher for browser/web app tiles.
// Streaming/web tiles (Netflix, Spotify, …) ship with an emoji placeholder; this fills
// in the real brand icon at runtime from DuckDuckGo's favicon service (privacy-friendly,
// normalized PNGs). Results are cached on disk so each domain is fetched at most once.
// We never bundle trademarked logos — icons are fetched on the user's machine on demand.
use std::path::{Path, PathBuf};

fn cache_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".cache/omnideck/icons"))
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

/// Fetch a site icon (cached) as a data URL, or None on no-domain / network error / no
/// real image. DuckDuckGo sometimes returns junk for a subdomain, so we also try the root.
pub async fn favicon(url: &str) -> Option<String> {
    let host = domain_of(url)?;
    let dir = cache_dir()?;
    let _ = std::fs::create_dir_all(&dir);
    let safe = host.replace(['/', ':'], "_");
    let path = dir.join(format!("{safe}.img"));
    if path.exists() {
        return to_data_url(&path);
    }
    // Try, per domain: DDG → the site's own /favicon.ico → Google s2; host first, then root.
    // DDG is privacy-friendly; the site's own favicon recovers real logos the services lack
    // (e.g. VLC's cone); Google s2 is the normalized last resort.
    let mut candidates: Vec<String> = Vec::new();
    for d in [Some(host.clone()), root_domain(&host)].into_iter().flatten() {
        candidates.push(ddg_url(&d));
        candidates.push(format!("https://{d}/favicon.ico"));
        candidates.push(google_url(&d));
    }
    let mut bytes = None;
    for url in candidates {
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
fn google_url(domain: &str) -> String {
    format!("https://www.google.com/s2/favicons?domain={domain}&sz=128")
}

/// GET an icon URL; Some(bytes) only if the body is a recognized image that isn't a known
/// placeholder. Google serves a 16x16 globe for unknown domains (caught by too_small), and
/// DuckDuckGo serves a fixed 1478-byte 48x48 generic for domains it doesn't have — rejected
/// here so unknown apps fall through to their real favicon or the emoji instead.
async fn fetch_image(url: &str) -> Option<Vec<u8>> {
    let bytes = reqwest::get(url).await.ok()?.bytes().await.ok()?;
    let is_ddg = url.contains("icons.duckduckgo.com");
    if is_ddg && bytes.len() == 1478 {
        return None; // DuckDuckGo's generic "unknown domain" fallback
    }
    if bytes.len() >= 100 && sniff(&bytes).is_some() && !too_small(&bytes) {
        Some(bytes.to_vec())
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
    use super::domain_of;
    #[test]
    fn extracts_domain() {
        assert_eq!(domain_of("--app=https://www.netflix.com").as_deref(), Some("www.netflix.com"));
        assert_eq!(domain_of("https://open.spotify.com/").as_deref(), Some("open.spotify.com"));
        assert_eq!(domain_of("https://app.plex.tv/desktop?foo=1").as_deref(), Some("app.plex.tv"));
        assert_eq!(domain_of("brave"), None); // bare binary, no domain
        assert_eq!(domain_of("--app=https://localhost"), None); // no dot
    }
}
