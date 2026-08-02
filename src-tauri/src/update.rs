// OmniDeck — in-app update check (roadmap #4, the "check" half).
//
// In a gamescope session the launcher IS the desktop — there's no terminal to run an AUR
// helper from, so at minimum the launcher must be able to *tell* the user a release exists.
// This module asks the GitHub releases API for the latest tag and compares it against
// CARGO_PKG_VERSION. Acting on the result (paru / flatpak update / AppImage self-replace)
// is deliberately NOT here — it's distribution-dependent plumbing with its own threat model
// (see NOTES-DEEPDIVE-ROADMAP.md #4); the UI only gets facts to show.
//
// Network policy rides the shared http::client(): 5s/10s/15s timeouts and the SSRF-checked
// redirect policy. The URL is a compile-time constant (api.github.com) — no user input
// reaches the request. Unauthenticated GitHub API calls are rate-limited to 60/hr, so the
// result is cached for the process lifetime; a manual "Check now" passes `force`.
use serde::Deserialize;
use std::sync::Mutex;

/// Where releases live. Owner/repo only — the path is built here, never from config.
const REPO: &str = "atiner117/omnideck";

/// What the UI needs to render a "Update available" row/toast.
#[derive(Clone, serde::Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct UpdateInfo {
    /// The running version (CARGO_PKG_VERSION).
    pub current: String,
    /// Latest release tag, `v` prefix stripped (e.g. "0.3.0").
    pub latest: String,
    /// True when `latest` is strictly newer than `current` (numeric semver compare).
    pub update_available: bool,
    /// The release's html_url — shown as text/QR, never auto-opened.
    pub url: String,
    /// Release notes body, plain text, truncated — the frontend renders it as TEXT
    /// (Svelte escaping), never as HTML/markdown.
    pub notes: String,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

/// Numeric-component version compare: true when `latest` > `current`. Tags are `v`-prefix
/// tolerant; components compare as unsigned integers, missing components are 0, and any
/// unparseable component makes the answer false — an odd tag must never nag the user.
fn is_newer(latest: &str, current: &str) -> bool {
    fn parts(v: &str) -> Option<Vec<u64>> {
        let v = v.trim().trim_start_matches(['v', 'V']);
        // Cut a pre-release/build suffix ("0.3.0-rc1" → "0.3.0"): a pre-release compares
        // as its base here, which is fine for "is there anything newer than what I run".
        let v = v.split(['-', '+']).next().unwrap_or(v);
        v.split('.').map(|p| p.parse::<u64>().ok()).collect()
    }
    match (parts(latest), parts(current)) {
        (Some(l), Some(c)) => {
            let n = l.len().max(c.len());
            for i in 0..n {
                let (a, b) = (l.get(i).copied().unwrap_or(0), c.get(i).copied().unwrap_or(0));
                if a != b {
                    return a > b;
                }
            }
            false
        }
        _ => false,
    }
}

fn to_info(rel: &GhRelease) -> UpdateInfo {
    let latest = rel.tag_name.trim().trim_start_matches(['v', 'V']).to_string();
    let mut notes = rel.body.clone().unwrap_or_default();
    // Toast/modal-sized: the release body is arbitrary remote text; cap it hard.
    if notes.len() > 4000 {
        let mut cut = 4000;
        while !notes.is_char_boundary(cut) {
            cut -= 1;
        }
        notes.truncate(cut);
        notes.push('…');
    }
    UpdateInfo {
        current: env!("CARGO_PKG_VERSION").into(),
        latest: latest.clone(),
        update_available: is_newer(&latest, env!("CARGO_PKG_VERSION")),
        url: rel.html_url.clone(),
        notes,
    }
}

/// Process-lifetime cache: unauthenticated GitHub API allows 60 req/hr, and an always-on
/// living-room launcher would happily burn that on idle re-renders. `force` bypasses it
/// (the manual "Check now" row).
static CACHE: Mutex<Option<UpdateInfo>> = Mutex::new(None);

pub async fn check(force: bool) -> Result<UpdateInfo, String> {
    if !force {
        if let Some(hit) = CACHE.lock().unwrap().clone() {
            return Ok(hit);
        }
    }
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp = crate::http::client()
        .get(&url)
        // GitHub's API rejects UA-less requests.
        .header("User-Agent", concat!("omnideck/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("update check failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("update check failed: GitHub returned {}", resp.status()));
    }
    let rel: GhRelease = resp.json().await.map_err(|e| format!("update check failed: {e}"))?;
    if rel.draft || rel.prerelease {
        // /releases/latest shouldn't return these, but never prompt an update to one.
        return Err("update check: latest release is a draft/pre-release — ignoring".into());
    }
    let info = to_info(&rel);
    *CACHE.lock().unwrap() = Some(info.clone());
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare_basics() {
        assert!(is_newer("0.3.0", "0.2.0"));
        assert!(is_newer("v1.0.0", "0.9.9"));
        assert!(is_newer("0.2.10", "0.2.9")); // numeric, not lexicographic
        assert!(is_newer("0.3", "0.2.5")); // short forms pad with 0
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.9", "0.2.0"));
        assert!(!is_newer("v0.2.0", "V0.2.0"));
    }

    #[test]
    fn version_compare_never_nags_on_weird_tags() {
        assert!(!is_newer("nightly", "0.2.0"));
        assert!(!is_newer("", "0.2.0"));
        assert!(!is_newer("0.2.x", "0.2.0"));
    }

    #[test]
    fn prerelease_suffix_compares_as_base() {
        assert!(is_newer("0.3.0-rc1", "0.2.0"));
        assert!(!is_newer("0.2.0-rc1", "0.2.0"));
    }

    #[test]
    fn release_maps_to_info_and_truncates_notes() {
        let rel = GhRelease {
            tag_name: "v99.0.0".into(),
            html_url: "https://github.com/atiner117/omnideck/releases/tag/v99.0.0".into(),
            body: Some("x".repeat(10_000)),
            draft: false,
            prerelease: false,
        };
        let info = to_info(&rel);
        assert_eq!(info.latest, "99.0.0");
        assert!(info.update_available);
        assert_eq!(info.current, env!("CARGO_PKG_VERSION"));
        assert!(info.notes.len() <= 4003, "notes must be capped: {}", info.notes.len());
        assert!(info.notes.ends_with('…'));
    }
}
