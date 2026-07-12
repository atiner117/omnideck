<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "$lib/backend";
  import type { App, Game, Config, Capability, MediaInfo, Settings, LiveApp } from "$lib/backend";
  import Modal from "$lib/Modal.svelte";
  import NowPlaying from "$lib/NowPlaying.svelte";
  import Wizard from "$lib/Wizard.svelte";
  import HelpModal from "$lib/HelpModal.svelte";
  import Icon from "$lib/Icon.svelte";
  import Waves from "$lib/Waves.svelte";
  import MediaModal from "$lib/MediaModal.svelte";
  import { MediaNav } from "$lib/medianav.svelte";
  import SearchModal from "$lib/SearchModal.svelte";
  import DeckSwitcher from "$lib/DeckSwitcher.svelte";
  import CatalogModal from "$lib/CatalogModal.svelte";
  import LauncherForm from "$lib/LauncherForm.svelte";
  import { initSfx, sfxMove, sfxEnter } from "$lib/sfx";
  import { ambientApply, ambientStop } from "$lib/ambient";
  import { OSK_ROWS, OSK_FLAT, OSK_COLS } from "$lib/osk";
  import type { Tile } from "$lib/tiles";
  import { SETTING_DEFS, ACCENTS, normalizeNum, type SettingDef, type CycleDef, type NumDef, type TextDef } from "$lib/settings-defs";

  const CATEGORIES = [
    { id: "dashboard", label: "Home", icon: "home" },
    { id: "games", label: "Games", icon: "games" },
    { id: "video", label: "Movies & TV", icon: "video" },
    { id: "music", label: "Music", icon: "music" },
    { id: "apps", label: "Apps", icon: "apps" },
    { id: "settings", label: "Settings", icon: "settings" },
  ] as const;
  const PRESET: Record<string, number> = { small: 1.3, medium: 1.6, large: 1.9, huge: 2.3 };
  const POWER = [
    { key: "exit", label: "Exit OmniDeck", icon: "exit" },
    { key: "suspend", label: "Suspend", icon: "moon" },
    { key: "reboot", label: "Restart", icon: "restart" },
    { key: "poweroff", label: "Shut down", icon: "power" },
  ] as const;
  const CATORDER: Record<string, number> = { games: 0, video: 1, music: 2, apps: 3 };
  // Display value for a settings row — headers/actions have none, everything else reads its def.
  function settingValue(d: SettingDef): string {
    const s = cfg?.settings;
    if (!s || d.type === "header" || d.type === "action") return "";
    return d.value(s);
  }

  let cap = $state<Capability | null>(null);
  let cfg = $state<Config | null>(null);
  let inSession = $state(false); // true when running as a gamescope session (vs desktop window)
  let accent = $state("#b14cff");
  let clock = $state("");
  // Now Playing: launch-tracked entries (games + non-media apps), each cleared when the
  // backend reports that process/game exited. Media apps are enriched with live MPRIS
  // metadata (song/artist) from the `media` poll below.
  type NowEntry = { id: string; kind: string; name: string; category: string };
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
    if (media && media.status === "Playing" && !mediaShown) out.push({ id: "media", kind: "media", name: media.player || "Media", category: "music", media });
    return out.slice(0, 3);
  });

  let allGames = $state<Game[]>([]);
  let favorites = $state<string[]>([]);
  let recentApps = $state<string[]>([]); // app ids, most-recent-first
  let catSel = $state(1);
  let focus = $state(0);
  let status = $state("Loading…");
  let fps = $state(0); // current (500ms window) frame rate
  let fpsAvg = $state(0); // smoothed average
  let fpsLo = $state(9999); // worst frame since reset (the dips)
  let fpsHi = $state(0); // best frame since reset
  function resetFpsStats() { fpsAvg = 0; fpsLo = 9999; fpsHi = 0; }
  // user-facing error channel (separate from the transient `status` launch toast)
  let toastErr = $state("");
  let toastErrTimer: ReturnType<typeof setTimeout> | undefined;
  function reportError(ctx: string, e: unknown) {
    console.warn(`[omnideck] ${ctx}:`, e);
    toastErr = ctx;
    clearTimeout(toastErrTimer);
    toastErrTimer = setTimeout(() => (toastErr = ""), 5000);
  }

  // Fatal boot failures get a persistent banner with Retry/Reload — not a 5s toast. On a
  // TV there are no devtools, so the failure has to carry its own recovery affordance.
  let bootErr = $state(""); // get_config rejected: cfg stays null, settings degrade to defaults
  let libErr = $state(""); // get_library rejected: distinct from a genuinely empty library
  let bannerDismissed = $state(false);
  const bootBanner = $derived(
    bannerDismissed ? "" : bootErr || (libErr ? `Couldn't load the game library: ${libErr}` : ""),
  );
  function retryBoot() {
    bannerDismissed = false;
    if (bootErr || !cfg) loadBoot();
    else loadLibrary();
  }
  function loadLibrary() {
    libErr = "";
    api.getLibrary()
      .then((lib) => { allGames = lib.games ?? []; })
      .catch((e) => { libErr = String(e); console.warn("[omnideck] get_library failed:", e); })
      .finally(() => { status = ""; }); // banner / games empty-state carry any failure from here
  }
  function loadBoot() {
    bootErr = "";
    status = "Loading…";
    api.getConfig()
      .then((c) => {
        cfg = c;
        accent = c.settings?.accent ?? "#b14cff";
        favorites = c.favorites ?? [];
        recentApps = c.recent_apps ?? [];
        if (c.config_error) reportError(c.config_error, null); // config.toml didn't parse — warn, don't silently revert
        if (c.settings && c.settings.onboarded === false) { wizardActive = true; wizardStep = 0; }
      })
      .catch((e) => { bootErr = `Couldn't load settings: ${e}`; console.warn("[omnideck] get_config failed:", e); })
      .finally(() => {
        // art loads lazily per windowed row (see the winItems $effect), not per game here
        loadLibrary();
      });
  }

  let art = $state<Record<string, string>>({});
  let logos = $state<Record<string, string>>({});
  let gridBox = $state<Record<string, boolean>>({});
  let heroes = $state<Record<string, string>>({}); // wide hero art for the background
  let appIcons = $state<Record<string, string>>({}); // fetched site icons for web/app tiles
  let iconBg = $state<Record<string, string>>({}); // contrast-aware tile bg per fetched icon
  let iconColor = $state<Record<string, string>>({}); // dominant "r,g,b" per fetched icon (app bg gradient)
  let searchEngineIcon = $state(""); // favicon of the configured web-search provider
  const iconTried = new Set<string>();
  const artFailed = new Set<string>(); // appids whose local art file 404'd (don't re-request)
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
      const d = await api.appIcon(url);
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
      } catch { /* canvas getImageData can throw on an odd/tainted image — skip the color calc */ }
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
  // Synthetic first tile in Movies & TV when a media server is configured — opens the
  // in-app library browser instead of exec'ing anything (intercepted in launchTile).
  const MEDIA_TILE: Tile = {
    kind: "app", id: "media-library", cat: "video",
    app: { id: "media-library", name: "Media Library", icon: "🎞️", exec: [], accent: "#7b5cff", category: "video" },
  };
  let items = $derived.by<Tile[]>(() => {
    switch (catId) {
      case "dashboard": return [...allTiles.filter((t) => favorites.includes(t.id)), ...recentTiles];
      case "games": return allTiles.filter((t) => t.cat === "games");
      case "video": return [...(mediaAvail ? [MEDIA_TILE] : []), ...appTiles.filter((t) => t.cat === "video")];
      case "music": return appTiles.filter((t) => t.cat === "music");
      case "apps": return appTiles.filter((t) => t.cat === "apps");
      default: return [];
    }
  });
  // hide rows that only apply to a current selection (custom size, bg color/image, custom volume)
  let visibleSettings = $derived(
    SETTING_DEFS.filter((d) => {
      if (d.type === "header") return true; // every section keeps ≥1 unconditional row
      const set = cfg?.settings; if (!set) return true;
      return d.visible?.(set) ?? true;
    }),
  );
  let itemCount = $derived(catId === "settings" ? visibleSettings.length : items.length);
  // ---- windowed (virtualized) item rail ----
  // The rail translates so the focused row sits at the top of the clipped wrap, meaning only
  // ~[focus, focus + viewport-rows] can ever be on screen. Render just that slice — a small
  // margin above (upward-slide transition + the `near` fade) and a generous one below (covers a
  // 4K panel at the smallest UI scale, ~32 visible rows) — and preserve absolute row offsets
  // with a spacer, so each keypress costs O(window), not O(library). Art loading keys off the
  // same window: a 1,000-game library no longer fires a fetch per game at mount.
  const WIN_ABOVE = 8, WIN_BELOW = 40;
  let winLo = $derived(Math.max(0, focus - WIN_ABOVE));
  let winItems = $derived(items.slice(winLo, focus + WIN_BELOW));
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

  // ---- synthesized navigation sounds (moved to $lib/sfx — reads the live settings) ----
  initSfx(() => ({ on: !!cfg?.settings?.sound, volume: cfg?.settings?.sound_volume ?? 0.6 }));

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
  // One-shot timers tracked so onMount cleanup can cancel any still pending — avoids reactive
  // state writes after the component is gone (matters under HMR / any future routing).
  const pendingTimers = new Set<ReturnType<typeof setTimeout>>();
  function later(fn: () => void, ms: number) {
    const t = setTimeout(() => { pendingTimers.delete(t); fn(); }, ms);
    pendingTimers.add(t);
    return t;
  }
  // Build an omnideck:// URL for an on-disk art file (Steam librarycache / our art cache). The
  // webview holds the URL and decodes the file to GPU on paint — vs a base64 data URL pinned and
  // re-diffed in reactive state. Each path segment is percent-encoded; the backend (asset.rs)
  // canonicalizes + allowlists before reading. Favicons + the bg image stay on data: for now.
  function artUrl(path: string): string {
    return "omnideck://localhost" + path.split("/").map(encodeURIComponent).join("/");
  }

  // The manifest's art path can go stale (Steam moved/GC'd its librarycache): drop the 404'd
  // URL so the styled name-tile fallback shows, and tombstone the id so loadArt doesn't loop.
  function artError(appid: string) {
    artFailed.add(appid);
    const rest = { ...art };
    delete rest[appid];
    art = rest;
  }

  async function loadArt(g: Game) {
    if (!art[g.appid] && !artFailed.has(g.appid)) {
      const p = g.art_box || g.art_header || g.art_hero;
      if (p) art = { ...art, [g.appid]: artUrl(p) }; // local art: serve the file directly (no IPC)
    }
    if (!g.art_box && !gridBox[g.appid] && cfg?.settings?.steamgriddb_key) {
      try { const path = await api.gridArt(g.appid); if (path) { art = { ...art, [g.appid]: artUrl(path) }; gridBox = { ...gridBox, [g.appid]: true }; } } catch { /* SteamGridDB best-effort (no key / network) */ }
    }
    if (g.art_hero && !heroes[g.appid]) heroes = { ...heroes, [g.appid]: artUrl(g.art_hero) }; // hero bg: serve directly
  }

  // ---- navigation (XMB: left/right = category, up/down = item) ----
  // Entry focus for the current category: settings starts past its leading section header.
  function resetFocus() { focus = catId === "settings" && visibleSettings[0]?.type === "header" ? 1 : 0; }
  function moveCat(d: number) { const n = CATEGORIES.length; catSel = (catSel + d + n) % n; resetFocus(); sfxMove(); }
  function moveItem(d: number) {
    settingsEditing = false;
    if (!itemCount) return;
    let f = (focus + d + itemCount) % itemCount;
    if (catId === "settings") {
      // Skip section headers in the pressed direction (they occupy row slots but aren't rows).
      let guard = 0;
      while (visibleSettings[f]?.type === "header" && guard++ < itemCount) f = (f + d + itemCount) % itemCount;
    }
    focus = f;
    sfxMove();
  }
  function onWheel(e: WheelEvent) { e.preventDefault(); if (navGate()) moveItem(e.deltaY > 0 ? 1 : -1); }
  // Apply a partial settings change by MUTATING the reactive cfg.settings in place. Svelte 5
  // $state is fine-grained, so only the touched keys signal — this avoids the old cfg={...cfg}
  // full-rebuild that re-ran every derived (games re-sort, etc.) and re-fetched the background
  // image on every nudge. Snapshot before sending so Tauri serializes a plain object, not a proxy.
  function patchSettings(patch: Partial<Settings>) {
    if (!cfg) return;
    Object.assign(cfg.settings, patch);
    api.saveSettings($state.snapshot(cfg.settings)).catch((e) => reportError("Couldn't save settings", e));
  }
  // D-pad ◀▶ nudge on a numeric row: step by the row's meta, then its side effect (volume blip).
  function adjustSetting(d: NumDef, dir: number) {
    if (!cfg) return;
    setNum(d, d.get(cfg.settings) + dir * (d.adjustStep ?? d.step));
    d.adjusted?.();
  }
  function doAction(key: string) {
    holdStop(); // a held D-pad press that opens this modal must not keep auto-repeating behind it
    if (key === "addcustom") formOpen = true; // LauncherForm owns its drafts; mounting resets them
  }
  // numeric settings: also typeable via a real <input> while editing
  function setNum(d: NumDef, raw: number) {
    if (!cfg || Number.isNaN(raw)) return;
    patchSettings(d.set(normalizeNum(d, raw)));
  }
  // text settings (background image path, search URL)
  function setText(d: TextDef, raw: string) {
    if (!cfg) return;
    patchSettings(d.set(raw.trim()));
  }
  function onBgColor(e: Event) {
    patchSettings({ background_color: (e.target as HTMLInputElement).value });
  }
  function focusSelect(node: HTMLInputElement) { node.focus(); node.select(); }
  function isTyping() {
    const a = document.activeElement;
    return !!a && ["INPUT", "SELECT", "TEXTAREA"].includes(a.tagName);
  }
  function onAccentColor(e: Event) {
    const v = (e.target as HTMLInputElement).value;
    patchSettings({ accent: v });
    accent = v;
  }
  // horizontal: adjusts the focused numeric setting ONLY while editing; otherwise always
  // switches category (so you can never get trapped in Settings).
  function horiz(dir: number) {
    const row = visibleSettings[focus];
    if (catId === "settings" && settingsEditing && row?.type === "num") { adjustSetting(row, dir); return; }
    settingsEditing = false;
    moveCat(dir);
  }
  function activate() {
    if (catId === "settings") {
      const row = visibleSettings[focus];
      if (!row || row.type === "header") return; // section label, not a setting
      if (row.type === "num" || row.type === "text") settingsEditing = !settingsEditing; // Enter toggles edit
      else if (row.type === "action") doAction(row.key);
      else cycleSetting(row);
      return;
    }
    const t = items[focus];
    if (t) { sfxEnter(); launchTile(t); }
  }
  // ---- media library (Jellyfin browse/play — MediaModal) ----
  // Browse state + drill-down flow live in MediaNav ($lib/medianav.svelte.ts); the page
  // keeps the page-level concerns it injects: errors, hold-repeat, and the play flow
  // (status toast + Now-Playing card).
  let mediaAvail = $state(false);
  const mediaNav = new MediaNav({
    onerror: reportError,
    holdstop: holdStop,
    onplay: (id, name) => {
      status = `▶ ${name}…`;
      const key = `media-${id}`;
      api.mediaPlay(id, name)
        .then(() => { nowList = [{ id: key, kind: "app", name, category: "video" }, ...nowList.filter((e) => e.id !== key)].slice(0, 3); })
        .catch((e) => reportError("Playback failed", e));
      later(() => (status = ""), 3500);
    },
  });

  // Deck switcher (iOS-style app cards): Guide tap opens it (backend hides the apps so this
  // overlay is what shows); pick a card to bring that app forward, Select to close it.
  let deckOpen = $state(false);
  let deckApps = $state<LiveApp[]>([]);
  let deckFocus = $state(0);
  // An app's launcher icon/emoji for its card, matched by launch id then name (games show 🎮).
  function deckIcon(a: LiveApp): string {
    const app = apps.find((x) => x.id === a.id) ?? apps.find((x) => x.name === a.name);
    return app?.icon ?? "🎮";
  }
  async function openDeck() {
    try { deckApps = await api.deckOpen(); } catch (e) { deckApps = []; console.debug("[omnideck] deck open failed", e); }
    if (deckApps.length === 0) return; // nothing running — don't show an empty deck
    deckFocus = 0;
    deckOpen = true;
  }
  function closeDeck() { deckOpen = false; }
  function deckMove(d: number) { if (deckApps.length) deckFocus = clamp(deckFocus + d, 0, deckApps.length - 1); }
  async function deckSelect() {
    const a = deckApps[deckFocus];
    deckOpen = false;
    if (a) await api.deckShow(a.group).catch((e) => reportError("Couldn't open app", e));
  }
  async function deckKill() {
    const a = deckApps[deckFocus];
    if (!a) return;
    await api.deckClose(a.group).catch((e) => console.debug("[omnideck] deck close failed", e));
    deckApps = deckApps.filter((x) => x.group !== a.group);
    if (deckApps.length === 0) { deckOpen = false; return; }
    deckFocus = clamp(deckFocus, 0, deckApps.length - 1);
  }
  // Posters for the rows around the focus (windowed like the game rail's art loading).
  // Stays in the page: $effect needs a component root and artUrl is page-local.
  $effect(() => {
    if (!mediaNav.open || !mediaNav.view) return;
    const win = mediaNav.view.rows.slice(Math.max(0, mediaNav.focus - 4), mediaNav.focus + 14);
    for (const r of win) {
      if (mediaNav.posters[r.id] !== undefined) continue;
      mediaNav.posters[r.id] = ""; // inflight marker (renders the fallback glyph meanwhile)
      api.mediaPoster(r.id)
        .then((p) => { if (p) mediaNav.posters = { ...mediaNav.posters, [r.id]: artUrl(p) }; })
        .catch(() => {});
    }
  });

  async function launchTile(t: Tile) {
    if (t.kind === "app" && t.app.id === "media-library") { mediaNav.openLibrary(); return; }
    const name = t.kind === "game" ? t.game.name : t.app.name;
    const id = t.id; // tile id doubles as the launch / now-playing correlation key
    try {
      const category = t.kind === "game" ? "games" : catOf(t.app);
      if (t.kind === "game") { status = `▶ Launching ${name}…`; await api.launchGame(t.game.appid, name, id); }
      else { status = `▶ ${name}…`; await api.launchCommand(t.app.exec, name, id); recordRecentApp(t.app.id); }
      nowList = [{ id, kind: t.kind, name, category }, ...nowList.filter((e) => e.id !== id)].slice(0, 3);
    } catch (e) { status = `launch error: ${e}`; return; }
    later(() => (status = ""), 3500);
  }
  function recordRecentApp(id: string) {
    recentApps = [id, ...recentApps.filter((x) => x !== id)].slice(0, 20);
    api.saveRecentApps(recentApps).catch((e) => reportError("Couldn't save recents", e));
  }
  function gotoSettings() { catSel = CATEGORIES.findIndex((c) => c.id === "settings"); resetFocus(); }
  function goHome() { catSel = CATEGORIES.findIndex((c) => c.id === "dashboard"); resetFocus(); }

  // ---- in-app info panel (games + apps) ----
  let infoOpen = $state(false);
  let helpOpen = $state(false); // controls reference (the old footer hint wall)
  let infoTile = $state<Tile | null>(null);
  function showInfo() { holdStop(); if (catId === "settings") return; const t = items[focus]; if (t) { infoTile = t; infoOpen = true; } }
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
  // Advance a cycle row to its next state (the per-row logic lives in the def's `cycle`).
  function cycleSetting(d: CycleDef) {
    if (!cfg) return;
    const patch = d.cycle(cfg.settings);
    patchSettings(patch);
    if (patch.accent) accent = patch.accent;
  }
  // Mouse click on a settings row (headers aren't rendered as buttons, so they never get here).
  function settingRowClick(d: SettingDef, i: number) {
    focus = i;
    if (d.type === "num" || d.type === "text") settingsEditing = !settingsEditing;
    else if (d.type === "action") doAction(d.key);
    else if (d.type === "cycle") cycleSetting(d);
  }

  function isFav(id: string) { return favorites.includes(id); }
  function favCurrent() {
    if (catId === "settings") return;
    const t = items[focus]; if (!t) return;
    favorites = isFav(t.id) ? favorites.filter((x) => x !== t.id) : [...favorites, t.id];
    api.saveFavorites(favorites).catch((e) => reportError("Couldn't save favorites", e));
  }

  // ---- add-apps catalog ----
  // ---- power menu · confirm · custom-launcher form ----
  let powerOpen = $state(false);
  let powerFocus = $state(0);
  let confirmAct = $state<{ key: string; label: string } | null>(null);
  let formOpen = $state(false); // custom-launcher form ($lib/LauncherForm.svelte owns the fields)
  function openPower() { holdStop(); powerOpen = true; powerFocus = 0; }
  function powerMove(d: number) { powerFocus = clamp(powerFocus + d, 0, POWER.length - 1); }
  function powerActivate() {
    holdStop(); // stop any in-progress hold-repeat when moving to the confirm/exit step
    const key = POWER[powerFocus].key;
    powerOpen = false;
    if (key === "exit") api.quit().catch((e) => reportError("Couldn't exit", e));
    else if (key === "suspend") api.powerAction("suspend").catch((e) => reportError("Suspend failed", e));
    else confirmAct = { key, label: POWER[powerFocus].label };
  }
  function doConfirm() {
    if (!confirmAct) return;
    api.powerAction(confirmAct.key).catch((e) => reportError("Power action failed", e));
    confirmAct = null;
  }
  // LauncherForm hands back the built App; the page owns persistence and the collision toast.
  function addLauncher(app: App, collided: boolean) {
    if (!cfg) { formOpen = false; return; }
    const next = [...apps, app];
    cfg = { ...cfg, apps: next };
    api.saveApps(next).catch((e) => reportError("Couldn't save apps", e));
    if (collided) { status = `Added "${app.name}" (a similar name already existed)`; later(() => (status = ""), 3000); }
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
  function openSearch() { holdStop(); searchOpen = true; searchQuery = ""; searchFocus = 0; oskFocus = 0; oskDim = false; }
  function searchMove(d: number) {
    searchFocus = clamp(searchFocus + d, 0, searchResults.length); // last index = web-search row
    queueMicrotask(() => document.querySelector(`[data-sr="${searchFocus}"]`)?.scrollIntoView({ block: "nearest" }));
  }
  function webSearch() {
    if (!searchQuery.trim()) return;
    let prov = cfg?.settings?.search_provider || "https://duckduckgo.com/?q=";
    if (!/^https?:\/\//i.test(prov)) prov = "https://duckduckgo.com/?q="; // ignore a non-URL provider (safety + UX)
    api.launchCommand(["BROWSER", prov + encodeURIComponent(searchQuery)], "Search").catch((e) => reportError("Search failed", e));
    searchOpen = false;
    status = `🔎 ${searchQuery}`;
    later(() => (status = ""), 2500);
  }
  function searchActivate() {
    if (searchFocus < searchResults.length) { searchOpen = false; launchTile(searchResults[searchFocus]); }
    else webSearch();
  }

  // ---- on-screen keyboard (layout in $lib/osk.ts; rendered by SearchModal) ----
  let oskFocus = $state(0);
  let oskDim = $state(false); // recede while a physical keyboard is typing; back on D-pad use
  function oskMove(dx: number, dy: number) {
    const rows = OSK_ROWS.length;
    const col = ((oskFocus % OSK_COLS) + dx + OSK_COLS) % OSK_COLS;
    const row = (Math.floor(oskFocus / OSK_COLS) + dy + rows) % rows;
    oskFocus = row * OSK_COLS + col;
  }
  function oskPress(k: string) {
    if (k === "␣") searchQuery += " ";
    else if (k === "⌫") searchQuery = searchQuery.slice(0, -1);
    else if (k === "✕") searchQuery = "";
    else if (k === "⏎") searchActivate();
    else searchQuery += k;
  }
  function toggleCatalog() { holdStop(); catalogOpen = !catalogOpen; catFocus = 0; }
  function catMove(d: number) { catFocus = clamp(catFocus + d, 0, displayedCatalog.length - 1); queueMicrotask(() => document.querySelector(`[data-cat="${catFocus}"]`)?.scrollIntoView({ block: "nearest" })); }
  function isAdded(id: string) { return apps.some((a) => a.id === id); }
  async function catToggle(i: number) {
    const e = displayedCatalog[i]; if (!e || !cfg) return;
    const next = isAdded(e.id) ? apps.filter((a) => a.id !== e.id) : [...apps, e];
    cfg = { ...cfg, apps: next };
    try { await api.saveApps(next); } catch (e) { reportError("Couldn't save apps", e); }
  }

  // ---- first-run wizard ----
  let wizardActive = $state(false);
  let wizardStep = $state(0);
  function finishWizard() { wizardActive = false; patchSettings({ onboarded: true }); }
  function wizardNext() { if (wizardStep >= 2) finishWizard(); else wizardStep++; }
  function wizardPrev() { if (wizardStep > 0) wizardStep--; }
  function wizardAccent(dir: number) { if (!cfg) return; const c = ACCENTS.indexOf(cfg.settings.accent ?? "#4cc2ff"); const a = ACCENTS[((c < 0 ? 0 : c) + (dir > 0 ? 1 : ACCENTS.length - 1)) % ACCENTS.length]; patchSettings({ accent: a }); accent = a; }

  // ---- Unified input router (review #10) ----------------------------------------------------
  // One ordered roster of overlay controllers. The router walks the list and the FIRST open
  // overlay consumes the event — keyboard, gamepad buttons, and stick axes all share this single
  // ordering (previously three hand-maintained ladders that each hardcoded the list). Adding an
  // overlay = one entry here. Order is stacking order: info/help render on top of the menus they
  // can be opened over, so they come first; deck/wizard preempt everything.
  type Overlay = {
    open: () => boolean;
    key: (e: KeyboardEvent) => void; // keyboard, when this overlay is topmost
    pad: (code: string) => void; // gamepad button_pressed, when this overlay is topmost
    stickY?: (d: number) => void; // vertical-stick row move; omitted = overlay swallows the axis
    stickX?: (d: number) => void; // horizontal stick; omitted = swallowed (only the deck uses it)
    allowHelp?: boolean; // "?" / F1 still opens Help on top of this overlay
  };
  const OVERLAYS: Overlay[] = [
    {
      // Deck switcher: arrows/L-R pick a card, Enter/A/X opens, Del/Select closes it,
      // Esc/B/Guide dismisses.
      open: () => deckOpen,
      key: (e) => {
        if (e.key === "ArrowLeft" && navGate()) deckMove(-1);
        else if (e.key === "ArrowRight" && navGate()) deckMove(1);
        else if (e.key === "Enter") deckSelect();
        else if (e.key === "Delete" || e.key === "Backspace") deckKill();
        else if (e.key === "Escape") closeDeck();
      },
      pad: (c) => {
        if (c === "DPadLeft") holdStart(c, () => deckMove(-1));
        else if (c === "DPadRight") holdStart(c, () => deckMove(1));
        else if (c === "South" || c === "West") deckSelect();
        else if (c === "Select") deckKill();
        else if (c === "East" || c === "Start") closeDeck();
      },
      stickX: (d) => deckMove(d), // horizontal card row — vertical stick is meaningless here
    },
    {
      // First-run wizard
      open: () => wizardActive,
      key: (e) => {
        if (e.key === "Enter") wizardNext();
        else if (e.key === "Escape") wizardPrev();
        else if (e.key === "ArrowLeft" && wizardStep === 1 && navGate()) wizardAccent(-1);
        else if (e.key === "ArrowRight" && wizardStep === 1 && navGate()) wizardAccent(1);
      },
      pad: (c) => {
        if (c === "South") wizardNext();
        else if (c === "East") wizardPrev();
        else if (c === "DPadLeft" && wizardStep === 1) wizardAccent(-1);
        else if (c === "DPadRight" && wizardStep === 1) wizardAccent(1);
      },
    },
    {
      // Tile info sheet — any accept/back input closes it
      open: () => infoOpen,
      key: (e) => { if (e.key === "Escape" || e.key === "Enter" || e.key === "i" || e.key === "I") infoOpen = false; },
      pad: (c) => { if (c === "East" || c === "South") infoOpen = false; },
    },
    {
      // Help sheet
      open: () => helpOpen,
      key: (e) => { if (e.key === "Escape" || e.key === "Enter" || e.key === "?" || e.key === "F1") helpOpen = false; },
      pad: (c) => { if (c === "East" || c === "South") helpOpen = false; },
    },
    {
      // Custom-launcher form: native inputs handle typing; only back-out is routed
      open: () => formOpen,
      key: (e) => { if (e.key === "Escape") formOpen = false; },
      pad: (c) => { if (c === "East") formOpen = false; },
      allowHelp: true,
    },
    {
      // Power confirm dialog
      open: () => !!confirmAct,
      key: (e) => { if (e.key === "Enter") doConfirm(); else if (e.key === "Escape") confirmAct = null; },
      pad: (c) => { if (c === "South") doConfirm(); else if (c === "East") confirmAct = null; },
      allowHelp: true,
    },
    {
      // Power menu
      open: () => powerOpen,
      key: (e) => {
        if (e.key === "ArrowUp" && navGate()) powerMove(-1);
        else if (e.key === "ArrowDown" && navGate()) powerMove(1);
        else if (e.key === "Enter") powerActivate();
        else if (e.key === "Escape") powerOpen = false;
      },
      pad: (c) => {
        if (c === "DPadUp") holdStart(c, () => powerMove(-1));
        else if (c === "DPadDown") holdStart(c, () => powerMove(1));
        else if (c === "South") powerActivate();
        else if (c === "East") powerOpen = false;
      },
      stickY: powerMove,
      allowHelp: true,
    },
    {
      // Media library
      open: () => mediaNav.open,
      key: (e) => {
        if (e.key === "ArrowUp" && navGate()) mediaNav.move(-1);
        else if (e.key === "ArrowDown" && navGate()) mediaNav.move(1);
        else if (e.key === "Enter") mediaNav.activate();
        else if (e.key === "Escape" || e.key === "Backspace") mediaNav.back();
      },
      pad: (c) => {
        if (c === "DPadUp") holdStart(c, () => mediaNav.move(-1));
        else if (c === "DPadDown") holdStart(c, () => mediaNav.move(1));
        else if (c === "South") mediaNav.activate();
        else if (c === "East") mediaNav.back();
      },
      stickY: (d) => mediaNav.move(d),
      allowHelp: true,
    },
    {
      // Search: D-pad drives the on-screen keyboard; bumpers move the result selection.
      open: () => searchOpen,
      key: (e) => {
        if (e.key === "ArrowUp") searchMove(-1);
        else if (e.key === "ArrowDown") searchMove(1);
        else if (e.key === "Enter") searchActivate();
        else if (e.key === "Escape") { if (searchQuery) searchQuery = ""; else searchOpen = false; }
        else if (e.key === "Backspace") { searchQuery = searchQuery.slice(0, -1); oskDim = true; }
        // preventDefault so Space can't ALSO natively re-activate a mouse-focused result row
        else if (e.key.length === 1 && /^[\w .\-]$/.test(e.key)) { e.preventDefault(); searchQuery += e.key; oskDim = true; }
      },
      pad: (c) => {
        if (["DPadUp", "DPadDown", "DPadLeft", "DPadRight", "South"].includes(c)) oskDim = false;
        if (c === "DPadUp") holdStart(c, () => oskMove(0, -1));
        else if (c === "DPadDown") holdStart(c, () => oskMove(0, 1));
        else if (c === "DPadLeft") holdStart(c, () => oskMove(-1, 0));
        else if (c === "DPadRight") holdStart(c, () => oskMove(1, 0));
        else if (c === "South") oskPress(OSK_FLAT[oskFocus]);
        else if (c === "LeftTrigger") searchMove(-1);
        else if (c === "RightTrigger") searchMove(1);
        else if (c === "West") searchQuery = searchQuery.slice(0, -1);
        else if (c === "East") { if (searchQuery) searchQuery = ""; else searchOpen = false; }
      },
      stickY: searchMove, // stick moves the result row; the OSK stays D-pad-only
      allowHelp: true,
    },
    {
      // App catalog
      open: () => catalogOpen,
      key: (e) => {
        if (e.key === "ArrowUp" && navGate()) catMove(-1);
        else if (e.key === "ArrowDown" && navGate()) catMove(1);
        else if (e.key === "Enter") catToggle(catFocus);
        else if (e.key === "Tab") { e.preventDefault(); catSort = catSort === "group" ? "alpha" : "group"; }
        else if (e.key === "Escape") { if (catQuery) catQuery = ""; else catalogOpen = false; }
        else if (e.key === "Backspace") catQuery = catQuery.slice(0, -1);
        // preventDefault so Space can't ALSO natively re-toggle a mouse-focused catalog row
        else if (e.key.length === 1 && /^[a-z0-9 ]$/i.test(e.key)) { e.preventDefault(); catQuery += e.key; }
      },
      pad: (c) => {
        if (c === "DPadUp") holdStart(c, () => catMove(-1));
        else if (c === "DPadDown") holdStart(c, () => catMove(1));
        else if (c === "South") catToggle(catFocus);
        else if (c === "North") catSort = catSort === "group" ? "alpha" : "group"; // toggle sort
        else if (c === "East") { if (catQuery) catQuery = ""; else catalogOpen = false; }
      },
      stickY: catMove,
      allowHelp: true,
    },
  ];
  const topOverlay = () => OVERLAYS.find((o) => o.open());
  // Single source of truth: is any modal/overlay open? Gates base navigation and stops
  // hold-repeat the instant a modal opens (derives straight from the roster).
  const anyModal = $derived(OVERLAYS.some((o) => o.open()));

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
    const ov = topOverlay();
    // "?" opens Help even on top of the list modals (which type their own charsets and never
    // consume it) — but not over the deck/wizard/info/help, which own all their input.
    if ((e.key === "?" || e.key === "F1") && (!ov || ov.allowHelp)) { e.preventDefault(); holdStop(); helpOpen = true; return; }
    if (ov) { ov.key(e); return; }
    if (e.key === "/") { e.preventDefault(); openSearch(); return; }
    if (e.key === "a" || e.key === "A") { toggleCatalog(); return; }
    if (e.key === "p" || e.key === "P") { gotoSettings(); return; }
    if (e.key === "h" || e.key === "H") { goHome(); return; }
    if (e.key === "f" || e.key === "F") { favCurrent(); return; }
    if (e.key === "i" || e.key === "I") { showInfo(); return; }
    if ((e.key === "r" || e.key === "R") && (bootErr || libErr)) { retryBoot(); return; }
    if (e.key === "Escape") { if (bootBanner) { bannerDismissed = true; return; } settingsEditing = false; return; }
    if (e.key === "ArrowLeft" && navGate()) horiz(-1);
    else if (e.key === "ArrowRight" && navGate()) horiz(1);
    else if (e.key === "ArrowUp" && navGate()) moveItem(-1);
    else if (e.key === "ArrowDown" && navGate()) moveItem(1);
    else if (e.key === "Enter") activate();
  }

  onMount(() => {
    window.addEventListener("keydown", onKey);
    api.getCapability().then((c) => (cap = c)).catch((e) => reportError("Capability probe failed", e));
    // adds the Media Library tile; a rejection here means the IPC itself failed (the command
    // is infallible), so surface it instead of silently never showing the tile
    api.mediaAvailable().then((v) => (mediaAvail = v)).catch((e) => reportError("Media server check failed", e));
    api.inGamescopeSession().then((v) => (inSession = v)).catch((e) => console.debug("[omnideck] inGamescopeSession probe failed", e));
    api.getCatalog().then((c) => (catalog = c)).catch((e) => reportError("Couldn't load app catalog", e));
    loadBoot();

    // Per-frame sampling catches brief dips a 500ms average smooths away; we only commit the
    // numbers to reactive state once per 500ms window so the tracker adds no per-frame cost.
    let raf = 0, frames = 0, winStart = performance.now(), lastFrame = winStart;
    const warmupEnd = winStart + 600; // skip the first frames (long initial frame) for lo/hi
    let loAcc = 9999, hiAcc = 0, avgAcc = 0, avgN = 0;
    const loop = (t: number) => {
      const dt = t - lastFrame; lastFrame = t;
      if (dt > 0 && t > warmupEnd) {
        const inst = Math.min(1000 / dt, 240);
        if (inst < loAcc) loAcc = inst;
        if (inst > hiAcc) hiAcc = inst;
        avgAcc += inst; avgN++;
      }
      frames++;
      if (t - winStart >= 500) {
        fps = Math.round((frames * 1000) / (t - winStart));
        if (avgN) fpsAvg = fpsAvg ? Math.round(fpsAvg * 0.7 + (avgAcc / avgN) * 0.3) : Math.round(avgAcc / avgN);
        if (loAcc < fpsLo) fpsLo = Math.round(loAcc); // session watermarks (persist until reset)
        if (hiAcc > fpsHi) fpsHi = Math.round(hiAcc);
        loAcc = 9999; hiAcc = 0; avgAcc = 0; avgN = 0; frames = 0; winStart = t;
        clock = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
      }
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);

    const off: Array<() => void> = [];
    // We add to nowList when we launch (we know game vs app there); the backend tells us when
    // the process/game exits — correlate by the launch id (the tile id), not the display name.
    api.onAppExited((e) => {
      const id = String(e.payload ?? "");
      nowList = nowList.filter((x) => x.id !== id);
      // Keep the deck honest if an app dies while it's open (e.g. we just closed one).
      if (deckOpen) api.deckList().then((a) => { deckApps = a; if (a.length === 0) deckOpen = false; else deckFocus = clamp(deckFocus, 0, a.length - 1); }).catch(() => {});
    }).then((u) => off.push(u));
    // Guide tap (gamepad) or Ctrl+Alt+Home → toggle the deck switcher.
    api.onGuideTap(() => { if (deckOpen) closeDeck(); else openDeck(); }).then((u) => off.push(u));
    // MPRIS Now Playing is event-driven (backend zbus watcher). One initial fetch covers the
    // window between mount and the listener attaching; after that, `media-changed` pushes
    // every track/status change in ms (works for native players + browser PWAs).
    const applyMedia = (m: MediaInfo | null) => { media = m && m.status !== "Stopped" ? m : null; };
    api.mediaNowPlaying().then(applyMedia).catch((e) => console.debug("[omnideck] media fetch failed", e));
    api.onMediaChanged((e) => applyMedia(e.payload)).then((u) => off.push(u));
    api.onGamepad((e) => {
      const p = e.payload;
      if (p.kind === "button_pressed") {
        const ov = topOverlay();
        if (ov) { ov.pad(p.code); return; }
        if (p.code === "North") { toggleCatalog(); return; }
        if (p.code === "Select") { openSearch(); return; }
        if (p.code === "Start") { goHome(); return; }
        if (p.code === "West") { favCurrent(); return; }
        if (p.code === "RightTrigger") { showInfo(); return; }
        if (p.code === "LeftTrigger" && (bootErr || libErr)) { retryBoot(); return; } // banner retry (LT is unused at base)
        if (p.code === "East") { if (bootBanner) { bannerDismissed = true; return; } settingsEditing = false; return; }
        if (p.code === "DPadLeft") holdStart(p.code, () => horiz(-1));
        else if (p.code === "DPadRight") holdStart(p.code, () => horiz(1));
        else if (p.code === "DPadUp") holdStart(p.code, () => moveItem(-1));
        else if (p.code === "DPadDown") holdStart(p.code, () => moveItem(1));
        else if (p.code === "South") activate();
      } else if (p.kind === "button_released") {
        if (p.code === heldCode) holdStop();
      } else if (p.kind === "axis_changed" && (p.code === "LeftStickX" || p.code === "LeftStickY")) {
        // One deadzone (no 0.3–0.6 dead band) + the same hold-repeat the D-pad uses. Track the
        // active axis:direction so a held stick auto-repeats once, and recentering or an
        // unhandled overlay opening stops it — fixes drift-stuck nav and phantom nav.
        const DZ = 0.6;
        const raw = p.value > DZ ? 1 : p.value < -DZ ? -1 : 0;
        // gilrs convention: positive Y = stick pushed UP (M2 finding: a DS4 navigated
        // upside-down because this was consumed unnegated). List/rail "down" is +1.
        const dir = p.code === "LeftStickY" ? -raw : raw;
        const code = `${p.code}:${dir}`;
        if (dir === 0) { if (heldCode.startsWith(p.code)) holdStop(); return; }
        // The stick drives whatever the topmost overlay declares for that axis (stickY = row
        // selection, stickX = deck cards; the D-pad keeps its modal-specific job, e.g. the OSK
        // in search). An open overlay with no declared handler swallows the axis.
        const ov = topOverlay();
        const fn = ov ? (p.code === "LeftStickY" ? ov.stickY : ov.stickX) : p.code === "LeftStickY" ? moveItem : horiz;
        if (!fn) { holdStop(); return; }
        if (heldCode !== code) holdStart(code, () => fn(dir));
      }
    }).then((u) => off.push(u));

    return () => {
      window.removeEventListener("keydown", onKey);
      cancelAnimationFrame(raf);
      clearTimeout(toastErrTimer);
      clearTimeout(bgTimer);
      pendingTimers.forEach(clearTimeout);
      holdStop();
      ambientStop();
      off.forEach((u) => u());
    };
  });

  $effect(() => { if (focus >= itemCount && itemCount) focus = itemCount - 1; });
  $effect(() => { if (catFocus >= displayedCatalog.length) catFocus = Math.max(0, displayedCatalog.length - 1); });
  // fetch site icons for visible web/app tiles + the add-apps catalog (cached on disk)
  $effect(() => { for (const t of items) if (t.kind === "app") loadAppIcon(t.app); });
  $effect(() => { for (const c of displayedCatalog) loadAppIcon(c); });
  // game art only for windowed rows — scrolling pulls art in just ahead of visibility
  $effect(() => { for (const t of winItems) if (t.kind === "game") loadArt(t.game); });
  // load the custom background image (data URL) when that mode is selected
  let bgSeq = 0;
  $effect(() => {
    const path = cfg?.settings?.background_image;
    if (cfg?.settings?.background_default === "image" && path) {
      const seq = ++bgSeq; // drop a stale resolve if the path changed before this one returned
      // Prefer the downscaled, display-sized cache (served over omnideck://, cheap to decode);
      // fall back to the full-image data URL if preparation failed (bad/unreadable source).
      api.bgImage(path)
        .then((p) => p ? artUrl(p) : api.getArt(path))
        .then((d) => { if (seq === bgSeq) bgImageUrl = d ?? ""; })
        .catch((e) => { if (seq === bgSeq) bgImageUrl = ""; console.debug("[omnideck] bg image load failed", e); });
    } else { bgImageUrl = ""; }
  });
  // Ambient pad follows its settings; idempotent, so this is safe to run on every change.
  $effect(() => {
    ambientApply(cfg?.settings?.ambient ?? false, cfg?.settings?.ambient_volume ?? 0.35);
  });
  // fetch the current web-search provider's favicon (shown on the search "web" row)
  let provSeq = 0;
  $effect(() => {
    const prov = cfg?.settings?.search_provider;
    if (prov) { const seq = ++provSeq; api.appIcon(prov).then((d) => { if (seq === provSeq) searchEngineIcon = d ?? ""; }).catch((e) => console.debug("[omnideck] search-engine favicon fetch failed", e)); }
  });
</script>

<main style="--accent:{accent}; --scale:{scaleNum}; --bg-blur:{cfg?.settings?.bg_blur ?? 0}px; --bg-bright:{cfg?.settings?.bg_brightness ?? 0.82}; background-color:{cfg?.settings?.background_color ?? '#05070b'}">
  {#if baseImageShown}<div class="xbg base has" style="background-image:url({bgImageUrl})"></div>{/if}
  <div class="xbg" class:has={!!overlay} class:wash={overlay?.kind === "wash"}
    style={overlay?.kind === "art" ? `background-image:url(${overlay.url})`
      : overlay?.kind === "wash" ? `background-image:radial-gradient(120% 90% at 75% 25%, rgba(${overlay.color},0.55) 0%, rgba(${overlay.color},0.18) 38%, transparent 72%)`
      : ""}></div>
  {#if cfg?.settings?.live_wallpaper === "waves"}<Waves {accent} />{/if}
  <div class="xbg-fade" class:dim={!hasImagery}></div>

  <header>
    <div class="brand">OMNIDECK</div>
    <div class="meta">
      <span class="clock">{clock}</span>
      <button class="badge gear" onclick={openSearch} title="Search (/)" aria-label="Search"><Icon name="search" /></button>
      <button class="badge gear" onclick={toggleCatalog} title="Add apps (A / Triangle)" aria-label="Add apps"><Icon name="plus" /></button>
      <button class="badge gear" onclick={gotoSettings} title="Settings (P)" aria-label="Settings"><Icon name="settings" /></button>
      <button class="badge gear" onclick={openPower} title="Power" aria-label="Power menu"><Icon name="power" /></button>
    </div>
  </header>

  <!-- XMB cross -->
  <div class="xmb">
    <div class="xcats" style="transform: translateX(calc(30vw - {catSel} * var(--cw)))">
      {#each CATEGORIES as c, i}
        <button class="xcat" class:sel={i === catSel} onclick={() => { catSel = i; resetFocus(); }}>
          <span class="xcicon"><Icon name={c.icon} /></span>
          {#if i === catSel}<span class="xclabel">{c.label}</span>{/if}
        </button>
      {/each}
    </div>

    <div class="xitems-wrap" onwheel={onWheel}>
      {#if catId === "settings"}
        <div class="xitems" style="transform: translateY(calc({-focus} * var(--ih)))">
          {#each visibleSettings as s, i}
            {#if s.type === "header"}
              <div class="xitem xshead" aria-hidden="true"><span class="xthumb settings hollow"></span><span class="xsheadlbl">{s.label}</span></div>
            {:else}
            <button class="xitem" class:focused={i === focus} class:editing={settingsEditing && i === focus && (s.type === "num" || s.type === "text")}
              onclick={() => settingRowClick(s, i)}>
              <span class="xthumb settings"><span class="xemoji">{s.type === "action" ? "+" : "›"}</span></span>
              <span class="xname">{s.label}
                {#if s.type === "num" && settingsEditing && i === focus}
                  <input class="numedit" type="number" use:focusSelect value={s.get(cfg?.settings)} step={s.step} min={s.lo} max={s.hi}
                    onchange={(e) => setNum(s, parseFloat((e.target as HTMLInputElement).value))} onclick={(e) => e.stopPropagation()} />
                  <span class="xsub">◀▶ or type · Enter</span>
                {:else if s.type === "text" && settingsEditing && i === focus}
                  <input class="textedit" type="text" use:focusSelect value={s.get(cfg?.settings)} placeholder="/path/to/image.jpg"
                    onchange={(e) => setText(s, (e.target as HTMLInputElement).value)} onclick={(e) => e.stopPropagation()} />
                  <span class="xsub">type a path · Enter</span>
                {:else}
                  <span class="xsub">{settingValue(s)}{s.type === "num" || s.type === "text" ? "  (Enter)" : ""}</span>
                {/if}
                {#if s.key === "accent"}<span class="swatch" style="background:{accent}"></span><input class="cwheel" type="color" value={accent} oninput={onAccentColor} onclick={(e) => e.stopPropagation()} />{/if}
                {#if s.key === "bgcolor"}<span class="swatch" style="background:{cfg?.settings?.background_color ?? '#05070b'}"></span><input class="cwheel" type="color" value={cfg?.settings?.background_color ?? '#05070b'} oninput={onBgColor} onclick={(e) => e.stopPropagation()} />{/if}
              </span>
            </button>
            {/if}
          {/each}
        </div>
      {:else if !items.length}
        <div class="xempty">
          {#if catId === "dashboard"}Nothing pinned — press <b>□ / F</b> on a tile to add it here.
          {:else if catId === "games" && libErr}Couldn't load the game library — press <b>R / LT</b> to retry.
          {:else if catId === "games"}No games found.
          {:else}Empty — press <b>△ / A</b> to add apps & media.{/if}
        </div>
      {:else}
        <div class="xitems" style="transform: translateY(calc({-focus} * var(--ih)))">
          {#if winLo > 0}<div class="xpad" style="height: calc({winLo} * var(--ih))" aria-hidden="true"></div>{/if}
          {#each winItems as t, wi (t.id)}
            {@const i = wi + winLo}
            <button class="xitem" class:focused={i === focus} class:near={Math.abs(i - focus) <= 4}
              onclick={() => { focus = i; launchTile(t); }}>
              <span class="xthumb" style={t.kind === "app" ? `background:${appIcons[t.app.id] ? (iconBg[t.app.id] ?? "#f4f5f8") : t.app.accent}` : ""}>
                {#if t.kind === "game" && art[t.game.appid]}
                  <img src={art[t.game.appid]} alt="" decoding="async" onerror={() => artError(t.game.appid)} />
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
    <SearchModal
      query={searchQuery}
      focus={searchFocus}
      results={searchResults}
      {oskFocus}
      {oskDim}
      {appIcons}
      {iconBg}
      engineIcon={searchEngineIcon}
      onfocus={(i) => (searchFocus = i)}
      onactivate={searchActivate}
      onwebsearch={webSearch}
      onoskfocus={(i) => (oskFocus = i)}
      onoskpress={oskPress}
      onclose={() => (searchOpen = false)}
    />
  {/if}

  {#if catalogOpen}
    <CatalogModal
      entries={displayedCatalog}
      focus={catFocus}
      query={catQuery}
      sort={catSort}
      {appIcons}
      {iconBg}
      {isAdded}
      onfocus={(i) => (catFocus = i)}
      ontoggle={catToggle}
      onsortswap={() => (catSort = catSort === "group" ? "alpha" : "group")}
      onclose={() => (catalogOpen = false)}
    />
  {/if}

  {#if helpOpen}
    <HelpModal {inSession} onclose={() => (helpOpen = false)} />
  {/if}

  {#if mediaNav.open}
    <MediaModal
      title={mediaNav.view?.title ?? "Media"}
      rows={mediaNav.view?.rows ?? []}
      focus={mediaNav.focus}
      posters={mediaNav.posters}
      loading={mediaNav.loading}
      depth={mediaNav.stack.length}
      onfocus={(i) => (mediaNav.focus = i)}
      onactivate={() => mediaNav.activate()}
      onclose={() => (mediaNav.open = false)}
    />
  {/if}

  {#if deckOpen}
    <DeckSwitcher apps={deckApps} focus={deckFocus} iconFor={deckIcon}
      onfocus={(i) => (deckFocus = i)} onselect={deckSelect} onkill={deckKill} onclose={closeDeck} />
  {/if}

  {#if infoOpen && infoTile}
    <Modal labelledby="dlg-info" backdropLabel="Close info" closeLabel="Close info" onclose={() => (infoOpen = false)}>
      {#if infoTile.kind === "game"}
        <h2 id="dlg-info">{infoTile.game.name}</h2>
        <dl class="infogrid">
          <dt>Type</dt><dd>Steam game</dd>
          <dt>App ID</dt><dd>{infoTile.game.appid}</dd>
          <dt>Installed in</dt><dd>{infoTile.game.library_path}/steamapps/common/{infoTile.game.installdir}</dd>
          <dt>Last played</dt><dd>{fmtPlayed(infoTile.game.last_played)}</dd>
          <dt>Status</dt><dd>{infoTile.game.installed ? "Installed" : "Not installed"}</dd>
        </dl>
        <div class="confirm-btns">
          <button class="cbtn danger" onclick={() => { const t = infoTile; infoOpen = false; if (t) launchTile(t); }}>▶ Launch</button>
          <button class="cbtn" onclick={() => { if (infoTile?.kind === "game") api.gameProperties(infoTile.game.appid).catch((e) => reportError("Couldn't open Steam properties", e)); }}>Steam properties</button>
        </div>
        <p class="phint">Steam properties opens Steam (for launch options / verify). Esc/◯ close.</p>
      {:else}
        <h2 id="dlg-info">{infoTile.app.name}</h2>
        <dl class="infogrid">
          <dt>Category</dt><dd>{infoTile.cat}</dd>
          <dt>Source</dt><dd>{appSource(infoTile.app)}</dd>
        </dl>
        <div class="confirm-btns">
          <button class="cbtn danger" onclick={() => { const t = infoTile; infoOpen = false; if (t) launchTile(t); }}>▶ Launch</button>
        </div>
        <p class="phint">Esc/◯ close · □/F favorite</p>
      {/if}
    </Modal>
  {/if}

  {#if powerOpen}
    <Modal labelledby="dlg-power" backdropLabel="Close power menu" closeLabel="Close power menu" onclose={() => (powerOpen = false)}>
      <h2 id="dlg-power">Power</h2>
      <div class="catlist">
        {#each POWER as p, i}
          <button type="button" class="crow" class:focused={i === powerFocus} onmouseenter={() => (powerFocus = i)} onclick={() => { powerFocus = i; powerActivate(); }}>
            <span class="cicon" style="background:#22304a"><Icon name={p.icon} /></span>
            <span class="cname">{p.key === "exit" && inSession ? "Log out" : p.label}</span>
          </button>
        {/each}
      </div>
      <p class="phint">↑↓ select · Enter/✕ choose · Esc/◯ close</p>
    </Modal>
  {/if}

  {#if confirmAct}
    <Modal labelledby="dlg-confirm" backdropLabel="Cancel" showClose={false} onclose={() => (confirmAct = null)}>
      <h2 id="dlg-confirm">{confirmAct.label}?</h2>
      <p class="wlead">This will {confirmAct.key === "reboot" ? "restart" : "shut down"} the computer.</p>
      <div class="confirm-btns">
        <button class="cbtn" onclick={() => (confirmAct = null)}>Cancel</button>
        <button class="cbtn danger" onclick={doConfirm}>{confirmAct.label}</button>
      </div>
      <p class="phint">Enter/✕ confirm · Esc/◯ cancel</p>
    </Modal>
  {/if}

  {#if formOpen}
    <LauncherForm {apps} onadd={addLauncher} onerror={reportError} onclose={() => (formOpen = false)} />
  {/if}

  {#if wizardActive && cfg}
    <Wizard step={wizardStep} tier={cap?.tier ?? null} gamesCount={games.length} catalogCount={catalog.length}
      accents={ACCENTS} accent={cfg.settings.accent ?? "#4cc2ff"} />
  {/if}

  <NowPlaying cards={nowCards} {inSession} onerror={reportError}
    ondismiss={(id) => (nowList = nowList.filter((x) => x.id !== id))} />

  {#if status}<div class="toast">{status}</div>{/if}
  {#if toastErr}<div class="toast err" role="alert" aria-live="assertive">⚠ {toastErr}</div>{/if}

  {#if bootBanner}
    <!-- persistent (not auto-hiding) banner for fatal boot failures — the UI is running on
         defaults / an empty library and the user needs an on-screen way to recover -->
    <div class="ebanner" role="alert" aria-live="assertive">
      <span class="ebmsg" title={bootBanner}>⚠ {bootBanner}</span>
      <span class="ebactions">
        <button class="ebtn" onclick={retryBoot}>Retry <kbd>R / LT</kbd></button>
        <button class="ebtn" onclick={() => location.reload()}>Reload</button>
        <button class="ebtn dim" onclick={() => (bannerDismissed = true)}>Dismiss <kbd>Esc / ◯</kbd></button>
      </span>
    </div>
  {/if}

  <footer>
    <span class="fdiag"><button class="fpsbtn" title="frame rate (current · avg · low · high) — click to reset lo/hi" onclick={resetFpsStats}>fps {fps} · avg {fpsAvg} · lo {fpsLo > 999 ? "—" : fpsLo} · hi {fpsHi}</button> · {cap?.tier ?? "?"}</span>
    <span class="fhints"><b>Enter/✕</b> select · <b>Esc/◯</b> back · <button class="fhelp" onclick={() => { holdStop(); helpOpen = true; }}><b>?</b> help</button></span>
  </footer>
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
  .xpad { flex: 0 0 auto; } /* offset spacer for rows above the rendered window */
  .xitem { height: var(--ih); display: flex; align-items: center; gap: 1rem; background: none; border: 0; color: #c2cbdb; cursor: pointer; text-align: left; opacity: .42; transition: opacity .12s, transform .12s; padding: 0 10px; border-radius: 12px; }
  .xitem.near { opacity: .72; }
  .xitem.focused { opacity: 1; transform: translateX(14px) scale(1.2); transform-origin: left center; }
  .xitem.editing { background: color-mix(in srgb, var(--accent) 16%, transparent); }
  .xthumb { width: calc(3.1rem * var(--scale)); height: calc(3.1rem * var(--scale)); border-radius: 10px; flex: 0 0 auto; overflow: hidden; display: grid; place-items: center; background: #1a2233; box-shadow: 0 4px 14px #0007; }
  /* Settings section headers: one row slot (the column transform positions by focus × --ih),
     label aligned with row names via an invisible thumb-width spacer. */
  .xthumb.hollow { background: none; box-shadow: none; }
  .xitem.xshead { opacity: 1; cursor: default; align-items: flex-end; padding-bottom: 6px; }
  .xsheadlbl { color: #6b7790; font-size: clamp(11px, 1.1vw, 13px); text-transform: uppercase; letter-spacing: 2px; font-weight: 700; }
  .xthumb img { width: 100%; height: 100%; object-fit: cover; }
  .xthumb img.appicon { object-fit: contain; padding: 18%; box-sizing: border-box; }
  .xthumb .xemoji { font-size: calc(1.5rem * var(--scale)); }
  .xitem.focused .xthumb { box-shadow: 0 0 0 2px var(--accent), 0 8px 24px #000a; }
  .xname { font-size: calc(clamp(16px, 1.7vw, 24px) * var(--scale)); font-weight: 600; display: flex; align-items: center; gap: 10px; }
  .xitem.focused .xname { font-weight: 800; }
  .xname .xsub { color: var(--accent); font-weight: 700; font-size: .8em; }
  .xfav { font-size: .8em; }
  .swatch { width: 30px; height: 18px; border-radius: 5px; display: inline-block; border: 1px solid #ffffff44; }
  .numedit { width: 5em; background: #0c1320; border: 1px solid var(--accent); color: #fff; border-radius: 7px; padding: 2px 8px; font-size: .8em; font-weight: 700; }
  .textedit { width: 18em; max-width: 40vw; background: #0c1320; border: 1px solid var(--accent); color: #fff; border-radius: 7px; padding: 2px 8px; font-size: .8em; }
  /* Suppress the default focus ring on elements that already show focus another way (inputs'
     accent border, the in-app .focused highlight used by controller/mouse nav)... */
  .numedit:focus, .textedit:focus,
  .badge:focus, .fpsbtn:focus,
  .xcat:focus, .xitem:focus { outline: none; }
  /* ...but show a clear accent ring for keyboard users (:focus-visible only).
     (.crow/.oskkey/.sortbtn/.cbtn rules live with the modal vocabulary in Modal.svelte.) */
  .numedit:focus-visible, .textedit:focus-visible,
  .badge:focus-visible, .fpsbtn:focus-visible,
  .xcat:focus-visible, .xitem:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .xempty { position: absolute; top: calc(16% + 7rem * var(--scale)); left: 30vw; right: 4vw; color: #8a96ab; font-size: clamp(15px, 1.8vw, 22px); }
  .xempty b { color: var(--accent); }

  .toast { position: fixed; bottom: 7vh; left: 50%; transform: translateX(-50%); background: var(--accent); color: #04121f; font-weight: 700; padding: 12px 28px; border-radius: 999px; box-shadow: 0 10px 40px color-mix(in srgb, var(--accent) 38%, transparent); font-size: clamp(14px, 1.6vw, 20px); }
  .toast.err { background: #c0392b; color: #fff; bottom: calc(7vh + 58px); box-shadow: 0 10px 40px #c0392b66; }
  .ebanner {
    position: fixed; top: 2vh; left: 50%; transform: translateX(-50%); z-index: 40;
    display: flex; gap: 16px; align-items: center; max-width: 80vw;
    background: #2a1216f2; border: 1px solid #ff5c6c66; color: #ffd7db;
    padding: 10px 18px; border-radius: 14px; box-shadow: 0 12px 40px #000a;
    font-size: clamp(13px, 1.4vw, 17px);
  }
  .ebmsg { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ebactions { display: flex; gap: 8px; flex-shrink: 0; }
  .ebtn { background: #ffffff14; border: 1px solid #ffffff2e; color: #fff; border-radius: 8px; padding: 6px 12px; font: inherit; cursor: pointer; }
  .ebtn:hover { background: #ffffff24; }
  .ebtn.dim { opacity: 0.7; }
  .ebtn kbd { font: inherit; font-size: 0.8em; opacity: 0.75; margin-left: 4px; }

  /* Now Playing card styles live in $lib/NowPlaying.svelte */
  /* Modal shell (.prefs*, backdrop, close) AND the shared modal-content vocabulary
     (.catlist/.crow/.cicon/.csearch/.phint/.osk* …) live in $lib/Modal.svelte — the
     in-page dialogs below (power, info, confirm, form) use those classes too. */
  .cwheel { width: 30px; height: 22px; padding: 0; border: 1px solid #ffffff55; border-radius: 5px; background: none; cursor: pointer; }
  .cwheel::-webkit-color-swatch-wrapper { padding: 0; }
  .cwheel::-webkit-color-swatch { border: none; border-radius: 4px; }

  .infogrid { display: grid; grid-template-columns: max-content 1fr; gap: 6px 18px; margin: 6px 0 8px; }
  .infogrid dt { color: #7e8aa0; font-size: clamp(12px, 1.2vw, 14px); font-weight: 700; }
  .infogrid dd { margin: 0; color: #dde5f0; font-size: clamp(12px, 1.3vw, 15px); word-break: break-word; }
  /* .confirm-btns / .cbtn moved to the shared modal vocabulary in $lib/Modal.svelte
     (the info + confirm dialogs here and LauncherForm all draw from it); .frow lives
     in $lib/LauncherForm.svelte. */

  /* wizard styles live in $lib/Wizard.svelte; .wlead stays — the confirm modal uses it too */
  .wlead { margin: 0; color: #aab6c9; font-size: clamp(15px, 1.7vw, 21px); max-width: 34em; line-height: 1.5; }

  footer { display: flex; justify-content: space-between; align-items: center; gap: 16px; padding: 7px 2.4vw; color: #8a96ab; font-size: clamp(10px, 0.95vw, 13px); border-top: 1px solid #141d2e44; background: #05070b66; }
  footer b { color: #93a0b6; font-weight: 600; }
  .fdiag { opacity: 0.75; }
  .fhints { white-space: nowrap; }
  .fhelp { background: none; border: 0; padding: 0; color: inherit; font: inherit; cursor: pointer; }
  .fhelp:hover b, .fhelp:hover { color: var(--accent); }
  .fhelp:focus { outline: none; }
  .fhelp:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .fpsbtn { background: none; border: 0; color: inherit; font: inherit; cursor: pointer; padding: 0; font-variant-numeric: tabular-nums; }
  .fpsbtn:hover { color: var(--accent); }

  /* Respect reduced-motion: stop the looping Now-Playing spinner/EQ and the XMB slide/scale
     transitions for vestibular-sensitive users (the UI stays fully functional, just static).
     (The deck switcher's own reduced-motion rule lives in DeckSwitcher.svelte.) */
  @media (prefers-reduced-motion: reduce) {
    .xcats, .xitems, .xbg, .xitem, .xcat .xcicon { transition: none !important; }
  }
</style>
