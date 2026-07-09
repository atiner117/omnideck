// OmniDeck — custom wallpaper downscaler.
//
// A user's background can be a full-resolution photo (the couch-test host had a 4000x3000,
// 3.9 MB JPEG). Loaded the old way — `get_art` → a base64 `data:` URL — the webview parsed
// a ~5 MB string and decoded ~48 MB of pixels on the main thread at startup, which is what
// dropped the dashboard to 12-18 fps until it settled. This prepares a display-sized copy
// ONCE, caches it, and serves it over `omnideck://` (bytes stay on disk, the webview holds a
// short URL) — the same win the Steam/media art already gets.
use std::path::{Path, PathBuf};

/// Where downscaled wallpapers are cached (an allowlisted `omnideck://` root — see asset.rs).
/// ~/.cache/omnideck/bg (XDG_CACHE_HOME aware), matching the other on-disk art caches.
pub fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("omnideck/bg"))
}

/// Longest edge we downscale a wallpaper to. The output must still COVER the panel
/// (CSS `background-size: cover`), so we scale by the larger of the width/height ratios;
/// this is the display's larger dimension with headroom for a 4K panel.
const MAX_EDGE: u32 = 2560;

/// Downscaled, cached copy of `src` sized to roughly the display, as an on-disk path the
/// frontend wraps in `omnideck://` (via its `artUrl`). Returns None on any failure — the
/// caller then falls back to the original data-URL path, so behavior is never worse.
pub fn prepared(src: &str, display: Option<(u32, u32)>) -> Option<PathBuf> {
    let src_path = Path::new(src);
    let meta = std::fs::metadata(src_path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let (dw, dh) = display.unwrap_or((2560, 1440));
    // Cover target, capped so a huge panel can't ask for an upscale-to-4K re-encode.
    let target_w = dw.clamp(1280, MAX_EDGE);
    let target_h = dh.clamp(720, MAX_EDGE);

    let dir = cache_dir()?;
    std::fs::create_dir_all(&dir).ok()?;
    // Cache key ties the output to the source path, its mtime, and the target size, so a
    // changed photo (or a resolution change) re-renders but a repeat launch is a no-op.
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let key = format!("{src}|{mtime}|{target_w}x{target_h}");
    let out = dir.join(format!("{:016x}.jpg", fnv1a(&key)));
    if out.exists() {
        return Some(out);
    }

    let img = image::ImageReader::open(src_path).ok()?.with_guessed_format().ok()?.decode().ok()?;
    let (iw, ih) = (img.width(), img.height());
    if iw == 0 || ih == 0 {
        return None;
    }
    // Cover scale — never upscale (a small source is left as-is, just re-encoded small).
    let scale = (target_w as f32 / iw as f32).max(target_h as f32 / ih as f32).min(1.0);
    let prepared = if scale < 1.0 {
        let (nw, nh) = ((iw as f32 * scale).round() as u32, (ih as f32 * scale).round() as u32);
        img.resize(nw.max(1), nh.max(1), image::imageops::FilterType::Triangle)
    } else {
        img
    };
    // Encode to a tmp sibling then rename, so a concurrent reader never sees a half file.
    let tmp = out.with_extension("jpg.tmp");
    prepared.to_rgb8().save_with_format(&tmp, image::ImageFormat::Jpeg).ok()?;
    std::fs::rename(&tmp, &out).ok()?;
    tracing::info!("background: prepared {iw}x{ih} -> {}", prepared.width());
    Some(out)
}

/// Tiny non-crypto hash for the cache filename (stable across runs, unlike DefaultHasher).
fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::fnv1a;

    #[test]
    fn hash_is_stable_and_varies() {
        assert_eq!(fnv1a("a|1|2560x1440"), fnv1a("a|1|2560x1440"));
        assert_ne!(fnv1a("a|1|2560x1440"), fnv1a("a|2|2560x1440"));
    }
}
