<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";

  type App = { id: string; name: string; icon: string; exec: string[]; accent: string; category?: string };
  type Game = {
    appid: string; name: string; installed: boolean; is_tool: boolean; last_played?: number;
    installdir?: string; library_path?: string;
    art_box?: string | null; art_header?: string | null; art_hero?: string | null; art_logo?: string | null;
  };
  type Tile =
    | { kind: "game"; id: string; cat: string; game: Game }
    | { kind: "app"; id: string; cat: string; app: App };

  const CATEGORIES = [
    { id: "dashboard", label: "Home", icon: "⭐" },
    { id: "games", label: "Games", icon: "🎮" },
    { id: "video", label: "Movies & TV", icon: "🎬" },
    { id: "music", label: "Music", icon: "🎵" },
    { id: "apps", label: "Apps", icon: "🧩" },
    { id: "settings", label: "Settings", icon: "⚙" },
  ];
  const ACCENTS = ["#4cc2ff", "#b14cff", "#6ee7a8", "#ff8a3d", "#ff5d6c", "#ffd166"];
  const SEARCH_MODES = [
    { mode: "duckduckgo", label: "DuckDuckGo", url: "https://duckduckgo.com/?q=" },
    { mode: "google", label: "Google", url: "https://www.google.com/search?q=" },
    { mode: "brave", label: "Brave", url: "https://search.brave.com/search?q=" },
    { mode: "bing", label: "Bing", url: "https://www.bing.com/search?q=" },
    { mode: "searxng", label: "SearXNG", url: "" }, // self-hosted: user supplies the URL
    { mode: "custom", label: "Custom", url: "" },
  ];
  const PRESET: Record<string, number> = { small: 1.3, medium: 1.6, large: 1.9, huge: 2.3 };
  const SIZE_MODES = ["small", "medium", "large", "huge", "custom"];
  const BG_DEFAULTS = ["color", "image"];
  const BG_COLORS = ["#05070b", "#0d1117", "#161b26", "#1a1a2e", "#000000", "#14110a"];
  const RECENTS_MODES = ["both", "games", "apps"];
  const ALL_SETTINGS = [
    { key: "size", label: "Size", type: "cycle" },
    { key: "custom", label: "Custom size", type: "num" },
    { key: "bgdefault", label: "Default background", type: "cycle" },
    { key: "bgcolor", label: "Background color", type: "cycle" },
    { key: "bgimage", label: "Background image", type: "text" },
    { key: "gamebg", label: "Game backgrounds", type: "cycle" },
    { key: "appbg", label: "App backgrounds", type: "cycle" },
    { key: "blur", label: "Background blur", type: "num" },
    { key: "bright", label: "Background brightness", type: "num" },
    { key: "recents", label: "Home recents", type: "num" },
    { key: "recents_show", label: "Recents show", type: "cycle" },
    { key: "sort", label: "Sort", type: "cycle" },
    { key: "runtimes", label: "Show runtimes", type: "cycle" },
    { key: "accent", label: "Accent", type: "cycle" },
    { key: "sound", label: "Navigation sounds", type: "cycle" },
    { key: "soundvol", label: "Sound volume", type: "num" },
    { key: "search", label: "Search provider", type: "cycle" },
    { key: "searchurl", label: "Search URL", type: "text" },
    { key: "addcustom", label: "Add custom launcher", type: "action" },
  ];
  const POWER = [
    { key: "exit", label: "Exit OmniDeck", icon: "↩" },
    { key: "suspend", label: "Suspend", icon: "🌙" },
    { key: "reboot", label: "Restart", icon: "🔄" },
    { key: "poweroff", label: "Shut down", icon: "⏻" },
  ];
  const CATORDER: Record<string, number> = { games: 0, video: 1, music: 2, apps: 3 };
  const round2 = (v: number) => Math.round(v * 100) / 100;
  const cap1 = (s: string) => (s ? s[0].toUpperCase() + s.slice(1) : s);
  function settingValue(key: string): string {
    const s = cfg?.settings; if (!s) return "";
    if (key === "size") return cap1(s.ui_scale ?? "medium");
    if (key === "custom") return `${s.ui_scale_custom ?? 1.6}×`;
    if (key === "blur") return `${s.bg_blur ?? 0}px`;
    if (key === "bright") return `${Math.round((s.bg_brightness ?? 0.82) * 100)}%`;
    if (key === "recents") { const n = s.dashboard_recents ?? 8; return n ? `${n}` : "off"; }
    if (key === "recents_show") return cap1(s.recents_show ?? "both");
    if (key === "bgdefault") return { color: "Solid color", image: "Custom image" }[s.background_default as string] ?? "Solid color";
    if (key === "bgcolor") return s.background_color ?? "#05070b";
    if (key === "bgimage") return s.background_image ? s.background_image.split("/").pop() : "(none)";
    if (key === "gamebg") return s.game_backgrounds ? "on" : "off";
    if (key === "appbg") return s.app_backgrounds ? "on" : "off";
    if (key === "sort") return s.sort;
    if (key === "runtimes") return s.show_runtimes ? "on" : "off";
    if (key === "sound") return soundLabel();
    if (key === "soundvol") return `${Math.round((s.sound_volume ?? 0.6) * 100)}%`;
    if (key === "search") return SEARCH_MODES.find((m) => m.mode === (s.search_mode ?? "duckduckgo"))?.label ?? "DuckDuckGo";
    if (key === "searchurl") return s.search_provider || "(not set)";
    return "";
  }

  let cap = $state<any>(null);
  let cfg = $state<any>(null);
  let inSession = $state(false); // true when running as a gamescope session (vs desktop window)
  let accent = $state("#b14cff");
  let clock = $state("");
  // Now Playing: launch-tracked entries (games + non-media apps), each cleared when the
  // backend reports that process/game exited. Media apps are enriched with live MPRIS
  // metadata (song/artist) from the `media` poll below.
  type NowEntry = { kind: string; name: string; category: string };
  type MediaInfo = { status: string; title: string; artist: string; player: string };
  let nowList = $state<NowEntry[]>([]);
  let media = $state<MediaInfo | null>(null);
  // One card per launch entry; a media app's card shows its song. If something is playing
  // that we didn't launch (e.g. music already open), show a standalone media card too.
  let nowCards = $derived.by(() => {
    const out: Array<NowEntry & { media: MediaInfo | null }> = [];
    let mediaShown = false;
    for (const e of nowList) {
      const isMedia = e.category === "music" || e.category === "video";
      if (isMedia && media) { out.push({ ...e, media }); mediaShown = true; }
      else out.push({ ...e, media: null });
    }
    // standalone card for media we didn't launch (phone via KDE Connect, etc.): only while
    // actively playing, so a paused background player doesn't leave a card lingering.
    if (media && media.status === "Playing" && !mediaShown) out.push({ kind: "media", name: media.player || "Media", category: "music", media });
    return out.slice(0, 3);
  });

  let allGames = $state<Game[]>([]);
  let favorites = $state<string[]>([]);
  let recentApps = $state<string[]>([]); // app ids, most-recent-first
  let catSel = $state(1);
  let focus = $state(0);
  let status = $state("Loading…");
  let fps = $state(0);
  let lastInput = $state("—");

  let art = $state<Record<string, string>>({});
  let logos = $state<Record<string, string>>({});
  let gridBox = $state<Record<string, boolean>>({});
  let heroes = $state<Record<string, string>>({}); // wide hero art for the background
  let appIcons = $state<Record<string, string>>({}); // fetched site icons for web/app tiles
  let iconBg = $state<Record<string, string>>({}); // contrast-aware tile bg per fetched icon
  let iconColor = $state<Record<string, string>>({}); // dominant "r,g,b" per fetched icon (app bg gradient)
  let searchEngineIcon = $state(""); // favicon of the configured web-search provider
  const iconTried = new Set<string>();
  const iconInflight = new Set<string>(); // ids with an in-flight fetch (avoid duplicate IPC calls)
  // native apps with no launch URL but a known site to pull an icon from
  const ICON_DOMAIN: Record<string, string> = {
    jellyfin: "jellyfin.org", "jellyfin-mp": "jellyfin.org",
    moonlight: "moonlight-stream.org", "spotify-app": "spotify.com",
    plexamp: "plex.tv", "plex-app": "plex.tv", kodi: "kodi.tv", lutris: "lutris.net",
    retroarch: "retroarch.com", vlc: "videolan.org", strawberry: "strawberrymusicplayer.org",
  };
  // the system browser's own icon for the generic "Web" tile
  const BROWSER_DOMAIN: Record<string, string> = {
    brave: "brave.com", "brave-browser": "brave.com", chromium: "chromium.org",
    "google-chrome-stable": "google.com", "google-chrome": "google.com",
    "vivaldi-stable": "vivaldi.com", "microsoft-edge": "microsoft.com", firefox: "firefox.com",
  };
  function webUrl(a: App): string | null {
    for (const arg of a.exec) {
      if (arg.startsWith("--app=")) return arg.slice(6);
      if (arg.startsWith("http://") || arg.startsWith("https://")) return arg;
    }
    return null;
  }
  function iconSource(a: App): string | null {
    const u = webUrl(a); if (u) return u;
    if (ICON_DOMAIN[a.id]) return "https://" + ICON_DOMAIN[a.id];
    const bin = a.exec[0];
    if (bin && BROWSER_DOMAIN[bin]) return "https://" + BROWSER_DOMAIN[bin];
    return null;
  }
  async function loadAppIcon(a: App) {
    if (appIcons[a.id] || iconTried.has(a.id) || iconInflight.has(a.id)) return;
    const url = iconSource(a); if (!url) return;
    iconInflight.add(a.id);
    try {
      const d = await invoke<string | null>("app_icon", { url });
      if (d) { appIcons = { ...appIcons, [a.id]: d }; computeIconBg(a.id, d, a.accent); }
      iconTried.add(a.id); // got a definitive answer (icon or "none") — don't refetch
    } catch {
      // transient (IPC/network) failure: leave un-tried so a later pass can retry the icon
    } finally {
      iconInflight.delete(a.id);
    }
  }
  function hexLum(hex: string): number {
    const m = /^#?([0-9a-f]{6})$/i.exec((hex ?? "").trim());
    if (!m) return 0.1;
    const n = parseInt(m[1], 16);
    return (0.2126 * ((n >> 16) & 255) + 0.7152 * ((n >> 8) & 255) + 0.0722 * (n & 255)) / 255;
  }
  // Keep the app's brand accent as the tile background when the icon already reads on it;
  // only fall back to black/white when the icon's luminance is too close to the accent's.
  function computeIconBg(id: string, dataUrl: string, accent: string) {
    const img = new Image();
    img.onload = () => {
      try {
        const n = 24;
        const cv = document.createElement("canvas"); cv.width = n; cv.height = n;
        const ctx = cv.getContext("2d"); if (!ctx) return;
        ctx.drawImage(img, 0, 0, n, n);
        const d = ctx.getImageData(0, 0, n, n).data;
        let r = 0, g = 0, b = 0, a = 0;
        for (let i = 0; i < d.length; i += 4) { const al = d[i + 3] / 255; r += d[i] * al; g += d[i + 1] * al; b += d[i + 2] * al; a += al; }
        const ar = a < 1 ? 90 : Math.round(r / a), ag = a < 1 ? 96 : Math.round(g / a), ab = a < 1 ? 110 : Math.round(b / a);
        const iconLum = a < 1 ? 1 : (0.2126 * r + 0.7152 * g + 0.0722 * b) / a / 255;
        const keepAccent = Math.abs(iconLum - hexLum(accent)) >= 0.12; // enough contrast → keep color
        iconBg = { ...iconBg, [id]: keepAccent ? accent : iconLum > 0.5 ? "#0d0f14" : "#f4f5f8" };
        iconColor = { ...iconColor, [id]: `${ar},${ag},${ab}` }; // dominant color for the app bg gradient
      } catch {}
    };
    img.src = dataUrl;
  }
  let catalog = $state<App[]>([]);
  let sortedCatalog = $derived(
    [...catalog].sort(
      (a, b) =>
        ((CATORDER[a.category ?? "apps"] ?? 9) - (CATORDER[b.category ?? "apps"] ?? 9)) ||
        a.name.localeCompare(b.name),
    ),
  );

  let apps = $derived<App[]>((cfg?.apps ?? []) as App[]);
  let games = $derived(
    allGames
      .filter((g) => g.installed && (cfg?.settings?.show_runtimes ? true : !g.is_tool))
      .sort((a, b) =>
        cfg?.settings?.sort === "recent"
          ? (b.last_played ?? 0) - (a.last_played ?? 0)
          : a.name.toLowerCase().localeCompare(b.name.toLowerCase()),
      ),
  );
  function catOf(a: App): string {
    if (a.category) return a.category;
    const m = catalog.find((c) => c.id === a.id);
    if (m?.category) return m.category;
    if (a.id === "steam-bpm" || a.id === "heroic") return "games";
    if (a.id === "jellyfin") return "video";
    return "apps";
  }
  let gameTiles = $derived<Tile[]>(games.map((g) => ({ kind: "game", id: "steam:" + g.appid, cat: "games", game: g })));
  let appTiles = $derived<Tile[]>(apps.map((a) => ({ kind: "app", id: a.id, cat: catOf(a), app: a })));
  let allTiles = $derived<Tile[]>([...gameTiles, ...appTiles]);
  let catId = $derived(CATEGORIES[catSel].id);
  // Home = pinned favorites first, then recents (games by last-played, apps by launch order),
  // filtered by the recents_show setting (games | apps | both), not already pinned.
  let recentTiles = $derived.by<Tile[]>(() => {
    const n = cfg?.settings?.dashboard_recents ?? 8;
    if (!n) return [];
    const show = cfg?.settings?.recents_show ?? "both";
    const recentGames =
      show === "apps" ? [] :
      gameTiles
        .filter((t) => t.kind === "game" && (t.game.last_played ?? 0) > 0 && !favorites.includes(t.id))
        .sort((a, b) => (b.kind === "game" ? b.game.last_played ?? 0 : 0) - (a.kind === "game" ? a.game.last_played ?? 0 : 0));
    const recentAppTiles =
      show === "games" ? [] :
      recentApps
        .map((id) => appTiles.find((t) => t.id === id))
        .filter((t): t is Tile => !!t && !favorites.includes(t.id));
    // interleave apps first (most recently launched) then games, capped
    return [...recentAppTiles, ...recentGames].slice(0, n);
  });
  let items = $derived.by<Tile[]>(() => {
    switch (catId) {
      case "dashboard": return [...allTiles.filter((t) => favorites.includes(t.id)), ...recentTiles];
      case "games": return allTiles.filter((t) => t.cat === "games");
      case "video": return appTiles.filter((t) => t.cat === "video");
      case "music": return appTiles.filter((t) => t.cat === "music");
      case "apps": return appTiles.filter((t) => t.cat === "apps");
      default: return [];
    }
  });
  // hide rows that only apply to a current selection (custom size, bg color/image, custom volume)
  let visibleSettings = $derived(
    ALL_SETTINGS.filter((s) => {
      const set = cfg?.settings ?? {};
      if (s.key === "custom") return set.ui_scale === "custom";
      if (s.key === "soundvol") return soundLabel() === "Custom";
      if (s.key === "bgcolor") return (set.background_default ?? "color") === "color";
      if (s.key === "bgimage") return set.background_default === "image";
      if (s.key === "searchurl") return set.search_mode === "searxng" || set.search_mode === "custom";
      // blur/brightness only matter when something is overlaid (game art or app wash)
      if (s.key === "blur" || s.key === "bright") return (set.game_backgrounds ?? true) || (set.app_backgrounds ?? true) || set.background_default === "image";
      return true;
    }),
  );
  let itemCount = $derived(catId === "settings" ? visibleSettings.length : items.length);
  let scaleNum = $derived(
    cfg?.settings?.ui_scale === "custom"
      ? (cfg?.settings?.ui_scale_custom ?? 1.6)
      : (PRESET[cfg?.settings?.ui_scale ?? "medium"] ?? 1.6),
  );
  let settingsEditing = $state(false);

  // Background = a base (solid color or a custom image) plus an optional overlay: the
  // focused game's wide hero art, or a dominant-color gradient from the focused app's icon.
  let bgDefault = $derived<string>(cfg?.settings?.background_default ?? "color");
  let bgImageUrl = $state(""); // custom base image (data URL)
  // Debounce which item drives the background: swapping a fullscreen image on every
  // keypress while cycling fast is what tanked fps, so only update once focus settles.
  let settledFocus = $state(0);
  let bgTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const f = focus; catSel; // track focus + category
    clearTimeout(bgTimer);
    bgTimer = setTimeout(() => { settledFocus = f; }, 150);
  });
  let overlay = $derived.by<{ kind: "art"; url: string } | { kind: "wash"; color: string } | null>(() => {
    if (catId === "settings") return null;
    const t = items[settledFocus];
    if (t?.kind === "game" && (cfg?.settings?.game_backgrounds ?? true)) {
      const u = heroes[t.game.appid]; return u ? { kind: "art", url: u } : null;
    }
    if (t?.kind === "app" && (cfg?.settings?.app_backgrounds ?? true)) {
      const c = iconColor[t.app.id]; return c ? { kind: "wash", color: c } : null;
    }
    return null;
  });
  let baseImageShown = $derived(bgDefault === "image" && !!bgImageUrl);
  let hasImagery = $derived(!!overlay || baseImageShown);

  // ---- synthesized navigation sounds (no shipped audio assets) ----
  // Volume presets (mirrors the Size presets): Off / Low / Med / High / Custom.
  const SOUND_PRESETS = [
    { label: "Off", on: false, vol: 0 },
    { label: "Low", on: true, vol: 0.3 },
    { label: "Medium", on: true, vol: 0.6 },
    { label: "High", on: true, vol: 1.0 },
  ];
  function soundLabel(): string {
    const s = cfg?.settings; if (!s) return "Medium";
    if (!s.sound) return "Off";
    return SOUND_PRESETS.find((p) => p.on && Math.abs(p.vol - (s.sound_volume ?? 0.6)) < 0.001)?.label ?? "Custom";
  }
  let actx: AudioContext | null = null;
  function blip(freq: number, dur = 0.05, base = 0.2, type: OscillatorType = "sine", force = false) {
    if (!force && !cfg?.settings?.sound) return;
    const vol = cfg?.settings?.sound_volume ?? 0.6;
    try {
      actx ??= new AudioContext();
      if (actx.state === "suspended") actx.resume();
      const o = actx.createOscillator(), g = actx.createGain();
      o.type = type; o.frequency.value = freq;
      g.gain.setValueAtTime(Math.max(0.0001, base * vol), actx.currentTime);
      g.gain.exponentialRampToValueAtTime(0.0001, actx.currentTime + dur);
      o.connect(g).connect(actx.destination);
      o.start(); o.stop(actx.currentTime + dur);
    } catch {}
  }
  const sfxMove = () => blip(420, 0.04, 0.32, "triangle");
  const sfxEnter = () => { blip(620, 0.06, 0.42); setTimeout(() => blip(880, 0.07, 0.38), 45); };
  const sfxBack = () => blip(300, 0.07, 0.32, "triangle");

  function tileName(t: Tile) { return t.kind === "app" ? t.app.name : t.game.name; }
  function clamp(v: number, lo: number, hi: number) { return Math.max(lo, Math.min(hi, v)); }
  let lastNav = 0;
  function navGate() { const n = performance.now(); if (n - lastNav < 100) return false; lastNav = n; return true; }

  // Gamepad hold-to-repeat: gilrs emits one button_pressed per press (no auto-repeat like
  // the keyboard). Run the action once, then repeat while the direction is held.
  let heldCode = "";
  let heldDelay: ReturnType<typeof setTimeout> | undefined;
  let heldRepeat: ReturnType<typeof setInterval> | undefined;
  function holdStop() { clearTimeout(heldDelay); clearInterval(heldRepeat); heldCode = ""; }
  function holdStart(code: string, fn: () => void) {
    holdStop();
    heldCode = code;
    fn();
    heldDelay = setTimeout(() => { heldRepeat = setInterval(() => { if (heldCode === code) fn(); else holdStop(); }, 110); }, 360);
  }

  async function loadArt(g: Game) {
    if (!art[g.appid]) {
      const p = g.art_box || g.art_header || g.art_hero;
      if (p) { try { const d = await invoke<string | null>("get_art", { path: p }); if (d) art = { ...art, [g.appid]: d }; } catch {} }
    }
    if (!g.art_box && !gridBox[g.appid] && cfg?.settings?.steamgriddb_key) {
      try { const d = await invoke<string | null>("grid_art", { appid: g.appid }); if (d) { art = { ...art, [g.appid]: d }; gridBox = { ...gridBox, [g.appid]: true }; } } catch {}
    }
    if (g.art_hero && !heroes[g.appid]) {
      try { const d = await invoke<string | null>("get_art", { path: g.art_hero }); if (d) heroes = { ...heroes, [g.appid]: d }; } catch {}
    }
  }

  // ---- navigation (XMB: left/right = category, up/down = item) ----
  function moveCat(d: number) { const n = CATEGORIES.length; catSel = (catSel + d + n) % n; focus = 0; sfxMove(); }
  function moveItem(d: number) { settingsEditing = false; if (itemCount) { focus = (focus + d + itemCount) % itemCount; sfxMove(); } }
  function onWheel(e: WheelEvent) { e.preventDefault(); if (navGate()) moveItem(e.deltaY > 0 ? 1 : -1); }
  function adjustSetting(key: string, dir: number) {
    if (!cfg) return;
    const s = { ...cfg.settings };
    if (key === "recents") s.dashboard_recents = clamp((s.dashboard_recents ?? 8) + dir, 0, 20);
    else if (key === "custom") s.ui_scale_custom = round2(clamp((s.ui_scale_custom ?? 1.6) + dir * 0.05, 0.8, 3.5));
    else if (key === "blur") s.bg_blur = clamp((s.bg_blur ?? 0) + dir * 2, 0, 24);
    else if (key === "bright") s.bg_brightness = round2(clamp((s.bg_brightness ?? 0.82) + dir * 0.05, 0.3, 1.0));
    else if (key === "soundvol") { s.sound_volume = round2(clamp((s.sound_volume ?? 0.6) + dir * 0.05, 0, 1)); s.sound = s.sound_volume > 0; }
    cfg = { ...cfg, settings: s };
    invoke("save_settings", { settings: s }).catch(() => {});
    if (key === "soundvol") blip(620, 0.06, 0.42, "sine", true);
  }
  function doAction(key: string) {
    if (key === "addcustom") { formOpen = true; fName = ""; fExec = ""; fIcon = "🚀"; fCat = "apps"; }
  }
  // --- numeric settings: also typeable via a real <input> while editing ---
  const NUM_META: Record<string, { get: () => number; lo: number; hi: number; step: number; int?: boolean }> = {
    recents: { get: () => cfg?.settings?.dashboard_recents ?? 8, lo: 0, hi: 20, step: 1, int: true },
    custom: { get: () => cfg?.settings?.ui_scale_custom ?? 1.6, lo: 0.8, hi: 3.5, step: 0.05 },
    blur: { get: () => cfg?.settings?.bg_blur ?? 0, lo: 0, hi: 24, step: 1, int: true },
    bright: { get: () => cfg?.settings?.bg_brightness ?? 0.82, lo: 0.3, hi: 1.0, step: 0.05 },
    soundvol: { get: () => cfg?.settings?.sound_volume ?? 0.6, lo: 0, hi: 1, step: 0.05 },
  };
  function setNum(key: string, raw: number) {
    const m = NUM_META[key]; if (!cfg || !m || Number.isNaN(raw)) return;
    let v = clamp(raw, m.lo, m.hi); if (m.int) v = Math.round(v); else v = round2(v);
    const s = { ...cfg.settings };
    if (key === "recents") s.dashboard_recents = v;
    else if (key === "custom") s.ui_scale_custom = v;
    else if (key === "blur") s.bg_blur = v;
    else if (key === "bright") s.bg_brightness = v;
    else if (key === "soundvol") { s.sound_volume = v; s.sound = v > 0; }
    cfg = { ...cfg, settings: s };
    invoke("save_settings", { settings: s }).catch(() => {});
  }
  // text settings (currently just the custom background image path)
  function setText(key: string, raw: string) {
    if (!cfg) return;
    const s = { ...cfg.settings };
    if (key === "bgimage") s.background_image = raw.trim();
    else if (key === "searchurl") s.search_provider = raw.trim();
    cfg = { ...cfg, settings: s };
    invoke("save_settings", { settings: s }).catch(() => {});
  }
  function textValue(key: string): string {
    if (key === "bgimage") return cfg?.settings?.background_image ?? "";
    if (key === "searchurl") return cfg?.settings?.search_provider ?? "";
    return "";
  }
  function onBgColor(e: Event) {
    const v = (e.target as HTMLInputElement).value;
    if (!cfg) return;
    const s = { ...cfg.settings, background_color: v };
    cfg = { ...cfg, settings: s };
    invoke("save_settings", { settings: s }).catch(() => {});
  }
  function focusSelect(node: HTMLInputElement) { node.focus(); node.select(); }
  function isTyping() {
    const a = document.activeElement;
    return !!a && ["INPUT", "SELECT", "TEXTAREA"].includes(a.tagName);
  }
  function onAccentColor(e: Event) {
    const v = (e.target as HTMLInputElement).value;
    if (!cfg) return;
    const s = { ...cfg.settings, accent: v };
    cfg = { ...cfg, settings: s };
    accent = v;
    invoke("save_settings", { settings: s }).catch(() => {});
  }
  // horizontal: adjusts the focused numeric setting ONLY while editing; otherwise always
  // switches category (so you can never get trapped in Settings).
  function horiz(dir: number) {
    if (catId === "settings" && settingsEditing && visibleSettings[focus]?.type === "num") { adjustSetting(visibleSettings[focus].key, dir); return; }
    settingsEditing = false;
    moveCat(dir);
  }
  function activate() {
    if (catId === "settings") {
      const row = visibleSettings[focus];
      if (row?.type === "num" || row?.type === "text") settingsEditing = !settingsEditing; // Enter toggles edit
      else if (row?.type === "action") doAction(row.key);
      else if (row) cycleSetting(row.key);
      return;
    }
    const t = items[focus];
    if (t) { sfxEnter(); launchTile(t); }
  }
  async function launchTile(t: Tile) {
    const name = t.kind === "game" ? t.game.name : t.app.name;
    try {
      const category = t.kind === "game" ? "games" : catOf(t.app);
      if (t.kind === "game") { status = `▶ Launching ${name}…`; await invoke("launch_game", { appid: t.game.appid, name }); }
      else { status = `▶ ${name}…`; await invoke("launch_command", { exec: t.app.exec, name }); recordRecentApp(t.app.id); }
      nowList = [{ kind: t.kind, name, category }, ...nowList.filter((e) => e.name !== name)].slice(0, 3);
    } catch (e) { status = `launch error: ${e}`; return; }
    setTimeout(() => (status = ""), 3500);
  }
  function recordRecentApp(id: string) {
    recentApps = [id, ...recentApps.filter((x) => x !== id)].slice(0, 20);
    invoke("save_recent_apps", { recentApps }).catch(() => {});
  }
  function gotoSettings() { catSel = CATEGORIES.findIndex((c) => c.id === "settings"); focus = 0; }
  function goHome() { catSel = CATEGORIES.findIndex((c) => c.id === "dashboard"); focus = 0; }

  // ---- in-app info panel (games + apps) ----
  let infoOpen = $state(false);
  let infoTile = $state<Tile | null>(null);
  function showInfo() { if (catId === "settings") return; const t = items[focus]; if (t) { infoTile = t; infoOpen = true; } }
  function appSource(a: App): string {
    const e = a.exec;
    if (e[0] === "flatpak") return "Flatpak · " + (e[2] ?? "");
    if (e[0] === "BROWSER") { const u = webUrl(a); return "Web app · " + (u ? (iconDomainText(u)) : "browser"); }
    return "Command · " + e.join(" ");
  }
  function iconDomainText(url: string): string {
    return url.replace(/^--app=/, "").replace(/^https?:\/\//, "").split("/")[0];
  }
  function fmtPlayed(ts?: number): string {
    if (!ts) return "never";
    const d = new Date(ts * 1000);
    return d.toLocaleDateString([], { year: "numeric", month: "short", day: "numeric" });
  }
  async function cycleSetting(key: string) {
    if (!cfg) return;
    const s = { ...cfg.settings };
    if (key === "size") { const c = SIZE_MODES.indexOf(s.ui_scale ?? "medium"); s.ui_scale = SIZE_MODES[((c < 0 ? 1 : c) + 1) % SIZE_MODES.length]; }
    else if (key === "sort") s.sort = s.sort === "recent" ? "alpha" : "recent";
    else if (key === "runtimes") s.show_runtimes = !s.show_runtimes;
    else if (key === "sound") {
      // cycle Off → Low → Medium → High (Custom is reached via the Sound volume row)
      const cur = soundLabel();
      const i = SOUND_PRESETS.findIndex((p) => p.label === cur);
      const next = SOUND_PRESETS[(i < 0 ? 0 : i + 1) % SOUND_PRESETS.length];
      s.sound = next.on; s.sound_volume = next.vol;
      if (next.on) blip(620, 0.06, 0.42, "sine", true);
    }
    else if (key === "bgdefault") { const c = BG_DEFAULTS.indexOf(s.background_default ?? "color"); s.background_default = BG_DEFAULTS[((c < 0 ? 0 : c) + 1) % BG_DEFAULTS.length]; }
    else if (key === "gamebg") s.game_backgrounds = !s.game_backgrounds;
    else if (key === "appbg") s.app_backgrounds = !s.app_backgrounds;
    else if (key === "bgcolor") { const c = BG_COLORS.indexOf(s.background_color ?? BG_COLORS[0]); s.background_color = BG_COLORS[((c < 0 ? -1 : c) + 1) % BG_COLORS.length]; }
    else if (key === "recents_show") { const c = RECENTS_MODES.indexOf(s.recents_show ?? "both"); s.recents_show = RECENTS_MODES[((c < 0 ? 0 : c) + 1) % RECENTS_MODES.length]; }
    else if (key === "accent") { const c = ACCENTS.indexOf(s.accent ?? "#4cc2ff"); s.accent = ACCENTS[((c < 0 ? 0 : c) + 1) % ACCENTS.length]; }
    else if (key === "search") {
      const c = SEARCH_MODES.findIndex((m) => m.mode === (s.search_mode ?? "duckduckgo"));
      const next = SEARCH_MODES[((c < 0 ? 0 : c) + 1) % SEARCH_MODES.length];
      s.search_mode = next.mode;
      if (next.url) s.search_provider = next.url; // preset
      else if (SEARCH_MODES.some((m) => m.url === s.search_provider)) s.search_provider = ""; // entering searxng/custom from a preset → clear for the URL field
    }
    cfg = { ...cfg, settings: s };
    accent = s.accent ?? "#4cc2ff";
    invoke("save_settings", { settings: s }).catch(() => {});
  }

  function mediaControl(action: string) {
    invoke("media_control", { action }).catch(() => {});
    // re-poll so the play/pause glyph and title update promptly
    setTimeout(() => invoke<MediaInfo | null>("media_now_playing").then((m) => { media = m && m.status !== "Stopped" ? m : null; }).catch(() => {}), 250);
  }
  function isFav(id: string) { return favorites.includes(id); }
  function favCurrent() {
    if (catId === "settings") return;
    const t = items[focus]; if (!t) return;
    favorites = isFav(t.id) ? favorites.filter((x) => x !== t.id) : [...favorites, t.id];
    invoke("save_favorites", { favorites }).catch(() => {});
  }

  // ---- add-apps catalog ----
  // ---- power menu · confirm · custom-launcher form ----
  let powerOpen = $state(false);
  let powerFocus = $state(0);
  let confirmAct = $state<{ key: string; label: string } | null>(null);
  let formOpen = $state(false);
  let fName = $state("");
  let fExec = $state("");
  let fIcon = $state("🚀");
  let fCat = $state("apps");
  function openPower() { powerOpen = true; powerFocus = 0; }
  function powerMove(d: number) { powerFocus = clamp(powerFocus + d, 0, POWER.length - 1); }
  function powerActivate() {
    const key = POWER[powerFocus].key;
    powerOpen = false;
    if (key === "exit") invoke("quit").catch(() => {});
    else if (key === "suspend") invoke("power_action", { action: "suspend" }).catch(() => {});
    else confirmAct = { key, label: POWER[powerFocus].label };
  }
  function doConfirm() {
    if (!confirmAct) return;
    invoke("power_action", { action: confirmAct.key }).catch(() => {});
    confirmAct = null;
  }
  function addCustom() {
    const name = fName.trim();
    const cmd = fExec.trim();
    if (!cfg || !name || !cmd) { formOpen = false; return; }
    const id = "custom-" + name.toLowerCase().replace(/[^a-z0-9]+/g, "-");
    // A bare URL (e.g. a SearXNG instance) is launched as a browser app so it opens in the
    // browser AND gets its site favicon; anything else is run as a normal argv command.
    const isUrl = /^https?:\/\//i.test(cmd);
    const exec = isUrl ? ["BROWSER", `--app=${cmd}`] : cmd.split(/\s+/);
    const app = { id, name, icon: fIcon || "🚀", exec, accent: "#3a4256", category: fCat };
    const next = [...apps.filter((a) => a.id !== id), app];
    cfg = { ...cfg, apps: next };
    invoke("save_apps", { apps: next }).catch(() => {});
    formOpen = false;
  }

  let catalogOpen = $state(false);
  let catFocus = $state(0);
  let catQuery = $state("");
  let catSort = $state<"group" | "alpha">("group");
  let displayedCatalog = $derived.by(() => {
    const base = catSort === "alpha" ? [...catalog].sort((a, b) => a.name.localeCompare(b.name)) : sortedCatalog;
    const q = catQuery.trim().toLowerCase();
    return q ? base.filter((c) => c.name.toLowerCase().includes(q)) : base;
  });

  // ---- global search (games + apps, with a web-search fallback) ----
  let searchOpen = $state(false);
  let searchQuery = $state("");
  let searchFocus = $state(0);
  let searchResults = $derived.by(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return [] as Tile[];
    return allTiles
      .filter((t) => (t.kind === "game" ? t.game.name : t.app.name).toLowerCase().includes(q))
      .slice(0, 40);
  });
  function openSearch() { searchOpen = true; searchQuery = ""; searchFocus = 0; }
  function searchMove(d: number) {
    searchFocus = clamp(searchFocus + d, 0, searchResults.length); // last index = web-search row
    queueMicrotask(() => document.querySelector(`[data-sr="${searchFocus}"]`)?.scrollIntoView({ block: "nearest" }));
  }
  function webSearch() {
    if (!searchQuery.trim()) return;
    let prov = cfg?.settings?.search_provider || "https://duckduckgo.com/?q=";
    if (!/^https?:\/\//i.test(prov)) prov = "https://duckduckgo.com/?q="; // ignore a non-URL provider (safety + UX)
    invoke("launch_command", { exec: ["BROWSER", prov + encodeURIComponent(searchQuery)], name: "Search" }).catch(() => {});
    searchOpen = false;
    status = `🔎 ${searchQuery}`;
    setTimeout(() => (status = ""), 2500);
  }
  function searchActivate() {
    if (searchFocus < searchResults.length) { searchOpen = false; launchTile(searchResults[searchFocus]); }
    else webSearch();
  }
  function toggleCatalog() { catalogOpen = !catalogOpen; catFocus = 0; }
  function catMove(d: number) { catFocus = clamp(catFocus + d, 0, displayedCatalog.length - 1); queueMicrotask(() => document.querySelector(`[data-cat="${catFocus}"]`)?.scrollIntoView({ block: "nearest" })); }
  function isAdded(id: string) { return apps.some((a) => a.id === id); }
  async function catToggle(i: number) {
    const e = displayedCatalog[i]; if (!e || !cfg) return;
    const next = isAdded(e.id) ? apps.filter((a) => a.id !== e.id) : [...apps, e];
    cfg = { ...cfg, apps: next };
    try { await invoke("save_apps", { apps: next }); } catch {}
  }

  // ---- first-run wizard ----
  let wizardActive = $state(false);
  let wizardStep = $state(0);
  async function finishWizard() { wizardActive = false; if (!cfg) return; const s = { ...cfg.settings, onboarded: true }; cfg = { ...cfg, settings: s }; try { await invoke("save_settings", { settings: s }); } catch {} }
  function wizardNext() { if (wizardStep >= 2) finishWizard(); else wizardStep++; }
  function wizardPrev() { if (wizardStep > 0) wizardStep--; }
  function wizardAccent(dir: number) { if (!cfg) return; const c = ACCENTS.indexOf(cfg.settings.accent ?? "#4cc2ff"); const a = ACCENTS[((c < 0 ? 0 : c) + (dir > 0 ? 1 : ACCENTS.length - 1)) % ACCENTS.length]; cfg = { ...cfg, settings: { ...cfg.settings, accent: a } }; accent = a; invoke("save_settings", { settings: cfg.settings }).catch(() => {}); }

  function onKey(e: KeyboardEvent) {
    // A real <input>/<select> is focused (settings number field, custom-launcher form):
    // let it handle typing/arrows natively; only Enter/Escape blur out of it.
    if (isTyping()) {
      if (e.key === "Enter" || e.key === "Escape") {
        (document.activeElement as HTMLElement)?.blur();
        settingsEditing = false;
        if (e.key === "Escape") formOpen = false;
      }
      return;
    }
    const arrow = ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(e.key);
    if (arrow || e.key === "Enter" || e.key === "Escape") e.preventDefault();
    // Allow hold-to-repeat for arrows (throttled by navGate); ignore auto-repeat for
    // action keys so holding Enter can't launch a dozen times.
    if (e.repeat && !arrow) return;
    if (wizardActive) {
      if (e.key === "Enter") wizardNext();
      else if (e.key === "Escape") wizardPrev();
      else if (e.key === "ArrowLeft" && wizardStep === 1 && navGate()) wizardAccent(-1);
      else if (e.key === "ArrowRight" && wizardStep === 1 && navGate()) wizardAccent(1);
      return;
    }
    if (infoOpen) { if (e.key === "Escape" || e.key === "Enter" || e.key === "i" || e.key === "I") infoOpen = false; return; }
    if (formOpen) {
      // native inputs handle typing; only intercept Escape to close
      if (e.key === "Escape") { e.preventDefault(); formOpen = false; }
      return;
    }
    if (confirmAct) {
      if (e.key === "Enter") doConfirm();
      else if (e.key === "Escape") confirmAct = null;
      return;
    }
    if (powerOpen) {
      if (e.key === "ArrowUp" && navGate()) powerMove(-1);
      else if (e.key === "ArrowDown" && navGate()) powerMove(1);
      else if (e.key === "Enter") powerActivate();
      else if (e.key === "Escape") powerOpen = false;
      return;
    }
    if (e.key === "/" && !searchOpen && !catalogOpen) { e.preventDefault(); openSearch(); return; }
    if (searchOpen) {
      if (e.key === "ArrowUp") searchMove(-1);
      else if (e.key === "ArrowDown") searchMove(1);
      else if (e.key === "Enter") searchActivate();
      else if (e.key === "Escape") { if (searchQuery) searchQuery = ""; else searchOpen = false; }
      else if (e.key === "Backspace") searchQuery = searchQuery.slice(0, -1);
      else if (e.key.length === 1 && /^[\w .\-]$/.test(e.key)) searchQuery += e.key;
      return;
    }
    if ((e.key === "a" || e.key === "A") && !catalogOpen) { toggleCatalog(); return; }
    if (catalogOpen) {
      if (e.key === "ArrowUp" && navGate()) catMove(-1);
      else if (e.key === "ArrowDown" && navGate()) catMove(1);
      else if (e.key === "Enter") catToggle(catFocus);
      else if (e.key === "Tab") { e.preventDefault(); catSort = catSort === "group" ? "alpha" : "group"; }
      else if (e.key === "Escape") { if (catQuery) catQuery = ""; else catalogOpen = false; }
      else if (e.key === "Backspace") catQuery = catQuery.slice(0, -1);
      else if (e.key.length === 1 && /^[a-z0-9 ]$/i.test(e.key)) catQuery += e.key;
      return;
    }
    if (e.key === "p" || e.key === "P") { gotoSettings(); return; }
    if (e.key === "h" || e.key === "H") { goHome(); return; }
    if (e.key === "f" || e.key === "F") { favCurrent(); return; }
    if (e.key === "i" || e.key === "I") { showInfo(); return; }
    if (e.key === "Escape") { settingsEditing = false; return; }
    if (e.key === "ArrowLeft" && navGate()) horiz(-1);
    else if (e.key === "ArrowRight" && navGate()) horiz(1);
    else if (e.key === "ArrowUp" && navGate()) moveItem(-1);
    else if (e.key === "ArrowDown" && navGate()) moveItem(1);
    else if (e.key === "Enter") activate();
  }

  onMount(() => {
    window.addEventListener("keydown", onKey);
    invoke("get_capability").then((c) => (cap = c)).catch(() => {});
    invoke<boolean>("in_gamescope_session").then((v) => (inSession = !!v)).catch(() => {});
    invoke<any>("get_catalog").then((c) => (catalog = c ?? [])).catch(() => {});
    invoke<any>("get_config")
      .then((c) => {
        cfg = c;
        accent = c.settings?.accent ?? "#b14cff";
        favorites = c.favorites ?? [];
        recentApps = c.recent_apps ?? [];
        if (c.settings && c.settings.onboarded === false) { wizardActive = true; wizardStep = 0; }
      })
      .catch((e) => { status = `Couldn't load settings: ${e}`; }) // don't silently brick on "Loading…"
      .finally(() => {
        invoke<any>("get_library").then((lib) => { allGames = lib.games ?? []; if (cfg) status = ""; allGames.filter((g) => g.installed && !g.is_tool).forEach(loadArt); }).catch((e) => (status = `library error: ${e}`));
      });

    let raf = 0, acc = 0, timer = performance.now();
    const loop = (t: number) => { acc++; if (t - timer >= 500) { fps = Math.round((acc * 1000) / (t - timer)); acc = 0; timer = t; clock = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }); } raf = requestAnimationFrame(loop); };
    raf = requestAnimationFrame(loop);

    const off: Array<() => void> = [];
    // We add to nowList when we launch (we know game vs app there); the backend tells us
    // when the process/game actually exits so we can remove it. Match by name.
    listen("app-exited", (e: any) => { const n = String(e.payload ?? ""); nowList = nowList.filter((x) => x.name !== n); }).then((u) => off.push(u));
    // Poll MPRIS for the current song/show (works for native players + browser PWAs).
    const pollMedia = () => invoke<MediaInfo | null>("media_now_playing").then((m) => { media = m && m.status !== "Stopped" ? m : null; }).catch(() => {});
    pollMedia();
    const mediaTimer = setInterval(pollMedia, 4000);
    listen("gamepad-event", (e: any) => {
      const p = e.payload as { kind: string; code: string; value: number };
      if (p.kind === "button_pressed") {
        lastInput = p.code;
        if (wizardActive) {
          if (p.code === "South") wizardNext(); else if (p.code === "East") wizardPrev();
          else if (p.code === "DPadLeft" && wizardStep === 1) wizardAccent(-1);
          else if (p.code === "DPadRight" && wizardStep === 1) wizardAccent(1);
          return;
        }
        if (formOpen) { if (p.code === "East") formOpen = false; return; }
        if (confirmAct) {
          if (p.code === "South") doConfirm();
          else if (p.code === "East") confirmAct = null;
          return;
        }
        if (infoOpen) { if (p.code === "East" || p.code === "South") infoOpen = false; return; }
        if (powerOpen) {
          if (p.code === "DPadUp") holdStart(p.code, () => powerMove(-1));
          else if (p.code === "DPadDown") holdStart(p.code, () => powerMove(1));
          else if (p.code === "South") powerActivate();
          else if (p.code === "East") powerOpen = false;
          return;
        }
        if (searchOpen) {
          if (p.code === "DPadUp") holdStart(p.code, () => searchMove(-1));
          else if (p.code === "DPadDown") holdStart(p.code, () => searchMove(1));
          else if (p.code === "South") searchActivate();
          else if (p.code === "East") searchOpen = false;
          return;
        }
        if (catalogOpen) {
          if (p.code === "DPadUp") holdStart(p.code, () => catMove(-1));
          else if (p.code === "DPadDown") holdStart(p.code, () => catMove(1));
          else if (p.code === "South") catToggle(catFocus);
          else if (p.code === "North") catSort = catSort === "group" ? "alpha" : "group"; // toggle sort
          else if (p.code === "East") catalogOpen = false;
          return;
        }
        if (p.code === "North") { toggleCatalog(); return; }
        if (p.code === "Select") { openSearch(); return; }
        if (p.code === "Start") { goHome(); return; }
        if (p.code === "West") { favCurrent(); return; }
        if (p.code === "RightTrigger") { showInfo(); return; }
        if (p.code === "East") { settingsEditing = false; return; }
        if (p.code === "DPadLeft") holdStart(p.code, () => horiz(-1));
        else if (p.code === "DPadRight") holdStart(p.code, () => horiz(1));
        else if (p.code === "DPadUp") holdStart(p.code, () => moveItem(-1));
        else if (p.code === "DPadDown") holdStart(p.code, () => moveItem(1));
        else if (p.code === "South") activate();
      } else if (p.kind === "button_released") {
        if (p.code === heldCode) holdStop();
      } else if (p.kind === "axis_changed" && (p.code === "LeftStickX" || p.code === "LeftStickY")) {
        if (wizardActive || catalogOpen || searchOpen || powerOpen || confirmAct || formOpen || infoOpen) return;
        if (Math.abs(p.value) > 0.6 && navGate()) {
          if (p.code === "LeftStickX") horiz(p.value > 0 ? 1 : -1);
          else moveItem(p.value > 0 ? 1 : -1);
        } else if (Math.abs(p.value) < 0.3) { holdStop(); }
      }
    }).then((u) => off.push(u));

    return () => { window.removeEventListener("keydown", onKey); cancelAnimationFrame(raf); clearInterval(mediaTimer); off.forEach((u) => u()); };
  });

  $effect(() => { if (focus >= itemCount && itemCount) focus = itemCount - 1; });
  $effect(() => { if (catFocus >= displayedCatalog.length) catFocus = Math.max(0, displayedCatalog.length - 1); });
  // fetch site icons for visible web/app tiles + the add-apps catalog (cached on disk)
  $effect(() => { for (const t of items) if (t.kind === "app") loadAppIcon(t.app); });
  $effect(() => { for (const c of displayedCatalog) loadAppIcon(c); });
  // load the custom background image (data URL) when that mode is selected
  $effect(() => {
    const path = cfg?.settings?.background_image;
    if (cfg?.settings?.background_default === "image" && path) {
      invoke<string | null>("get_art", { path }).then((d) => { bgImageUrl = d ?? ""; }).catch(() => { bgImageUrl = ""; });
    } else { bgImageUrl = ""; }
  });
  // fetch the current web-search provider's favicon (shown on the search "web" row)
  $effect(() => {
    const prov = cfg?.settings?.search_provider;
    if (prov) invoke<string | null>("app_icon", { url: prov }).then((d) => { searchEngineIcon = d ?? ""; }).catch(() => {});
  });
</script>

<main style="--accent:{accent}; --scale:{scaleNum}; --bg-blur:{cfg?.settings?.bg_blur ?? 0}px; --bg-bright:{cfg?.settings?.bg_brightness ?? 0.82}; background-color:{cfg?.settings?.background_color ?? '#05070b'}">
  {#if baseImageShown}<div class="xbg base has" style="background-image:url({bgImageUrl})"></div>{/if}
  <div class="xbg" class:has={!!overlay} class:wash={overlay?.kind === "wash"}
    style={overlay?.kind === "art" ? `background-image:url(${overlay.url})`
      : overlay?.kind === "wash" ? `background-image:radial-gradient(120% 90% at 75% 25%, rgba(${overlay.color},0.55) 0%, rgba(${overlay.color},0.18) 38%, transparent 72%)`
      : ""}></div>
  <div class="xbg-fade" class:dim={!hasImagery}></div>

  <header>
    <div class="brand">OMNIDECK</div>
    <div class="meta">
      <span class="clock">{clock}</span>
      <button class="badge gear" onclick={openSearch} title="Search (/)">🔍</button>
      <button class="badge gear" onclick={toggleCatalog} title="Add apps (A / Triangle)">＋</button>
      <button class="badge gear" onclick={gotoSettings} title="Settings (P)">⚙</button>
      <button class="badge gear" onclick={openPower} title="Power">⏻</button>
    </div>
  </header>

  <!-- XMB cross -->
  <div class="xmb">
    <div class="xcats" style="transform: translateX(calc(30vw - {catSel} * var(--cw)))">
      {#each CATEGORIES as c, i}
        <button class="xcat" class:sel={i === catSel} onclick={() => { catSel = i; focus = 0; }}>
          <span class="xcicon">{c.icon}</span>
          {#if i === catSel}<span class="xclabel">{c.label}</span>{/if}
        </button>
      {/each}
    </div>

    <div class="xitems-wrap" onwheel={onWheel}>
      {#if catId === "settings"}
        <div class="xitems" style="transform: translateY(calc({-focus} * var(--ih)))">
          {#each visibleSettings as s, i}
            <button class="xitem" class:focused={i === focus} class:editing={settingsEditing && i === focus && (s.type === "num" || s.type === "text")}
              onclick={() => { focus = i; if (s.type === "num" || s.type === "text") settingsEditing = !settingsEditing; else if (s.type === "action") doAction(s.key); else cycleSetting(s.key); }}>
              <span class="xthumb settings"><span class="xemoji">⚙</span></span>
              <span class="xname">{s.label}
                {#if s.type === "num" && settingsEditing && i === focus}
                  <input class="numedit" type="number" use:focusSelect value={NUM_META[s.key].get()} step={NUM_META[s.key].step} min={NUM_META[s.key].lo} max={NUM_META[s.key].hi}
                    onchange={(e) => setNum(s.key, parseFloat((e.target as HTMLInputElement).value))} onclick={(e) => e.stopPropagation()} />
                  <span class="xsub">◀▶ or type · Enter</span>
                {:else if s.type === "text" && settingsEditing && i === focus}
                  <input class="textedit" type="text" use:focusSelect value={textValue(s.key)} placeholder="/path/to/image.jpg"
                    onchange={(e) => setText(s.key, (e.target as HTMLInputElement).value)} onclick={(e) => e.stopPropagation()} />
                  <span class="xsub">type a path · Enter</span>
                {:else}
                  <span class="xsub">{settingValue(s.key)}{s.type === "num" || s.type === "text" ? "  (Enter)" : ""}</span>
                {/if}
                {#if s.key === "accent"}<span class="swatch" style="background:{accent}"></span><input class="cwheel" type="color" value={accent} oninput={onAccentColor} onclick={(e) => e.stopPropagation()} />{/if}
                {#if s.key === "bgcolor"}<span class="swatch" style="background:{cfg?.settings?.background_color ?? '#05070b'}"></span><input class="cwheel" type="color" value={cfg?.settings?.background_color ?? '#05070b'} oninput={onBgColor} onclick={(e) => e.stopPropagation()} />{/if}
              </span>
            </button>
          {/each}
        </div>
      {:else if !items.length}
        <div class="xempty">
          {#if catId === "dashboard"}Nothing pinned — press <b>□ / F</b> on a tile to add it here.
          {:else if catId === "games"}No games found.
          {:else}Empty — press <b>△ / A</b> to add apps & media.{/if}
        </div>
      {:else}
        <div class="xitems" style="transform: translateY(calc({-focus} * var(--ih)))">
          {#each items as t, i (t.id)}
            <button class="xitem" class:focused={i === focus} class:near={Math.abs(i - focus) <= 4}
              onclick={() => { focus = i; launchTile(t); }}>
              <span class="xthumb" style={t.kind === "app" ? `background:${appIcons[t.app.id] ? (iconBg[t.app.id] ?? "#f4f5f8") : t.app.accent}` : ""}>
                {#if t.kind === "game" && art[t.game.appid] && Math.abs(i - focus) <= 8}
                  <img src={art[t.game.appid]} alt="" decoding="async" />
                {:else if t.kind === "app" && appIcons[t.app.id]}
                  <img class="appicon" src={appIcons[t.app.id]} alt="" decoding="async" />
                {:else}
                  <span class="xemoji">{t.kind === "app" ? t.app.icon : "🎮"}</span>
                {/if}
              </span>
              <span class="xname">{tileName(t)}{#if isFav(t.id)}<span class="xfav">⭐</span>{/if}</span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  {#if searchOpen}
    <button class="prefs-backdrop" aria-label="Close search" onclick={() => (searchOpen = false)}></button>
    <div class="prefs catalog">
      <button class="prefs-close" title="Close (Esc)" onclick={() => (searchOpen = false)}>✕</button>
      <h2>Search</h2>
      <div class="csearch active">{searchQuery ? `🔎 ${searchQuery}` : "Type to search your games, apps & the web…"}</div>
      <div class="catlist">
        {#each searchResults as t, i (t.id)}
          <div class="crow" class:focused={i === searchFocus} data-sr={i} onmouseenter={() => (searchFocus = i)} onclick={() => { searchFocus = i; searchActivate(); }}>
            <span class="cicon" style="background:{t.kind === 'app' && appIcons[t.app.id] ? (iconBg[t.app.id] ?? '#f4f5f8') : t.kind === 'app' ? t.app.accent : '#22304a'}">{#if t.kind === "app" && appIcons[t.app.id]}<img class="appicon" src={appIcons[t.app.id]} alt="" />{:else}{t.kind === "app" ? t.app.icon : "🎮"}{/if}</span>
            <span class="cname">{t.kind === "app" ? t.app.name : t.game.name}</span>
            <span class="ccat">{t.cat}</span>
          </div>
        {/each}
        <div class="crow" class:focused={searchFocus === searchResults.length} data-sr={searchResults.length} onmouseenter={() => (searchFocus = searchResults.length)} onclick={() => webSearch()}>
          <span class="cicon" style="background:#3a3f4a">{#if searchEngineIcon}<img class="appicon" src={searchEngineIcon} alt="" />{:else}🌐{/if}</span>
          <span class="cname">Search the web{searchQuery ? ` for “${searchQuery}”` : "…"}</span>
        </div>
      </div>
      <p class="phint">type · ↑↓ select · Enter open · Esc clear/close · web-search provider set in Settings</p>
    </div>
  {/if}

  {#if catalogOpen}
    <button class="prefs-backdrop" aria-label="Close add apps" onclick={() => (catalogOpen = false)}></button>
    <div class="prefs catalog">
      <button class="prefs-close" title="Close (Esc)" onclick={() => (catalogOpen = false)}>✕</button>
      <div class="chead">
        <h2>Add apps &amp; media</h2>
        <button class="sortbtn" onclick={() => (catSort = catSort === "group" ? "alpha" : "group")}>{catSort === "group" ? "Grouped" : "A–Z"}</button>
      </div>
      <div class="csearch" class:active={catQuery}>{catQuery ? `🔎 ${catQuery}` : "Type to search…  ·  Tab: sort"}</div>
      <div class="catlist">
        {#each displayedCatalog as c, i (c.id)}
          {#if catSort === "group" && (i === 0 || displayedCatalog[i - 1].category !== c.category)}<div class="cgroup">{c.category ?? "apps"}</div>{/if}
          <div class="crow" class:focused={i === catFocus} data-cat={i} onmouseenter={() => (catFocus = i)} onclick={() => { catFocus = i; catToggle(i); }}>
            <span class="cicon" style="background:{appIcons[c.id] ? (iconBg[c.id] ?? '#f4f5f8') : c.accent}">{#if appIcons[c.id]}<img class="appicon" src={appIcons[c.id]} alt="" />{:else}{c.icon}{/if}</span>
            <span class="cname">{c.name}</span>
            <span class="cstate" class:on={isAdded(c.id)}>{isAdded(c.id) ? "✓ Added" : "+ Add"}</span>
          </div>
        {/each}
        {#if !displayedCatalog.length}<div class="cgroup">no matches for “{catQuery}”</div>{/if}
      </div>
      <p class="phint">type to search · Tab sort · ↑↓ select · Enter/✕ toggle · Esc clear/close</p>
    </div>
  {/if}

  {#if infoOpen && infoTile}
    <button class="prefs-backdrop" aria-label="Close info" onclick={() => (infoOpen = false)}></button>
    <div class="prefs info">
      <button class="prefs-close" title="Close (Esc)" onclick={() => (infoOpen = false)}>✕</button>
      {#if infoTile.kind === "game"}
        <h2>{infoTile.game.name}</h2>
        <dl class="infogrid">
          <dt>Type</dt><dd>Steam game</dd>
          <dt>App ID</dt><dd>{infoTile.game.appid}</dd>
          <dt>Installed in</dt><dd>{infoTile.game.library_path}/steamapps/common/{infoTile.game.installdir}</dd>
          <dt>Last played</dt><dd>{fmtPlayed(infoTile.game.last_played)}</dd>
          <dt>Status</dt><dd>{infoTile.game.installed ? "Installed" : "Not installed"}</dd>
        </dl>
        <div class="confirm-btns">
          <button class="cbtn danger" onclick={() => { const t = infoTile; infoOpen = false; if (t) launchTile(t); }}>▶ Launch</button>
          <button class="cbtn" onclick={() => { if (infoTile?.kind === "game") invoke("game_properties", { appid: infoTile.game.appid }).catch(() => {}); }}>Steam properties</button>
        </div>
        <p class="phint">Steam properties opens Steam (for launch options / verify). Esc/◯ close.</p>
      {:else}
        <h2>{infoTile.app.name}</h2>
        <dl class="infogrid">
          <dt>Category</dt><dd>{infoTile.cat}</dd>
          <dt>Source</dt><dd>{appSource(infoTile.app)}</dd>
        </dl>
        <div class="confirm-btns">
          <button class="cbtn danger" onclick={() => { const t = infoTile; infoOpen = false; if (t) launchTile(t); }}>▶ Launch</button>
        </div>
        <p class="phint">Esc/◯ close · □/F favorite</p>
      {/if}
    </div>
  {/if}

  {#if powerOpen}
    <button class="prefs-backdrop" aria-label="Close power menu" onclick={() => (powerOpen = false)}></button>
    <div class="prefs power">
      <button class="prefs-close" title="Close (Esc)" onclick={() => (powerOpen = false)}>✕</button>
      <h2>Power</h2>
      <div class="catlist">
        {#each POWER as p, i}
          <div class="crow" class:focused={i === powerFocus} onmouseenter={() => (powerFocus = i)} onclick={() => { powerFocus = i; powerActivate(); }}>
            <span class="cicon" style="background:#22304a">{p.icon}</span>
            <span class="cname">{p.key === "exit" && inSession ? "Log out" : p.label}</span>
          </div>
        {/each}
      </div>
      <p class="phint">↑↓ select · Enter/✕ choose · Esc/◯ close</p>
    </div>
  {/if}

  {#if confirmAct}
    <button class="prefs-backdrop" aria-label="Cancel" onclick={() => (confirmAct = null)}></button>
    <div class="prefs confirm">
      <h2>{confirmAct.label}?</h2>
      <p class="wlead">This will {confirmAct.key === "reboot" ? "restart" : "shut down"} the computer.</p>
      <div class="confirm-btns">
        <button class="cbtn" onclick={() => (confirmAct = null)}>Cancel</button>
        <button class="cbtn danger" onclick={doConfirm}>{confirmAct.label}</button>
      </div>
      <p class="phint">Enter/✕ confirm · Esc/◯ cancel</p>
    </div>
  {/if}

  {#if formOpen}
    <button class="prefs-backdrop" aria-label="Close" onclick={() => (formOpen = false)}></button>
    <div class="prefs">
      <button class="prefs-close" title="Close (Esc)" onclick={() => (formOpen = false)}>✕</button>
      <h2>Add custom launcher</h2>
      <div class="frow"><label for="f-name">Name</label><input id="f-name" bind:value={fName} placeholder="My App" /></div>
      <div class="frow"><label for="f-exec">Command</label><input id="f-exec" bind:value={fExec} placeholder="/usr/bin/foo --flag" /></div>
      <div class="frow"><label for="f-icon">Icon</label><input id="f-icon" bind:value={fIcon} placeholder="🚀" /></div>
      <div class="frow"><label for="f-cat">Category</label>
        <select id="f-cat" bind:value={fCat}>
          <option value="games">Games</option>
          <option value="video">Movies &amp; TV</option>
          <option value="music">Music</option>
          <option value="apps">Apps</option>
        </select>
      </div>
      <div class="confirm-btns">
        <button class="cbtn" onclick={() => (formOpen = false)}>Cancel</button>
        <button class="cbtn danger" onclick={addCustom}>Add</button>
      </div>
      <p class="phint">Command is split on spaces. Use the full path if it isn't on PATH. Esc to close.</p>
    </div>
  {/if}

  {#if wizardActive && cfg}
    <div class="wizard">
      {#if wizardStep === 0}
        <div class="wstep">
          <div class="wlogo">OMNIDECK</div><h2>Welcome 👋</h2>
          <p class="wlead">Your living-room launcher for games, streaming, music, and your own media.</p>
          <ul class="wfacts"><li>Mode: <b>{cap?.tier ?? "…"}</b></li><li>Games: <b>{games.length}</b></li><li>Apps to add: <b>{catalog.length}</b></li></ul>
          <div class="wnav">Press <b>Enter / ✕</b> to continue</div>
        </div>
      {:else if wizardStep === 1}
        <div class="wstep">
          <h2>Pick a theme</h2><p class="wlead">Choose an accent — change it anytime in Settings.</p>
          <div class="wswatches">{#each ACCENTS as a}<span class="wsw" class:sel={cfg.settings.accent === a} style="background:{a}"></span>{/each}</div>
          <div class="wnav"><b>◀ ▶</b> change · <b>Enter / ✕</b> next · <b>Esc / ◯</b> back</div>
        </div>
      {:else}
        <div class="wstep">
          <h2>You're set! 🎮</h2>
          <ul class="wfacts"><li><b>← →</b> category · <b>↑ ↓</b> items</li><li><b>Enter / ✕</b> launch · <b>□ / F</b> favorite</li><li><b>△ / A</b> add apps · <b>Start / H</b> home · <b>P</b> settings · <b>/ Select</b> search · <b>i / R1</b> info</li></ul>
          <div class="wnav">Press <b>Enter / ✕</b> to start</div>
        </div>
      {/if}
    </div>
  {/if}

  {#if nowCards.length}
    <div class="nowstack">
      {#each nowCards as c (c.kind + c.name)}
        <div class="nowplaying">
          {#if c.media && c.media.status === "Playing"}<span class="np-eq"><i></i><i></i><i></i></span>
          {:else if c.media}<span class="np-icon">⏸</span>
          {:else}<span class="np-spinner"></span>{/if}
          <span class="np-label">
            {c.media ? "Now playing" : c.kind === "game" ? "Game running" : "Running"}<br />
            {#if c.media && c.media.title}<b>{c.media.title}</b>{#if c.media.artist}<span class="np-sub"> — {c.media.artist}</span>{/if}
            {:else}<b>{c.kind === "game" ? "🎮 " : "▶ "}{c.name}</b>{/if}
          </span>
          {#if c.media}
            <span class="np-controls">
              <button class="np-c" title="Previous" onclick={() => mediaControl("previous")}>⏮</button>
              <button class="np-c" title="Play / Pause" onclick={() => mediaControl("play-pause")}>{c.media.status === "Playing" ? "⏸" : "▶"}</button>
              <button class="np-c" title="Next" onclick={() => mediaControl("next")}>⏭</button>
            </span>
          {/if}
          {#if c.kind === "app"}<button class="np-c" title="Close &amp; return (or press the Guide button)" onclick={() => invoke("close_current_app")}>↩</button>{/if}
          {#if c.kind !== "media"}<button class="np-x" title="Dismiss (doesn't close the app)" onclick={() => (nowList = nowList.filter((x) => x.name !== c.name))}>✕</button>{/if}
        </div>
      {/each}
    </div>
  {/if}

  {#if status}<div class="toast">{status}</div>{/if}

  <footer>fps {fps} · {cap?.tier ?? "?"} · {lastInput} · <b>← →</b> category · <b>↑ ↓</b> items · <b>Enter/✕</b> launch · <b>□/F</b> favorite · <b>△/A</b> add · <b>/ Select</b> search · <b>i/R1</b> info · <b>Start/H</b> home · <b>P</b> settings</footer>
</main>

<style>
  :global(html), :global(body) { margin: 0; height: 100%; }
  :global(body) { background: #05070b; overflow: hidden; }

  main {
    position: relative; height: 100vh; box-sizing: border-box; display: flex; flex-direction: column;
    color: #eef2f8; font-family: "Inter", system-ui, sans-serif; overflow: hidden;
    --scale: 1.08;
    /* base sizes are viewport-aware (clamp) so a small window degrades gracefully;
       at full-screen they hit the rem cap, so the primary use is unchanged. */
    --cw: calc(clamp(4.2rem, 8.5vw, 7rem) * var(--scale));
    --ih: calc(clamp(2.8rem, 5.2vh, 4.4rem) * var(--scale));
  }
  .xbg { position: absolute; inset: 0; background-size: cover; background-position: center; filter: blur(var(--bg-blur, 0px)) brightness(var(--bg-bright, .82)) saturate(1.12); opacity: 0; transition: opacity .3s ease; z-index: 0; }
  .xbg.has { opacity: 1; }
  .xbg.base { z-index: 0; } /* custom image sits under the dynamic overlay */
  /* app icon → blurred, enlarged color wash (the small icon becomes a branded gradient) */
  /* app background is now a cheap dominant-color gradient (no image decode / heavy blur) */
  .xbg.wash { filter: brightness(var(--bg-bright, .9)) saturate(1.25); }
  /* sharp art; dark only under the item list (left), clear on the right so the art reads */
  .xbg-fade { position: absolute; inset: 0; z-index: 0; background: linear-gradient(90deg, #05070bfa 0%, #05070bf0 26%, #05070b9e 55%, #05070b33 100%), linear-gradient(180deg, #05070b59 0%, transparent 32%, #05070b99 100%); }
  /* solid-color mode: only a light left-edge darken for item legibility, color shows elsewhere */
  .xbg-fade.dim { background: linear-gradient(90deg, #00000066 0%, transparent 55%); }
  header, .xmb, .toast, footer { position: relative; z-index: 2; }

  header { display: flex; align-items: center; justify-content: space-between; padding: 1.8vh 2.4vw 1vh; }
  .brand { font-size: clamp(20px, 2.4vw, 36px); font-weight: 800; letter-spacing: 3px; color: var(--accent); }
  .meta { display: flex; gap: 10px; align-items: center; }
  .clock { color: #cdd7e6; font-weight: 700; font-variant-numeric: tabular-nums; font-size: calc(clamp(13px, 1.5vw, 19px) * var(--scale)); margin-right: 4px; }
  .badge { background: #121a2b99; border: 1px solid #25324d; border-radius: 999px; padding: 5px 14px; color: #9fb0c8; font-size: clamp(11px, 1.2vw, 14px); }
  .gear { cursor: pointer; font-size: 1.05em; line-height: 1; }

  .xmb { flex: 1; position: relative; min-height: 0; }
  /* horizontal category axis */
  .xcats { position: absolute; top: 16%; left: 0; display: flex; gap: 0; will-change: transform; transition: transform .16s cubic-bezier(.2,.7,.2,1); }
  .xcat { width: var(--cw); flex: 0 0 var(--cw); background: none; border: 0; color: #8392ab; cursor: pointer; display: flex; flex-direction: column; align-items: center; gap: 8px; position: relative; }
  .xcat .xcicon { font-size: calc(clamp(28px, 3.2vw, 46px) * var(--scale)); opacity: .55; transition: opacity .2s, transform .2s; }
  .xcat.sel .xcicon { opacity: 1; transform: scale(1.45); filter: drop-shadow(0 0 18px color-mix(in srgb, var(--accent) 70%, transparent)); }
  .xclabel { position: absolute; top: calc(4.2rem * var(--scale)); white-space: nowrap; color: #fff; font-weight: 700; font-size: calc(clamp(14px, 1.5vw, 20px) * var(--scale)); }

  /* vertical item cascade, focused item parked at the cross line */
  /* Anchor the list below the category label using the SAME rem*scale unit the label
     uses (.xclabel sits at 4.2rem*scale), so they never collide on short viewports
     (720p, or a 1280x800 handheld). A bare 34% clipped the top icon at small heights. */
  .xitems-wrap { position: absolute; top: calc(16% + 7rem * var(--scale)); left: 30vw; right: 4vw; bottom: 0; overflow: hidden; }
  .xitems { display: flex; flex-direction: column; gap: 0; will-change: transform; transition: transform .12s cubic-bezier(.2,.7,.2,1); }
  .xitem { height: var(--ih); display: flex; align-items: center; gap: 1rem; background: none; border: 0; color: #c2cbdb; cursor: pointer; text-align: left; opacity: .42; transition: opacity .12s, transform .12s; padding: 0 10px; border-radius: 12px; }
  .xitem.near { opacity: .72; }
  .xitem.focused { opacity: 1; transform: translateX(14px) scale(1.2); transform-origin: left center; }
  .xitem.editing { background: color-mix(in srgb, var(--accent) 16%, transparent); }
  .xthumb { width: calc(3.1rem * var(--scale)); height: calc(3.1rem * var(--scale)); border-radius: 10px; flex: 0 0 auto; overflow: hidden; display: grid; place-items: center; background: #1a2233; box-shadow: 0 4px 14px #0007; }
  .xthumb img { width: 100%; height: 100%; object-fit: cover; }
  .xthumb img.appicon { object-fit: contain; padding: 18%; box-sizing: border-box; }
  .cicon img.appicon { width: 70%; height: 70%; object-fit: contain; }
  .xthumb .xemoji { font-size: calc(1.5rem * var(--scale)); }
  .xitem.focused .xthumb { box-shadow: 0 0 0 2px var(--accent), 0 8px 24px #000a; }
  .xname { font-size: calc(clamp(16px, 1.7vw, 24px) * var(--scale)); font-weight: 600; display: flex; align-items: center; gap: 10px; }
  .xitem.focused .xname { font-weight: 800; }
  .xname .xsub { color: var(--accent); font-weight: 700; font-size: .8em; }
  .xfav { font-size: .8em; }
  .swatch { width: 30px; height: 18px; border-radius: 5px; display: inline-block; border: 1px solid #ffffff44; }
  .numedit { width: 5em; background: #0c1320; border: 1px solid var(--accent); color: #fff; border-radius: 7px; padding: 2px 8px; font-size: .8em; font-weight: 700; }
  .textedit { width: 18em; max-width: 40vw; background: #0c1320; border: 1px solid var(--accent); color: #fff; border-radius: 7px; padding: 2px 8px; font-size: .8em; }
  .numedit:focus, .textedit:focus { outline: none; }
  .xempty { position: absolute; top: calc(16% + 7rem * var(--scale)); left: 30vw; right: 4vw; color: #8a96ab; font-size: clamp(15px, 1.8vw, 22px); }
  .xempty b { color: var(--accent); }

  .toast { position: fixed; bottom: 7vh; left: 50%; transform: translateX(-50%); background: var(--accent); color: #04121f; font-weight: 700; padding: 12px 28px; border-radius: 999px; box-shadow: 0 10px 40px color-mix(in srgb, var(--accent) 38%, transparent); font-size: clamp(14px, 1.6vw, 20px); }

  .nowstack { position: fixed; z-index: 12; right: 2.4vw; bottom: 8vh; display: flex; flex-direction: column; gap: 10px; align-items: flex-end; }
  .nowplaying { display: flex; align-items: center; gap: 16px; background: #0c1320e8; border: 1px solid color-mix(in srgb, var(--accent) 45%, transparent); border-radius: 16px; padding: 14px 20px; box-shadow: 0 20px 60px #000b; max-width: 42vw; }
  .np-spinner { width: 22px; height: 22px; border-radius: 50%; border: 3px solid #2c3a5c; border-top-color: var(--accent); animation: np-spin 0.9s linear infinite; flex: 0 0 auto; }
  @keyframes np-spin { to { transform: rotate(360deg); } }
  .np-icon { font-size: 20px; color: var(--accent); flex: 0 0 auto; }
  .np-eq { display: flex; align-items: flex-end; gap: 2px; height: 20px; flex: 0 0 auto; }
  .np-eq i { width: 4px; background: var(--accent); border-radius: 2px; animation: np-eq 0.9s ease-in-out infinite; }
  .np-eq i:nth-child(1) { animation-delay: 0s; } .np-eq i:nth-child(2) { animation-delay: 0.3s; } .np-eq i:nth-child(3) { animation-delay: 0.6s; }
  @keyframes np-eq { 0%, 100% { height: 6px; } 50% { height: 18px; } }
  .np-label { font-size: clamp(13px, 1.4vw, 17px); color: #9fb0c8; line-height: 1.3; min-width: 0; }
  .np-label b { color: #fff; font-size: 1.1em; }
  .np-sub { color: #9fb0c8; }
  .np-x { background: #1b2540; border: 1px solid #2c3a5c; color: #9fb0c8; border-radius: 8px; width: 30px; height: 30px; cursor: pointer; font-size: 14px; flex: 0 0 auto; }
  .np-x:hover { border-color: var(--accent); color: #fff; }
  .np-controls { display: flex; gap: 6px; flex: 0 0 auto; }
  .np-c { background: #1b2540; border: 1px solid #2c3a5c; color: #cdd7e6; border-radius: 8px; width: 32px; height: 32px; cursor: pointer; font-size: 14px; }
  .np-c:hover { border-color: var(--accent); color: #fff; }

  .prefs-backdrop { position: fixed; inset: 0; background: rgba(4,6,10,.6); border: 0; padding: 0; cursor: pointer; z-index: 10; }
  .prefs { position: fixed; z-index: 11; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(620px, 92vw); background: #121826; border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent); border-radius: 18px; padding: 22px 26px; box-shadow: 0 30px 80px #000c; display: flex; flex-direction: column; gap: 4px; }
  .prefs h2 { margin: 0 0 10px; font-size: clamp(20px, 2.2vw, 26px); }
  .prefs-close { position: absolute; top: 14px; right: 14px; width: 34px; height: 34px; border-radius: 9px; background: #1b2540; border: 1px solid #2c3a5c; color: #9fb0c8; cursor: pointer; font-size: 15px; line-height: 1; }
  .prefs-close:hover { border-color: var(--accent); color: #fff; }
  .catlist { max-height: 60vh; overflow-y: auto; display: flex; flex-direction: column; gap: 2px; margin: 4px 0; }
  .crow { display: flex; align-items: center; gap: 14px; padding: 9px 12px; border-radius: 10px; border: 2px solid transparent; cursor: pointer; }
  .crow.focused { background: #1b2540; border-color: var(--accent); }
  .cicon { width: 38px; height: 38px; border-radius: 9px; display: grid; place-items: center; font-size: 20px; flex: 0 0 auto; }
  .cname { flex: 1; font-size: clamp(14px, 1.5vw, 18px); font-weight: 600; }
  .ccat { color: #6b7790; font-size: clamp(11px, 1.1vw, 13px); text-transform: uppercase; letter-spacing: 1px; }
  .cstate { color: #7e8aa0; font-weight: 700; font-size: clamp(12px, 1.3vw, 15px); min-width: 72px; text-align: right; }
  .cstate.on { color: #6ee7a8; }
  .cgroup { color: #6b7790; font-size: clamp(11px, 1.1vw, 13px); text-transform: uppercase; letter-spacing: 2px; font-weight: 700; padding: 12px 10px 4px; }
  .cgroup:first-child { padding-top: 2px; }
  .chead { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding-right: 44px; }
  .sortbtn { background: #1b2540; border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent); color: #cdd7e6; border-radius: 999px; padding: 4px 14px; cursor: pointer; font-size: clamp(11px, 1.1vw, 14px); font-weight: 700; }
  .csearch { color: #93a0b6; font-size: clamp(12px, 1.2vw, 15px); padding: 4px 2px 6px; }
  .csearch.active { color: var(--accent); font-weight: 700; }
  .cwheel { width: 30px; height: 22px; padding: 0; border: 1px solid #ffffff55; border-radius: 5px; background: none; cursor: pointer; }
  .cwheel::-webkit-color-swatch-wrapper { padding: 0; }
  .cwheel::-webkit-color-swatch { border: none; border-radius: 4px; }
  .phint { color: #7e8aa0; font-size: clamp(11px, 1.1vw, 13px); margin: 3px 0 0; }

  .infogrid { display: grid; grid-template-columns: max-content 1fr; gap: 6px 18px; margin: 6px 0 8px; }
  .infogrid dt { color: #7e8aa0; font-size: clamp(12px, 1.2vw, 14px); font-weight: 700; }
  .infogrid dd { margin: 0; color: #dde5f0; font-size: clamp(12px, 1.3vw, 15px); word-break: break-word; }
  .confirm-btns { display: flex; gap: 12px; justify-content: flex-end; margin: 14px 0 4px; }
  .cbtn { background: #1b2540; border: 1px solid #2c3a5c; color: #cdd7e6; border-radius: 10px; padding: 9px 22px; cursor: pointer; font-size: clamp(13px, 1.4vw, 16px); font-weight: 700; }
  .cbtn:hover { border-color: var(--accent); }
  .cbtn.danger { background: var(--accent); color: #04121f; border-color: transparent; }
  .frow { display: flex; align-items: center; gap: 14px; margin: 8px 0; }
  .frow label { width: 96px; flex: 0 0 auto; color: #9fb0c8; font-weight: 600; font-size: clamp(13px, 1.3vw, 15px); }
  .frow input, .frow select { flex: 1; background: #0c1320; border: 1px solid #2c3a5c; color: #eef2f8; border-radius: 9px; padding: 9px 12px; font-size: clamp(13px, 1.4vw, 16px); }
  .frow input:focus, .frow select:focus { outline: none; border-color: var(--accent); }

  .wizard { position: fixed; inset: 0; z-index: 20; display: grid; place-items: center; background: radial-gradient(1200px 800px at 50% 30%, #1a2236 0%, #05070b 70%); }
  .wstep { width: min(640px, 90vw); text-align: center; display: flex; flex-direction: column; align-items: center; gap: 16px; padding: 32px; }
  .wlogo { font-size: clamp(26px, 3.4vw, 48px); font-weight: 800; letter-spacing: 4px; color: var(--accent); }
  .wstep h2 { margin: 0; font-size: clamp(26px, 3.4vw, 44px); }
  .wlead { margin: 0; color: #aab6c9; font-size: clamp(15px, 1.7vw, 21px); max-width: 34em; line-height: 1.5; }
  .wfacts { list-style: none; padding: 0; margin: 6px 0; display: flex; flex-direction: column; gap: 8px; color: #cdd7e6; font-size: clamp(14px, 1.6vw, 20px); }
  .wfacts b { color: #fff; }
  .wswatches { display: flex; gap: 16px; margin: 8px 0; }
  .wsw { width: 56px; height: 56px; border-radius: 14px; border: 3px solid transparent; box-shadow: 0 6px 20px #0008; }
  .wsw.sel { border-color: #fff; transform: scale(1.12); }
  .wnav { margin-top: 14px; color: #7e8aa0; font-size: clamp(13px, 1.4vw, 17px); }
  .wnav b { color: var(--accent); }

  footer { padding: 7px 2.4vw; color: #5b6678; font-size: clamp(10px, 0.95vw, 13px); border-top: 1px solid #141d2e44; background: #05070b66; }
  footer b { color: #93a0b6; font-weight: 600; }
</style>
