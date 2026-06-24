// OmniDeck — non-Steam app/media tiles (the "media" half of the launcher).
// Each entry carries a category (video | music | games | apps) so the UI can group
// them in the category rail. v1 auto-detects installed apps from PATH/flatpak; a
// user-editable ~/.config/omnideck/config.toml is the source of truth at runtime.
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct App {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub exec: Vec<String>,
    pub accent: String,
    #[serde(default)]
    pub category: String, // "video" | "music" | "games" | "apps"
}

fn app(id: &str, name: &str, icon: &str, accent: &str, category: &str, exec: Vec<String>) -> App {
    App {
        id: id.into(),
        name: name.into(),
        icon: icon.into(),
        exec,
        accent: accent.into(),
        category: category.into(),
    }
}

fn has(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
        .unwrap_or(false)
}

/// First available browser, preferring Chromium-family (needed for `--app=` PWA mode).
pub fn detect_browser() -> Option<String> {
    for b in [
        "brave", "brave-browser", "chromium", "google-chrome-stable", "google-chrome",
        "vivaldi-stable", "microsoft-edge", "firefox",
    ] {
        if has(b) {
            return Some(b.to_string());
        }
    }
    None
}

/// Sensible default tiles for a fresh install (detected from what's present).
pub fn list() -> Vec<App> {
    let mut v = Vec::new();
    if has("steam") {
        v.push(app("steam-bpm", "Big Picture", "🎮", "#1b2a44", "games",
            vec!["steam".into(), "steam://open/bigpicture".into()]));
    }
    if has("heroic") {
        v.push(app("heroic", "Heroic", "🦸", "#2a2250", "games", vec!["heroic".into()]));
    }
    if has("jellyfin-mpv-shim") {
        v.push(app("jellyfin", "Jellyfin", "🪼", "#005a8c", "video", vec!["jellyfin-mpv-shim".into()]));
    }
    if has("brave") {
        v.push(app("web", "Web", "🌐", "#5a2d12", "apps", vec!["brave".into()]));
    }
    v
}

fn installed_flatpaks() -> std::collections::HashSet<String> {
    match std::process::Command::new("flatpak")
        .args(["list", "--app", "--columns=application"])
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|s| s.trim().to_string())
            .collect(),
        _ => std::collections::HashSet::new(),
    }
}

/// Native/flatpak media+music+game apps that are ACTUALLY installed (flatpak preferred,
/// native binary fallback). Only installed apps are returned — no broken tiles.
fn detected_apps() -> Vec<App> {
    let flatpaks = installed_flatpaks();
    // (id, name, icon, accent, flatpak_id, native_binary, category)
    let candidates = [
        ("spotify-app", "Spotify", "🎵", "#1db954", "com.spotify.Client", "spotify", "music"),
        ("feishin", "Feishin", "🎼", "#3478f6", "io.github.jeffvli.feishin", "feishin", "music"),
        ("plexamp", "Plexamp", "🎼", "#e5a00d", "", "plexamp", "music"),
        ("strawberry", "Strawberry", "🍓", "#ff6b6b", "org.strawberrymusicplayer.strawberry", "strawberry", "music"),
        ("tidal-hifi", "Tidal HiFi", "🌊", "#1a1a1a", "com.mastermindzh.tidal-hifi", "tidal-hifi", "music"),
        ("jellyfin-mp", "Jellyfin", "🪼", "#aa5cc3", "com.github.iwalton3.jellyfin-media-player", "jellyfinmediaplayer", "video"),
        ("plex-app", "Plex", "🎞️", "#e5a00d", "tv.plex.PlexDesktop", "plex-desktop", "video"),
        ("kodi", "Kodi", "📽️", "#17b2e7", "tv.kodi.Kodi", "kodi", "video"),
        ("vlc", "VLC", "🔶", "#ff8800", "org.videolan.VLC", "vlc", "video"),
        ("lutris", "Lutris", "🍷", "#ff5900", "net.lutris.Lutris", "lutris", "games"),
        ("retroarch", "RetroArch", "🕹️", "#222222", "org.libretro.RetroArch", "retroarch", "games"),
        ("esde", "EmulationStation", "👾", "#5b46f3", "org.es_de.frontend", "es-de", "games"),
        ("dolphin", "Dolphin", "🐬", "#5078ff", "org.DolphinEmu.dolphin-emu", "dolphin-emu", "games"),
        ("pcsx2", "PCSX2", "🎮", "#1a6fb5", "net.pcsx2.PCSX2", "pcsx2-qt", "games"),
        ("moonlight", "Moonlight", "🌙", "#5fb0e5", "com.moonlight_stream.Moonlight", "moonlight", "games"),
        ("steamlink", "Steam Link", "🔗", "#1b2a44", "com.valvesoftware.SteamLink", "steamlink", "games"),
        // System / desktop (detection-gated; KDE here, but generic binaries work elsewhere)
        ("systemsettings", "System Settings", "🛠️", "#3daee9", "", "systemsettings", "apps"),
        ("files", "Files", "📁", "#f2c14e", "", "dolphin", "apps"),
        ("terminal", "Terminal", "⌨️", "#2a2a2a", "", "konsole", "apps"),
    ];
    let mut out = Vec::new();
    for (id, name, icon, accent, fid, bin, category) in candidates {
        let exec = if !fid.is_empty() && flatpaks.contains(fid) {
            Some(vec!["flatpak".into(), "run".into(), fid.into()])
        } else if has(bin) {
            Some(vec![bin.into()])
        } else {
            None
        };
        if let Some(exec) = exec {
            out.push(app(id, name, icon, accent, category, exec));
        }
    }
    out
}

/// Built-in catalog: installed native/flatpak apps first, then browser app-mode entries
/// for mainstream services (the "BROWSER" token resolves to the host browser at launch).
pub fn catalog() -> Vec<App> {
    fn web(id: &str, name: &str, icon: &str, url: &str, accent: &str, category: &str) -> App {
        app(id, name, icon, accent, category, vec!["BROWSER".into(), format!("--app={url}")])
    }
    let mut v = detected_apps();
    v.extend([
        // Video
        web("netflix", "Netflix", "🎬", "https://www.netflix.com", "#e50914", "video"),
        web("disney", "Disney+", "🏰", "https://www.disneyplus.com", "#113ccf", "video"),
        web("max", "Max", "🅷", "https://play.max.com", "#0046ff", "video"),
        web("hulu", "Hulu", "🟢", "https://www.hulu.com", "#1ce783", "video"),
        web("prime", "Prime Video", "📦", "https://www.primevideo.com", "#1f9fe5", "video"),
        web("crunchyroll", "Crunchyroll", "🍥", "https://www.crunchyroll.com", "#f47521", "video"),
        web("youtube", "YouTube", "▶️", "https://www.youtube.com", "#ff0000", "video"),
        web("appletv", "Apple TV+", "🍏", "https://tv.apple.com", "#333333", "video"),
        web("plex-web", "Plex", "🎞️", "https://app.plex.tv", "#e5a00d", "video"),
        web("twitch", "Twitch", "🟣", "https://www.twitch.tv", "#9146ff", "video"),
        web("peacock", "Peacock", "🦚", "https://www.peacocktv.com", "#0a0a0a", "video"),
        web("paramount", "Paramount+", "⛰️", "https://www.paramountplus.com", "#0064ff", "video"),
        web("tubi", "Tubi", "📺", "https://tubitv.com", "#fa382b", "video"),
        // Music
        web("spotify-web", "Spotify", "🎵", "https://open.spotify.com", "#1db954", "music"),
        web("ytmusic", "YT Music", "🎧", "https://music.youtube.com", "#ff0000", "music"),
        web("tidal", "Tidal", "🌊", "https://listen.tidal.com", "#1a1a1a", "music"),
        web("deezer", "Deezer", "🎶", "https://www.deezer.com", "#a238ff", "music"),
        web("applemusic", "Apple Music", "🍎", "https://music.apple.com", "#fa2d48", "music"),
        web("soundcloud", "SoundCloud", "🔊", "https://soundcloud.com", "#ff5500", "music"),
        web("bandcamp", "Bandcamp", "🟦", "https://bandcamp.com", "#629aa9", "music"),
        web("qobuz", "Qobuz", "🎼", "https://www.qobuz.com", "#0070ef", "music"),
        // Cloud gaming
        web("gfn", "GeForce NOW", "☁️", "https://play.geforcenow.com", "#76b900", "games"),
        web("xcloud", "Xbox Cloud", "🟩", "https://www.xbox.com/play", "#107c10", "games"),
    ]);
    v
}
