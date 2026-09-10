// OmniDeck — `omnideck://` asset protocol.
//
// Serves on-disk image files to the webview as plain URLs instead of base64 `data:` URLs pinned
// in reactive state. A 600x900 capsule is ~2.67x its file size once base64'd + held as UTF-16 in
// the webview heap, and it's re-diffed on every navigation — hundreds of MB on a large library.
// Under this scheme the bytes stay on disk and the webview holds a URL string (decoded to GPU on
// paint). See NOTES-PERFORMANCE.md / NOTES-RESEARCH.md §2.
//
// Security: one chokepoint (`resolve_and_read`) — open the file once, then require *that
// descriptor* to be a regular file under an allowlisted root with an image extension, under the
// size cap. Anything else 404s. The checks deliberately read the open handle rather than the
// path, so nothing can be swapped underneath them between a check and the read.
use std::io::Read;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const MAX_BYTES: u64 = 32 * 1024 * 1024;

/// Canonicalized directories art may legitimately come from: Steam's librarycache (local capsule/
/// hero art), our own SteamGridDB art cache, the artwork cache (media posters, artwork_cache.rs),
/// and the downscaled-wallpaper cache (background.rs). Computed once. Favicons still stay on
/// `data:`, so the icon cache is intentionally NOT a root yet.
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
        if let Some(artwork) = crate::artwork_cache::cache_dir() {
            let _ = std::fs::create_dir_all(&artwork);
            if let Ok(p) = artwork.canonicalize() {
                v.push(p);
            }
        }
        // Downscaled custom wallpapers (bg_image → background::prepared): served over
        // `omnideck://` like the other art, so its cache dir must be an allowlisted root too.
        if let Some(bg) = crate::background::cache_dir() {
            let _ = std::fs::create_dir_all(&bg);
            if let Ok(p) = bg.canonicalize() {
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

/// The path the kernel resolved an open descriptor to. `readlink("/proc/self/fd/N")` returns the
/// fully-resolved absolute path of the inode this handle points at — `..` and symlinks already
/// collapsed — so it answers "what did I *actually* open" rather than "what would this path
/// resolve to if I looked again". Linux-only, which is the only target (`proc.rs`/`switcher.rs`
/// already read `/proc` directly). An unlinked file reads back as `"<path> (deleted)"`, which
/// fails the extension check below — the safe direction.
fn fd_path(f: &std::fs::File) -> Option<PathBuf> {
    std::fs::read_link(format!("/proc/self/fd/{}", f.as_raw_fd())).ok()
}

/// Read the requested file iff it's an allowed image under an allowlisted root. (bytes, mime).
///
/// Single resolution on purpose. This used to `canonicalize()`, then `metadata()`, then `read()`
/// the path — three independent trips through user-writable cache dirs, so the inode that passed
/// the root/extension/size checks was not necessarily the inode whose bytes got served. Now the
/// open happens first and every check interrogates that descriptor: `fd_path` for the root and
/// extension gates, `fstat` on the handle for the type and size, and the bytes come off the same
/// fd. Swapping the path afterwards changes nothing we already hold.
fn resolve_and_read(raw_path: &str) -> Option<(Vec<u8>, &'static str)> {
    let decoded = percent_decode(raw_path);
    let mut f = std::fs::File::open(&decoded).ok()?;

    let canonical = fd_path(&f)?;
    if !roots().iter().any(|root| canonical.starts_with(root)) {
        return None;
    }
    let mime = mime_for(&canonical)?;

    let meta = f.metadata().ok()?; // fstat(2) on the handle, not a fourth path lookup
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return None;
    }

    // Cap the read itself rather than trusting the stat'd length: a file being appended to
    // between the fstat and the read must not grow past the cap in our buffer.
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    f.by_ref().take(MAX_BYTES).read_to_end(&mut bytes).ok()?;
    Some((bytes, mime))
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

    /// The root/extension gates run on `fd_path`, so what they see has to be the inode actually
    /// opened — not the path the caller handed us. Open through a symlink and confirm the handle
    /// reports the target: that is what makes the check unswappable after the fact.
    #[test]
    fn fd_path_reports_the_opened_inode_not_the_request_path() {
        let dir = std::env::temp_dir().join(format!("omnideck-fdpath-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let real = dir.join("real.png");
        std::fs::write(&real, b"\x89PNG\r\n\x1a\n").unwrap();
        let link = dir.join("link.png");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let want = real.canonicalize().unwrap();
        assert_eq!(fd_path(&std::fs::File::open(&real).unwrap()).unwrap(), want);
        assert_eq!(fd_path(&std::fs::File::open(&link).unwrap()).unwrap(), want);

        // A traversing request resolves the same way, so `starts_with(root)` can't be fooled by
        // `..` segments that the old canonicalize-then-reopen dance re-resolved separately.
        let dotted = dir.join("sub/../real.png");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        assert_eq!(fd_path(&std::fs::File::open(&dotted).unwrap()).unwrap(), want);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A request that resolves outside every allowlisted root is a 404, not a read. `roots()` is
    /// built from real cache dirs, so `/etc/hostname` is the stable "definitely not a root" case —
    /// and it also exercises the extension gate.
    #[test]
    fn resolve_and_read_refuses_paths_outside_the_roots() {
        assert!(resolve_and_read("/etc/hostname").is_none());
        assert!(resolve_and_read("/etc/../etc/hostname").is_none());
        assert!(resolve_and_read("/nonexistent-omnideck-asset.png").is_none());
        assert_eq!(respond("/etc/hostname").status(), 404);
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
