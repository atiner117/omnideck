// Fake backend state for the browser harness — what a well-stocked machine would report.
//
// Every shape here is typed against the GENERATED ts-rs bindings (src/lib/bindings), so a
// Rust struct rename breaks `bun run check:e2e` instead of silently feeding the UI
// `undefined`. The one translation is `Wire<T>`: ts-rs maps Rust u64 -> TS `bigint`, but the
// value that actually crosses the IPC boundary is a JSON number, so the fixtures use numbers
// and `Wire<T>` rewrites those fields for the type checker.
//
// The defaults below mirror `Settings::default()` / `defaults()` in src-tauri/src/config.rs —
// screenshots should show the out-of-box look, not a look invented here.
import type { App } from "../../src/lib/bindings/App";
import type { Capability } from "../../src/lib/bindings/Capability";
import type { Config } from "../../src/lib/bindings/Config";
import type { Game } from "../../src/lib/bindings/Game";
import type { Library } from "../../src/lib/bindings/Library";
import type { LiveApp } from "../../src/lib/bindings/LiveApp";
import type { MediaInfo } from "../../src/lib/bindings/MediaInfo";
import type { MediaItem } from "../../src/lib/bindings/MediaItem";
import type { MediaSections } from "../../src/lib/bindings/MediaSections";
import type { UpdateInfo } from "../../src/lib/bindings/UpdateInfo";

/** A binding type as it arrives over IPC: ts-rs `bigint` fields are JSON numbers on the wire. */
export type Wire<T> = T extends bigint
  ? number
  : T extends string | number | boolean | null | undefined
    ? T
    : T extends Array<infer U>
      ? Array<Wire<U>>
      : T extends object
        ? { [K in keyof T]: Wire<T[K]> }
        : T;

/** Everything the mocked commands read. Overridable per test — see `installTauriMock`. */
export type Backend = {
  capability: Wire<Capability>;
  config: Wire<Config>;
  library: Wire<Library>;
  catalog: Wire<App>[];
  mediaAvailable: boolean;
  mediaSections: Wire<MediaSections>;
  /** `media_browse` responses, keyed by the parent id that was drilled into. */
  mediaBrowse: Record<string, Wire<MediaItem>[]>;
  nowPlaying: Wire<MediaInfo> | null;
  liveApps: Wire<LiveApp>[];
  update: Wire<UpdateInfo>;
  /** `get_art` responses (data URLs), keyed by the on-disk path the UI asks for. */
  art: Record<string, string>;
  /** `app_icon` responses (data URLs), keyed by the site URL. Empty = the emoji/accent tile. */
  appIcons: Record<string, string>;
  inSession: boolean;
};

const app = (
  id: string,
  name: string,
  icon: string,
  accent: string,
  category: string,
  exec: string[],
): Wire<App> => ({ id, name, icon, exec, accent, category });

/** A catalog web app, built the way `apps::catalog()` builds one. */
const web = (id: string, name: string, icon: string, url: string, accent: string, category: string) =>
  app(id, name, icon, accent, category, ["BROWSER", `--app=${url}`]);

// Fixed epoch seconds (Feb 2026) — deterministic "recently played" ordering on Home.
const T = 1_770_000_000;
const game = (appid: string, name: string, installdir: string, lastPlayed: number): Wire<Game> => ({
  appid,
  name,
  installdir,
  library_path: "/home/deck/.local/share/Steam",
  installed: true,
  is_tool: false,
  last_played: lastPlayed,
  // Steam box art lives on disk and is served over the `omnideck://` protocol, which only
  // exists inside the Tauri webview — in a plain browser it can't resolve, so the fixtures
  // declare no art and the UI renders its styled name-tile fallback. See e2e/mock/tauri.ts.
  art_box: null,
  art_header: null,
  art_hero: null,
  art_logo: null,
});

const GAMES: Wire<Game>[] = [
  game("1086940", "Baldur's Gate 3", "Baldurs Gate 3", T - 3_600),
  game("1245620", "ELDEN RING", "ELDEN RING", T - 86_400),
  game("1145360", "Hades", "Hades", T - 2 * 86_400),
  game("367520", "Hollow Knight", "hollow_knight", T - 5 * 86_400),
  game("413150", "Stardew Valley", "Stardew Valley", T - 9 * 86_400),
  game("548430", "Deep Rock Galactic", "Deep Rock Galactic", T - 14 * 86_400),
  game("1091500", "Cyberpunk 2077", "Cyberpunk 2077", T - 21 * 86_400),
  game("427520", "Factorio", "Factorio", T - 30 * 86_400),
  game("646570", "Slay the Spire", "SlayTheSpire", T - 44 * 86_400),
  game("588650", "Dead Cells", "Dead Cells", T - 60 * 86_400),
  game("620", "Portal 2", "Portal 2", 0),
  game("546560", "Half-Life: Alyx", "Half-Life Alyx", 0),
];

// What `apps::list()` detects on a machine with steam + heroic + the jellyfin shim + brave,
// plus a handful the user has already added from the catalog.
const APPS: Wire<App>[] = [
  app("steam-bpm", "Big Picture", "🎮", "#1b2a44", "games", ["steam", "steam://open/bigpicture"]),
  app("heroic", "Heroic", "🦸", "#2a2250", "games", ["heroic"]),
  app("jellyfin", "Jellyfin", "🪼", "#005a8c", "video", ["BROWSER", "--app=http://media.lan:8096"]),
  app("web", "Web", "🌐", "#5a2d12", "apps", ["brave"]),
  web("netflix", "Netflix", "🎬", "https://www.netflix.com", "#e50914", "video"),
  web("youtube", "YouTube", "▶️", "https://www.youtube.com", "#ff0000", "video"),
  web("plex-web", "Plex", "🎞️", "https://app.plex.tv", "#e5a00d", "video"),
  web("spotify-web", "Spotify", "🎵", "https://open.spotify.com", "#1db954", "music"),
  web("ytmusic", "YT Music", "🎧", "https://music.youtube.com", "#ff0000", "music"),
  web("gfn", "GeForce NOW", "☁️", "https://play.geforcenow.com", "#76b900", "games"),
];

// A trimmed `apps::catalog()` — enough rows for the Add-apps modal to show its grouping,
// its focused row, and both added/not-added states.
const CATALOG: Wire<App>[] = [
  ...APPS.filter((a) => ["steam-bpm", "heroic", "jellyfin", "web"].includes(a.id)),
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
  web("spotify-web", "Spotify", "🎵", "https://open.spotify.com", "#1db954", "music"),
  web("ytmusic", "YT Music", "🎧", "https://music.youtube.com", "#ff0000", "music"),
  web("tidal", "Tidal", "🌊", "https://listen.tidal.com", "#1a1a1a", "music"),
  web("deezer", "Deezer", "🎶", "https://www.deezer.com", "#a238ff", "music"),
  web("applemusic", "Apple Music", "🍎", "https://music.apple.com", "#fa2d48", "music"),
  web("soundcloud", "SoundCloud", "🔊", "https://soundcloud.com", "#ff5500", "music"),
  web("gfn", "GeForce NOW", "☁️", "https://play.geforcenow.com", "#76b900", "games"),
  web("xcloud", "Xbox Cloud", "🟩", "https://www.xbox.com/play", "#107c10", "games"),
];

const item = (i: Partial<Wire<MediaItem>> & Pick<Wire<MediaItem>, "id" | "name" | "kind">): Wire<MediaItem> => ({
  overview: null,
  played_pct: null,
  runtime_mins: null,
  series: null,
  position_secs: null,
  played: false,
  ...i,
});

/** The Jellyfin root: Continue Watching, Latest, then the libraries. */
const MEDIA_SECTIONS: Wire<MediaSections> = {
  server_name: "media.lan",
  resume: [
    item({ id: "ep-andor-107", name: "Announcement", kind: "Episode", series: "Andor", played_pct: 41, runtime_mins: 47, position_secs: 1_162 }),
    item({ id: "mv-dune2", name: "Dune: Part Two", kind: "Movie", runtime_mins: 166, played_pct: 12, position_secs: 1_195 }),
  ],
  latest: [
    item({ id: "mv-arrival", name: "Arrival", kind: "Movie", runtime_mins: 116 }),
    item({ id: "mv-blade2049", name: "Blade Runner 2049", kind: "Movie", runtime_mins: 164 }),
    item({ id: "sr-severance", name: "Severance", kind: "Series" }),
    item({ id: "mv-past-lives", name: "Past Lives", kind: "Movie", runtime_mins: 105, played: true }),
  ],
  libraries: [
    { id: "lib-movies", name: "Movies", kind: "movies" },
    { id: "lib-shows", name: "TV Shows", kind: "tvshows" },
    { id: "lib-music", name: "Music", kind: "music" },
  ],
};

const MEDIA_BROWSE: Record<string, Wire<MediaItem>[]> = {
  "lib-movies": [
    item({ id: "mv-arrival", name: "Arrival", kind: "Movie", runtime_mins: 116 }),
    item({ id: "mv-blade2049", name: "Blade Runner 2049", kind: "Movie", runtime_mins: 164 }),
    item({ id: "mv-dune2", name: "Dune: Part Two", kind: "Movie", runtime_mins: 166, played_pct: 12, position_secs: 1_195 }),
    item({ id: "mv-past-lives", name: "Past Lives", kind: "Movie", runtime_mins: 105, played: true }),
    item({ id: "mv-sicario", name: "Sicario", kind: "Movie", runtime_mins: 121 }),
  ],
  "lib-shows": [
    item({ id: "sr-andor", name: "Andor", kind: "Series" }),
    item({ id: "sr-severance", name: "Severance", kind: "Series" }),
    item({ id: "sr-foundation", name: "Foundation", kind: "Series" }),
  ],
  "sr-andor": [
    item({ id: "se-andor-1", name: "Season 1", kind: "Season" }),
    item({ id: "se-andor-2", name: "Season 2", kind: "Season" }),
  ],
};

/** The default machine: everything present, nothing playing, no wizard. */
export function defaultBackend(): Backend {
  return {
    capability: {
      tier: "plain-window",
      gpus: [{ pci: "0000:01:00.0", vendor: "NVIDIA", vendor_id: "10de", device_id: "2204", driver: "nvidia", class: "0300" }],
      render_nodes: ["/dev/dri/renderD128"],
      drm_cards: ["/dev/dri/card1"],
      kms_connectors: ["HDMI-A-1"],
      kms_active: true,
      vulkan_icds: ["/usr/share/vulkan/icd.d/nvidia_icd.json"],
      has_real_gpu: true,
      nvidia_present: true,
      nvidia_modeset_loaded: true,
      gamescope: true,
      gamescope_session_plus: false,
      cage: false,
      diagnostics: [],
    },
    config: {
      config_version: 1,
      settings: {
        grid_columns: 6,
        sort: "alpha",
        show_runtimes: false,
        accent: "#4cc2ff",
        theme: "omnidark",
        steamgriddb_key: "",
        onboarded: true,
        ui_scale: "medium",
        ui_scale_custom: 1.6,
        bg_blur: 0,
        bg_brightness: 0.82,
        search_provider: "https://duckduckgo.com/?q=",
        search_mode: "duckduckgo",
        sound: true,
        sound_volume: 0.5,
        dashboard_recents: 8,
        recents_show: "both",
        background_default: "color",
        background_color: "#05070b",
        background_image: "",
        game_backgrounds: true,
        app_backgrounds: true,
        live_wallpaper: "waves",
        ambient: false,
        ambient_volume: 0.35,
        pin_hash: "",
        locked_categories: [],
        check_updates: true,
        overscan_pct: 0,
      },
      appearance: { layout: "rail" },
      launch_overrides: {},
      input: { guide_hold_ms: 800, session_hotkeys: true },
      media_server: {
        kind: "jellyfin",
        url: "http://media.lan:8096",
        token: "", // masked over IPC, exactly as get_config does
        prefer_mpv: true,
        mpv_args: [],
        auto_profiles: true,
        audio_samplerate: 0,
        display_fps: 0,
        art_cache_mb: 200,
      },
      screensaver: { enabled: true, idle_dim_secs: 60, ken_burns_secs: 180, blank_secs: 600 },
      remote: { enabled: false, port: 8777, token: "" },
      apps: APPS,
      favorites: ["steam:1245620", "jellyfin", "steam:413150"],
      recent_apps: ["netflix", "spotify-web"],
      config_path: "/home/deck/.config/omnideck/config.toml",
      config_error: null,
      has_pin: false,
    },
    library: {
      steam_root: "/home/deck/.local/share/Steam",
      games: GAMES,
      libraries: [
        { path: "/home/deck/.local/share/Steam", label: "Internal", available: true, app_count: 9 },
        { path: "/run/media/games", label: "Games SSD", available: true, app_count: 3 },
      ],
      errors: [],
    },
    catalog: CATALOG,
    mediaAvailable: true,
    mediaSections: MEDIA_SECTIONS,
    mediaBrowse: MEDIA_BROWSE,
    nowPlaying: null,
    liveApps: [],
    update: { current: "0.2.0", latest: "0.2.0", update_available: false, url: "", notes: "" },
    art: {},
    // No favicons: `app_icon` returns data URLs the backend fetched from the network, which
    // this harness deliberately never does. Tiles fall back to emoji-on-accent — a real code
    // path, just not the network-warmed one. Drop entries here to pin specific icons.
    appIcons: {},
    inSession: false,
  };
}

/** Something is playing (drives the Now Playing card + its transport overlay). */
export const PLAYING: Wire<MediaInfo> = {
  status: "Playing",
  title: "Nightcall",
  artist: "Kavinsky",
  player: "Spotify",
};

/** Two launched apps, for the deck switcher. */
export const LIVE_APPS: Wire<LiveApp>[] = [
  { group: 4821, name: "ELDEN RING", id: "steam:1245620" },
  { group: 4907, name: "Spotify", id: "spotify-web" },
];
