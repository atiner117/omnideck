// OmniDeck — tiny filesystem helpers shared across modules.
//
// `write_atomic` is the temp-sibling + rename pattern (same one background.rs uses for
// cached JPEGs): a SIGTERM/power-cut mid-write can never leave a half-written file at the
// destination, because the destination only ever changes via rename(2) — atomic when the
// temp file lives in the same directory (same filesystem).
use std::fs;
use std::path::{Path, PathBuf};

/// Temp sibling in the SAME directory as `path` (rename across filesystems isn't atomic).
/// Pid-suffixed so two OmniDeck processes racing (launcher + `omnideck media` CLI) don't
/// stomp each other's temp file mid-write.
fn tmp_sibling(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!(".{name}.tmp.{}", std::process::id()))
}

/// Write `bytes` to `path` atomically: write a temp sibling, then rename it into place.
/// On any failure the temp file is cleaned up and `path` is untouched (either the old
/// content or absent — never truncated/partial).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = tmp_sibling(path);
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp); // best-effort: don't leave droppings next to the target
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("omnideck-fsutil-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn writes_new_file_and_leaves_no_temp() {
        let dir = scratch("new");
        let target = dir.join("mpv.conf");
        write_atomic(&target, b"profile-desc=x\n").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"profile-desc=x\n");
        let extras: Vec<_> = fs::read_dir(&dir).unwrap().flatten().collect();
        assert_eq!(extras.len(), 1, "temp sibling must be renamed away");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn replaces_existing_content() {
        let dir = scratch("replace");
        let target = dir.join("mpv.conf");
        fs::write(&target, b"old").unwrap();
        write_atomic(&target, b"new").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_parent_dir_errors_without_droppings() {
        let dir = scratch("noparent");
        let target = dir.join("nope").join("mpv.conf");
        assert!(write_atomic(&target, b"x").is_err());
        assert!(!target.exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
