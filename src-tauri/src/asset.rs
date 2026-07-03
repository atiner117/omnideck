// OmniDeck — `omnideck://` asset protocol.
//
// Serves on-disk image files to the webview as plain URLs instead of base64 `data:` URLs pinned
// in reactive state. A 600x900 capsule is ~2.67x its file size once base64'd + held as UTF-16 in
// the webview heap, and it's re-diffed on every navigation — hundreds of MB on a large library.
// Under this scheme the bytes stay on disk and the webview holds a URL string (decoded to GPU on
// paint). See NOTES-PERFORMANCE.md / NOTES-RESEARCH.md §2.
//
// Security: one chokepoint (`resolve_and_read`) — canonicalize the path (resolves `..`/symlinks),
// require it under an allowlisted root, require an image extension, cap the size. Anything else 404s.
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const MAX_BYTES: u64 = 32 * 1024 * 1024;

/// Canonicalized directories art may legitimately come from: Steam's librarycache (local capsule/
/// hero art) and our own SteamGridDB art cache. Computed once. Favicons + the custom background
/// image stay on `data:` for now, so the icon cache is intentionally NOT a root yet.
fn roots() -> &'static Vec<PathBuf> {
    static ROOTS: OnceLock<Vec<PathBuf>> = OnceLock::new();
    ROOTS.get_or_init(|| {
        let mut v = Vec::new();
        if let Some(steam) = crate::library::steam_root() {
            if let Ok(p) = Path::new(&steam).join("appcache/librarycache").canonicalize() {
                v.push(p);
            }
        }
        if let Some(art) = crate::steamgriddb::cache_dir() {
            let _ = std::fs::create_dir_all(&art); // so canonicalize() succeeds before the first fetch
            if let Ok(p) = art.canonicalize() {
                v.push(p);
            }
        }
        v
    })
}

/// Image MIME from the extension, or None for anything we won't serve.
fn mime_for(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => Some("image/png"),
        Some("webp") => Some("image/webp"),
        Some("jpg") | Some("jpeg") => Some("image/jpeg"),
        _ => None,
    }
}

/// Read the requested file iff it's an allowed image under an allowlisted root. (bytes, mime).
fn resolve_and_read(raw_path: &str) -> Option<(Vec<u8>, &'static str)> {
    let decoded = percent_decode(raw_path);
    let canonical = Path::new(&decoded).canonicalize().ok()?; // resolves `..` + symlinks
    if !roots().iter().any(|root| canonical.starts_with(root)) {
        return None;
    }
    let mime = mime_for(&canonical)?;
    if std::fs::metadata(&canonical).ok()?.len() > MAX_BYTES {
        return None;
    }
    Some((std::fs::read(&canonical).ok()?, mime))
}

/// Build the HTTP response for an `omnideck://` request: 200 with the image, or 404.
pub fn respond(uri_path: &str) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::Response;
    match resolve_and_read(uri_path) {
        Some((bytes, mime)) => Response::builder()
            .header("content-type", mime)
            .header("cache-control", "max-age=86400")
            .body(bytes)
            .unwrap_or_else(|_| Response::new(Vec::new())),
        None => Response::builder()
            .status(404)
            .body(b"not found".to_vec())
            .unwrap_or_else(|_| Response::new(Vec::new())),
    }
}

/// Minimal percent-decoder (the project keeps deps lean — no `percent-encoding` crate). Decodes
/// `%XX` byte escapes; leaves a malformed/truncated escape as the literal `%`.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex_val(b[i + 1]), hex_val(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decode_handles_escapes() {
        assert_eq!(percent_decode("/a/b%20c.jpg"), "/a/b c.jpg");
        assert_eq!(percent_decode("/x/%2e%2e/y"), "/x/../y"); // traversal is decoded, then canonicalize defeats it
        assert_eq!(percent_decode("/plain.png"), "/plain.png");
        assert_eq!(percent_decode("trailing%2"), "trailing%2"); // truncated escape stays literal
    }

    #[test]
    fn mime_only_for_images() {
        assert_eq!(mime_for(Path::new("/a/x.PNG")), Some("image/png"));
        assert_eq!(mime_for(Path::new("/a/x.jpeg")), Some("image/jpeg"));
        assert_eq!(mime_for(Path::new("/a/x.webp")), Some("image/webp"));
        assert_eq!(mime_for(Path::new("/a/x.gif")), None);
        assert_eq!(mime_for(Path::new("/etc/passwd")), None);
    }
}
