<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "$lib/backend";
  import type { App, Game, Config, Capability, MediaInfo, Settings, LiveApp } from "$lib/backend";
  import { clamp, railWindow } from "$lib/nav";
  import { mintLaunchId, baseId } from "$lib/launchId";
  import { cardActions, type NpAction } from "$lib/npActions";
  import Modal from "$lib/Modal.svelte";
  import NowPlaying from "$lib/NowPlaying.svelte";
  import Wizard from "$lib/Wizard.svelte";
  import HelpModal from "$lib/HelpModal.svelte";
  import Icon from "$lib/Icon.svelte";
  import Waves from "$lib/Waves.svelte";
  import MediaModal, { type MediaRow } from "$lib/MediaModal.svelte";
  import type { MediaItem } from "$lib/backend";
  import SearchModal from "$lib/SearchModal.svelte";
  import DeckSwitcher from "$lib/DeckSwitcher.svelte";
  import CatalogModal from "$lib/CatalogModal.svelte";
  import { initSfx, sfxMove, sfxEnter } from "$lib/sfx";
  import { ambientApply, ambientStop } from "$lib/ambient";
  import { OSK_ROWS, OSK_FLAT, OSK_COLS } from "$lib/osk";
  import { splitArgv } from "$lib/argv";
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

  // Now Playing transport overlay: the bottom-right card stack is pointer/navpad-clickable, but
  // a controller on the dashboard had no way to reach its prev/play-pause/next/switch/close
  // controls (the global pad router never focused them). This overlay surfaces the primary
  // card's actions as a D-pad/A-navigable row. It's dashboard-contextual (opened by L1 or the
  // `N` key when something's playing), never a global chord — while a launched app is in front
  // the pad drives the app, not this, so common buttons aren't consumed there.
  let npOpen = $state(false);
  let npFocus = $state(0);
  const npActions = $derived.by<NpAction[]>(() => {
    const c = nowCards[0];
    if (!c) return [];
    // The same builder the corner card stack renders (NowPlaying.svelte) — one control
    // set, two surfaces. `after` closes the overlay on the terminal actions.
    return cardActions(c, {
      inSession,
      onerror: reportError,
      ondismiss: (id) => (nowList = nowList.filter((x) => x.id !== id)),
      after: () => (npOpen = false),
    });
  });
  function openNowPlaying() { if (nowCards.length) { npFocus = 0; npOpen = true; } }
  function npMove(d: number) { if (npActions.length) npFocus = clamp(npFocus + d, 0, npActions.length - 1); }
  function npActivate() { npActions[npFocus]?.run(); }
  // If the underlying card set empties (media stopped, app closed) while open, dismiss the overlay.
  $effect(() => { if (npOpen && npActions.length === 0) npOpen = false; else if (npOpen) npFocus = clamp(npFocus, 0, Math.max(0, npActions.length - 1)); });

  let allGames = $state<Game[]>([]);
  let favorites = $state<string[]>([]);
  let recentApps = $state<string[]>([]); // app ids, most-recent-first
  let catSel = $state(1);
  let focusRaw = $state(0); // raw cursor — read via the clamped `focus` $derived below
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

  // Durable boot failures (config/library/capability/catalog), keyed by subsystem. Unlike the
  // 5s action toast above, these persist until the subsystem loads — a couch user shouldn't
  // miss "library error" in a toast that's gone before they look up. Retry re-runs the failed
  // loaders (see loadCapability/loadCatalog/loadConfigAndLibrary).
  let bootErrors = $state<Record<string, string>>({});
  function setBootError(key: string, msg: string | null) {
    if (msg) bootErrors = { ...bootErrors, [key]: msg };
    else if (key in bootErrors) { const next = { ...bootErrors }; delete next[key]; bootErrors = next; }
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
  // The cursor clamps to the live list length as a $derived (not a write-back $effect): when
  // the list shrinks under it (uninstall, filter change) every read sees the clamped index in
  // the same render pass — no second render, no effect re-entry risk. Writers set focusRaw.
  let focus = $derived(itemCount ? Math.min(focusRaw, itemCount - 1) : 0);
  // ---- windowed (virtualized) item rail ----
  // The rail translates so the focused row sits at the top of the clipped wrap, meaning only
  // ~[focus, focus + viewport-rows] can ever be on screen. Render just that slice — a small
  // margin above (upward-slide transition + the `near` fade) and a generous one below (covers a
  // 4K panel at the smallest UI scale, ~32 visible rows) — and preserve absolute row offsets
  // with a spacer, so each keypress costs O(window), not O(library). Art AND app-icon loading
  // key off the same window: a 1,000-game library no longer fires a fetch per game at mount.
  const WIN_ABOVE = 8, WIN_BELOW = 40;
  let winRange = $derived(railWindow(items.length, focus, WIN_ABOVE, WIN_BELOW));
  let winLo = $derived(winRange.lo);
  let winItems = $derived(items.slice(winRange.lo, winRange.hi));
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
  function resetFocus() { focusRaw = catId === "settings" && visibleSettings[0]?.type === "header" ? 1 : 0; }
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
    focusRaw = f;
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
    if (key === "addcustom") { formOpen = true; fName = ""; fExec = ""; fIcon = "🚀"; fCat = "apps"; }
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
  let mediaAvail = $state(false);
  let mediaOpen = $state(false);
  let mediaLoading = $state(false);
  let mediaStack = $state<{ title: string; rows: MediaRow[] }[]>([]);
  let mediaFocus = $state(0);
  let mediaPosters = $state<Record<string, string>>({});

  // Deck switcher (iOS-style app cards): Guide tap opens it (backend hides the apps so this
  // overlay is what shows); pick a card to bring that app forward, Select to close it.
  let deckOpen = $state(false);
  let deckApps = $state<LiveApp[]>([]);
  let deckFocus = $state(0);
  // An app's launcher icon/emoji for its card, matched by launch id then name (games show 🎮).
  function deckIcon(a: LiveApp): string {
    // Launch ids are `tileId#seq` ($lib/launchId) — match the tile id, then fall back to name.
    const tileId = a.id ? baseId(a.id) : undefined;
    const app = apps.find((x) => x.id === tileId) ?? apps.find((x) => x.name === a.name);
    return app?.icon ?? "🎮";
  }
  async function openDeck() {
    try { deckApps = await api.deckOpen(); } catch (e) { deckApps = []; console.debug("[omnideck] deck open failed", e); }
    if (deckApps.length === 0) return; // nothing running — don't show an empty deck
    deckFocus = 0;
    deckOpen = true;
  }
  // Dismiss = put back what opening the deck took away: deck_open hid (and possibly froze)
  // the foreground app, so a tap-tap round trip must land back in it, not strand it hidden.
  function closeDeck() {
    deckOpen = false;
    api.deckCancel().catch((e) => console.debug("[omnideck] deck cancel failed", e));
  }
  function deckMove(d: number) { if (deckApps.length) deckFocus = clamp(deckFocus + d, 0, deckApps.length - 1); }
  async function deckSelect() {
    const a = deckApps[deckFocus];
    deckOpen = false;
    if (a) await api.deckShow(a.group).catch((e) => reportError("Couldn't open app", e));
  }
  async function deckKill() {
    const a = deckApps[deckFocus];
    if (!a) return;
    try {
      await api.deckClose(a.group);
    } catch (e) {
      // Keep the card: the app is still running, and silently dropping it claimed a close
      // that didn't happen (reopening the deck resurrected the "closed" card).
      reportError("Couldn't close app", e);
      return;
    }
    deckApps = deckApps.filter((x) => x.group !== a.group);
    if (deckApps.length === 0) { deckOpen = false; return; }
    deckFocus = clamp(deckFocus, 0, deckApps.length - 1);
  }
  const mediaView = $derived(mediaStack[mediaStack.length - 1]);
  function mediaRow(i: MediaItem, group?: string): MediaRow {
    const browse = ["Series", "Season", "Folder", "BoxSet", "CollectionFolder"].includes(i.kind);
    const pct = i.played_pct ? `${Math.round(i.played_pct)}% · ` : "";
    const mins = i.runtime_mins ? `${i.runtime_mins} min` : i.kind.toLowerCase();
    const sub = i.series ? `${pct}${i.series}` : `${pct}${mins}`;
    return { id: i.id, name: i.name, sub, group, browse };
  }
  async function openMedia() {
    holdStop();
    mediaOpen = true;
    mediaLoading = true;
    mediaStack = [];
    mediaFocus = 0;
    try {
      const s = await api.mediaSections();
      // An item can be in BOTH resume and latest — drop the duplicate (also: a keyed
      // {#each} throws on duplicate keys, which silently blanks the whole list).
      const seen = new Set(s.resume.map((i) => i.id));
      mediaStack = [{
        title: s.server_name,
        rows: [
          ...s.resume.map((i) => mediaRow(i, "Continue watching")),
          ...s.latest.filter((i) => !seen.has(i.id)).map((i) => mediaRow(i, "Latest")),
          ...s.libraries.map((l) => ({ id: l.id, name: l.name, sub: l.kind, group: "Libraries", browse: true })),
        ],
      }];
    } catch (e) { reportError("Media library", e); mediaOpen = false; }
    mediaLoading = false;
  }
  async function mediaActivate() {
    const r = mediaView?.rows[mediaFocus];
    if (!r || mediaLoading) return;
    if (r.browse) {
      mediaLoading = true;
      try {
        const items = await api.mediaBrowse(r.id);
        mediaStack = [...mediaStack, { title: r.name, rows: items.map((i) => mediaRow(i)) }];
        mediaFocus = 0;
      } catch (e) { reportError("Media library", e); }
      mediaLoading = false;
    } else {
      mediaOpen = false;
      status = `▶ ${r.name}…`;
      // The backend mints the per-LAUNCH key (media-<id>#<seq>) — keying the card on the
      // item id let a replay of the same item share a key, and the first instance's exit
      // then cleared the card of the one still playing.
      api.mediaPlay(r.id, r.name)
        .then((key) => { nowList = [{ id: key, kind: "app", name: r.name, category: "video" }, ...nowList.filter((e) => e.id !== key)].slice(0, 3); })
        .catch((e) => reportError("Playback failed", e));
      later(() => (status = ""), 3500);
    }
  }
  function mediaBack() {
    if (mediaStack.length > 1) { mediaStack = mediaStack.slice(0, -1); mediaFocus = 0; }
    else mediaOpen = false;
  }
  function mediaMove(d: number) {
    const n = mediaView?.rows.length ?? 0;
    if (!n) return;
    mediaFocus = clamp(mediaFocus + d, 0, n - 1);
    queueMicrotask(() => document.querySelector(`[data-med="${mediaFocus}"]`)?.scrollIntoView({ block: "nearest" }));
  }
  // Posters for the rows around the focus (windowed like the game rail's art loading).
  $effect(() => {
    if (!mediaOpen || !mediaView) return;
    const win = mediaView.rows.slice(Math.max(0, mediaFocus - 4), mediaFocus + 14);
    for (const r of win) {
      if (mediaPosters[r.id] !== undefined) continue;
      mediaPosters[r.id] = ""; // inflight marker (renders the fallback glyph meanwhile)
      api.mediaPoster(r.id)
        .then((p) => { if (p) mediaPosters = { ...mediaPosters, [r.id]: artUrl(p) }; })
        .catch(() => {});
    }
  });

  async function launchTile(t: Tile) {
    if (t.kind === "app" && t.app.id === "media-library") { openMedia(); return; }
    const name = t.kind === "game" ? t.game.name : t.app.name;
    // A UNIQUE per-launch id (not the tile id) — see $lib/launchId for the format contract.
    // The backend passes this straight back as the exit key; the tile id stays the
    // favorites/recents key.
    const id = mintLaunchId(t.id);
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
    focusRaw = i; // focus is $derived (#72's clamp refactor) — writes go through focusRaw
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
  let formOpen = $state(false);
  let fName = $state("");
  let fExec = $state("");
  let fIcon = $state("🚀");
  let fCat = $state("apps");
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
  function addCustom() {
    const name = fName.trim();
    const cmd = fExec.trim();
    if (!cfg || !name || !cmd) { formOpen = false; return; }
    // Slugify, trimming leading/trailing dashes so "My App!" and "My App?" don't both collapse
    // to "custom-my-app-"; reject a name with no usable characters.
    const base = "custom-" + name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
    if (base === "custom-") { reportError("Add a name with at least one letter or number", null); return; }
    // De-dup with a numeric suffix instead of silently overwriting an existing same-slug launcher.
    const collided = apps.some((a) => a.id === base);
    let id = base; for (let n = 2; apps.some((a) => a.id === id); n++) id = `${base}-${n}`;
    // A bare URL (e.g. a SearXNG instance) is launched as a browser app so it opens in the
    // browser AND gets its site favicon; anything else is run as a normal argv command.
    // The split is quote-aware (review #6) so paths with spaces work: "/My Games/app" --flag.
    const isUrl = /^https?:\/\//i.test(cmd);
    const argv = isUrl ? null : splitArgv(cmd);
    if (!isUrl && (!argv || argv.length === 0)) {
      reportError(argv ? "Command is empty" : "Unbalanced quote in command", null);
      return; // keep the form open so the user can fix it
    }
    const exec = isUrl ? ["BROWSER", `--app=${cmd}`] : argv!;
    const app = { id, name, icon: fIcon || "🚀", exec, accent: "#3a4256", category: fCat };
    const next = [...apps, app];
    cfg = { ...cfg, apps: next };
    api.saveApps(next).catch((e) => reportError("Couldn't save apps", e));
    if (collided) { status = `Added "${name}" (a similar name already existed)`; later(() => (status = ""), 3000); }
    formOpen = false;
  }

  let catalogOpen = $state(false);
  let catFocusRaw = $state(0); // raw cursor — read via the clamped `catFocus` $derived below
  let catQuery = $state("");
  let catSort = $state<"group" | "alpha">("group");
  let displayedCatalog = $derived.by(() => {
    const base = catSort === "alpha" ? [...catalog].sort((a, b) => a.name.localeCompare(b.name)) : sortedCatalog;
    const q = catQuery.trim().toLowerCase();
    return q ? base.filter((c) => c.name.toLowerCase().includes(q)) : base;
  });
  // clamped like `focus` above: typing in the filter shrinks displayedCatalog under the cursor
  let catFocus = $derived(Math.min(catFocusRaw, Math.max(0, displayedCatalog.length - 1)));

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
  function toggleCatalog() { holdStop(); catalogOpen = !catalogOpen; catFocusRaw = 0; }
  function catMove(d: number) { catFocusRaw = clamp(catFocus + d, 0, displayedCatalog.length - 1); queueMicrotask(() => document.querySelector(`[data-cat="${catFocus}"]`)?.scrollIntoView({ block: "nearest" })); }
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

  // Single source of truth: is any modal/overlay open? Gates base navigation and stops
  // hold-repeat the instant a modal opens (replaces a 7-term list that had to be kept in sync).
  const anyModal = $derived(
    npOpen || deckOpen || wizardActive || catalogOpen || searchOpen || powerOpen || !!confirmAct || formOpen || infoOpen || helpOpen || mediaOpen,
  );

  function onKey(e: KeyboardEvent) {
    // F5 retries a failed boot from anywhere (the panel's keyboard twin) — cheap and global,
    // independent of the XMB nav state.
    if (e.key === "F5" && Object.keys(bootErrors).length) { e.preventDefault(); retryBoot(); return; }
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
    if (deckOpen) {
      // Deck switcher, keyboard twin of the gamepad gate: arrows pick, Enter opens,
      // Delete/Backspace closes the card, Escape dismisses.
      if (e.key === "ArrowLeft" && navGate()) deckMove(-1);
      else if (e.key === "ArrowRight" && navGate()) deckMove(1);
      else if (e.key === "Enter") deckSelect();
      else if (e.key === "Delete" || e.key === "Backspace") deckKill();
      else if (e.key === "Escape") closeDeck();
      return;
    }
    if (npOpen) {
      // Now Playing transport: arrows move across the actions, Enter activates, Esc closes.
      if (e.key === "ArrowLeft" && navGate()) npMove(-1);
      else if (e.key === "ArrowRight" && navGate()) npMove(1);
      else if (e.key === "Enter") npActivate();
      else if (e.key === "Escape") npOpen = false;
      return;
    }
    if (wizardActive) {
      if (e.key === "Enter") wizardNext();
      else if (e.key === "Escape") wizardPrev();
      else if (e.key === "ArrowLeft" && wizardStep === 1 && navGate()) wizardAccent(-1);
      else if (e.key === "ArrowRight" && wizardStep === 1 && navGate()) wizardAccent(1);
      return;
    }
    if (infoOpen) { if (e.key === "Escape" || e.key === "Enter" || e.key === "i" || e.key === "I") infoOpen = false; return; }
    if (helpOpen) { if (e.key === "Escape" || e.key === "Enter" || e.key === "?" || e.key === "F1") helpOpen = false; return; }
    if (e.key === "?" || e.key === "F1") { e.preventDefault(); holdStop(); helpOpen = true; return; }
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
    if (mediaOpen) {
      if (e.key === "ArrowUp" && navGate()) mediaMove(-1);
      else if (e.key === "ArrowDown" && navGate()) mediaMove(1);
      else if (e.key === "Enter") mediaActivate();
      else if (e.key === "Escape" || e.key === "Backspace") mediaBack();
      return;
    }
    if (e.key === "/" && !searchOpen && !catalogOpen) { e.preventDefault(); openSearch(); return; }
    if (searchOpen) {
      if (e.key === "ArrowUp") searchMove(-1);
      else if (e.key === "ArrowDown") searchMove(1);
      else if (e.key === "Enter") searchActivate();
      else if (e.key === "Escape") { if (searchQuery) searchQuery = ""; else searchOpen = false; }
      else if (e.key === "Backspace") { searchQuery = searchQuery.slice(0, -1); oskDim = true; }
      // preventDefault so Space can't ALSO natively re-activate a mouse-focused result row
      else if (e.key.length === 1 && /^[\w .\-]$/.test(e.key)) { e.preventDefault(); searchQuery += e.key; oskDim = true; }
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
      // preventDefault so Space can't ALSO natively re-toggle a mouse-focused catalog row
      else if (e.key.length === 1 && /^[a-z0-9 ]$/i.test(e.key)) { e.preventDefault(); catQuery += e.key; }
      return;
    }
    if (e.key === "p" || e.key === "P") { gotoSettings(); return; }
    if (e.key === "h" || e.key === "H") { goHome(); return; }
    if (e.key === "f" || e.key === "F") { favCurrent(); return; }
    if (e.key === "i" || e.key === "I") { showInfo(); return; }
    if ((e.key === "n" || e.key === "N") && nowCards.length) { openNowPlaying(); return; }
    if (e.key === "Escape") { settingsEditing = false; return; }
    if (e.key === "ArrowLeft" && navGate()) horiz(-1);
    else if (e.key === "ArrowRight" && navGate()) horiz(1);
    else if (e.key === "ArrowUp" && navGate()) moveItem(-1);
    else if (e.key === "ArrowDown" && navGate()) moveItem(1);
    else if (e.key === "Enter") activate();
  }

  // Boot-time subsystem loaders — named so the boot-error panel's Retry can re-run just the
  // ones that failed. Each clears its bootError on success and records it on failure.
  async function loadCapability() {
    try { cap = await api.getCapability(); setBootError("capability", null); }
    catch (e) { setBootError("capability", `Capability probe failed: ${e}`); }
  }
  async function loadCatalog() {
    try { catalog = await api.getCatalog(); setBootError("catalog", null); }
    catch (e) { setBootError("catalog", `Couldn't load the app catalog: ${e}`); }
  }
  async function loadConfigAndLibrary() {
    try {
      const c = await api.getConfig();
      cfg = c;
      accent = c.settings?.accent ?? "#b14cff";
      favorites = c.favorites ?? [];
      recentApps = c.recent_apps ?? [];
      if (c.settings && c.settings.onboarded === false) { wizardActive = true; wizardStep = 0; }
      status = ""; // config loaded — clear the "Loading…" toast; the dashboard can render now
      // A parse error isn't a hard load failure (we fell back to defaults) but the user should
      // still see it and be able to fix + retry — surface it in the durable panel, not a toast.
      setBootError("config", c.config_error ?? null);
    } catch (e) {
      status = "";
      setBootError("config", `Couldn't load settings: ${e}`);
      return; // no cfg — don't load the library into a half-initialized state
    }
    // Art loads lazily per windowed row (see the winItems $effect), not per game here.
    try {
      const lib = await api.getLibrary();
      allGames = lib.games ?? [];
      setBootError("library", null);
    } catch (e) {
      setBootError("library", `Library error: ${e}`);
    }
  }
  /** Re-run the boot loaders that are currently in an error state (the panel's Retry). */
  function retryBoot() {
    if (bootErrors.capability) loadCapability();
    if (bootErrors.catalog) loadCatalog();
    if (bootErrors.config || bootErrors.library) loadConfigAndLibrary();
  }

  onMount(() => {
    window.addEventListener("keydown", onKey);
    loadCapability();
    api.mediaAvailable().then((v) => (mediaAvail = v)).catch(() => {}); // optional; missing = no tile
    api.inGamescopeSession().then((v) => (inSession = v)).catch((e) => console.debug("[omnideck] inGamescopeSession probe failed", e));
    loadCatalog();
    loadConfigAndLibrary();

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
        if (deckOpen) {
          // Card deck: L/R picks a card, A/X opens it, Select closes it, B/Guide dismisses.
          if (p.code === "DPadLeft") holdStart(p.code, () => deckMove(-1));
          else if (p.code === "DPadRight") holdStart(p.code, () => deckMove(1));
          else if (p.code === "South" || p.code === "West") deckSelect();
          else if (p.code === "Select") deckKill();
          else if (p.code === "East" || p.code === "Start") closeDeck();
          return;
        }
        if (npOpen) {
          // Now Playing transport: L/R across the actions, A activates, B closes.
          if (p.code === "DPadLeft") holdStart(p.code, () => npMove(-1));
          else if (p.code === "DPadRight") holdStart(p.code, () => npMove(1));
          else if (p.code === "South") npActivate();
          else if (p.code === "East" || p.code === "Start") npOpen = false;
          return;
        }
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
        if (helpOpen) { if (p.code === "East" || p.code === "South") helpOpen = false; return; }
        if (powerOpen) {
          if (p.code === "DPadUp") holdStart(p.code, () => powerMove(-1));
          else if (p.code === "DPadDown") holdStart(p.code, () => powerMove(1));
          else if (p.code === "South") powerActivate();
          else if (p.code === "East") powerOpen = false;
          return;
        }
        if (mediaOpen) {
          if (p.code === "DPadUp") holdStart(p.code, () => mediaMove(-1));
          else if (p.code === "DPadDown") holdStart(p.code, () => mediaMove(1));
          else if (p.code === "South") mediaActivate();
          else if (p.code === "East") mediaBack();
          return;
        }
        if (searchOpen) {
          // D-pad drives the on-screen keyboard; bumpers move the result selection.
          if (["DPadUp", "DPadDown", "DPadLeft", "DPadRight", "South"].includes(p.code)) oskDim = false;
          if (p.code === "DPadUp") holdStart(p.code, () => oskMove(0, -1));
          else if (p.code === "DPadDown") holdStart(p.code, () => oskMove(0, 1));
          else if (p.code === "DPadLeft") holdStart(p.code, () => oskMove(-1, 0));
          else if (p.code === "DPadRight") holdStart(p.code, () => oskMove(1, 0));
          else if (p.code === "South") oskPress(OSK_FLAT[oskFocus]);
          else if (p.code === "LeftTrigger") searchMove(-1);
          else if (p.code === "RightTrigger") searchMove(1);
          else if (p.code === "West") searchQuery = searchQuery.slice(0, -1);
          else if (p.code === "East") { if (searchQuery) searchQuery = ""; else searchOpen = false; }
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
        if (p.code === "LeftTrigger" && nowCards.length) { openNowPlaying(); return; } // L1 → Now Playing transport
        if (p.code === "East") { settingsEditing = false; return; }
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
        if (deckOpen) {
          // Deck: horizontal stick moves the card selection; vertical does nothing.
          if (p.code === "LeftStickX" && heldCode !== code) holdStart(code, () => deckMove(dir));
          else if (p.code === "LeftStickY") holdStop();
          return;
        }
        if (p.code === "LeftStickY") {
          // In the list modals the stick drives the row selection (the D-pad keeps its
          // modal-specific job, e.g. the OSK in search). Other overlays swallow the stick.
          const rowMove = powerOpen ? powerMove : searchOpen ? searchMove : catalogOpen ? catMove : mediaOpen ? mediaMove : null;
          if (anyModal && !rowMove) { holdStop(); return; }
          const fn = rowMove ?? moveItem;
          if (heldCode !== code) holdStart(code, () => fn(dir));
        } else {
          if (anyModal) { holdStop(); return; } // stick X has no modal meaning (OSK is D-pad)
          if (heldCode !== code) holdStart(code, () => horiz(dir));
        }
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

  // fetch site icons for web/app tiles — windowed like game art, so a rail keypress costs
  // O(window) and a large library doesn't fan out icon IPC for every off-screen tile
  $effect(() => { for (const t of winItems) if (t.kind === "app") loadAppIcon(t.app); });
  // the add-apps catalog is the one full-list pass (it's small + scrollable) — only while open
  $effect(() => { if (!catalogOpen) return; for (const c of displayedCatalog) loadAppIcon(c); });
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
          {:else if catId === "games"}No games found.
          {:else}Empty — press <b>△ / A</b> to add apps & media.{/if}
        </div>
      {:else}
        <div class="xitems" style="transform: translateY(calc({-focus} * var(--ih)))">
          {#if winLo > 0}<div class="xpad" style="height: calc({winLo} * var(--ih))" aria-hidden="true"></div>{/if}
          {#each winItems as t, wi (t.id)}
            {@const i = wi + winLo}
            <button class="xitem" class:focused={i === focus} class:near={Math.abs(i - focus) <= 4}
              onclick={() => { focusRaw = i; launchTile(t); }}>
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
      onfocus={(i) => (catFocusRaw = i)}
      ontoggle={catToggle}
      onsortswap={() => (catSort = catSort === "group" ? "alpha" : "group")}
      onclose={() => (catalogOpen = false)}
    />
  {/if}

  {#if helpOpen}
    <HelpModal {inSession} onclose={() => (helpOpen = false)} />
  {/if}

  {#if mediaOpen}
    <MediaModal
      title={mediaView?.title ?? "Media"}
      rows={mediaView?.rows ?? []}
      focus={mediaFocus}
      posters={mediaPosters}
      loading={mediaLoading}
      depth={mediaStack.length}
      onfocus={(i) => (mediaFocus = i)}
      onactivate={mediaActivate}
      onclose={() => (mediaOpen = false)}
    />
  {/if}

  {#if deckOpen}
    <DeckSwitcher apps={deckApps} focus={deckFocus} iconFor={deckIcon}
      onfocus={(i) => (deckFocus = i)} onselect={deckSelect} onkill={deckKill} onclose={closeDeck} />
  {/if}

  {#if npOpen}
    <!-- Now Playing transport: controller/keyboard-reachable twin of the corner card's controls
         (L1 or N opens it on the dashboard). Pointer/click works too. -->
    <div class="np-scrim" role="button" tabindex="-1" aria-label="Close Now Playing"
         onclick={() => (npOpen = false)} onkeydown={(e) => { if (e.key === "Escape") npOpen = false; }}></div>
    <section class="np-transport" aria-label="Now Playing controls">
      {#if nowCards[0]}
        <div class="np-t-title">
          {nowCards[0].media?.title ?? nowCards[0].name}{#if nowCards[0].media?.artist}<span class="np-t-sub"> — {nowCards[0].media.artist}</span>{/if}
        </div>
      {/if}
      <div class="np-t-row">
        {#each npActions as a, i (a.label)}
          <button class="np-t-btn" class:sel={i === npFocus} title={a.label} aria-label={a.label}
            onclick={() => { npFocus = i; a.run(); }} onmouseenter={() => (npFocus = i)}>{a.icon}</button>
        {/each}
      </div>
      <p class="np-t-hint">{npActions[npFocus]?.label ?? ""} · A activate · B close</p>
    </section>
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
    <Modal labelledby="dlg-form" backdropLabel="Close" onclose={() => (formOpen = false)}>
      <h2 id="dlg-form">Add custom launcher</h2>
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
      <p class="phint">Split on spaces; quote paths that contain them: "/My Games/app" --flag. Use the full path if it isn't on PATH. Esc to close.</p>
    </Modal>
  {/if}

  {#if wizardActive && cfg}
    <Wizard step={wizardStep} tier={cap?.tier ?? null} gamesCount={games.length} catalogCount={catalog.length}
      accents={ACCENTS} accent={cfg.settings.accent ?? "#4cc2ff"} />
  {/if}

  <NowPlaying cards={nowCards} {inSession} onerror={reportError}
    ondismiss={(id) => (nowList = nowList.filter((x) => x.id !== id))} />

  {#if status}<div class="toast">{status}</div>{/if}
  {#if toastErr}<div class="toast err" role="alert" aria-live="assertive">⚠ {toastErr}</div>{/if}

  <!-- Durable boot-failure panel: persists (unlike the 5s toast) until the subsystem loads,
       with a Retry that re-runs just the failed loaders. Pointer/keyboard-focusable now; F5
       also retries. (Controller-button retry can piggyback on the Now Playing focus work.) -->
  {#if Object.keys(bootErrors).length}
    <div class="boot-errors" role="alert" aria-live="assertive">
      <div class="boot-errors-hd">⚠ OmniDeck had trouble starting</div>
      <ul>
        {#each Object.entries(bootErrors) as [key, msg] (key)}<li>{msg}</li>{/each}
      </ul>
      <button class="boot-retry" onclick={retryBoot}>Retry (F5)</button>
    </div>
  {/if}

  <footer>
    <span class="fdiag"><button class="fpsbtn" title="frame rate (current · avg · low · high) — click to reset lo/hi" onclick={resetFpsStats}>fps {fps} · avg {fpsAvg} · lo {fpsLo > 999 ? "—" : fpsLo} · hi {fpsHi}</button> · {cap?.tier ?? "?"}</span>
    <span class="fhints"><b>Enter/✕</b> select · <b>Esc/◯</b> back · <button class="fhelp" onclick={() => { holdStop(); helpOpen = true; }}><b>?</b> help</button></span>
  </footer>
</main>

<style>
  /* Design tokens — the surface/text/border vocabulary shared across every component.
     Change a shade once here; components reference var(--…). Defined on :root so the values
     cascade to every component (fixed/position-independent ones included). The dynamic
     --accent / --scale / --bg-* are set on <main> instead (they depend on user settings). */
  :global(:root) {
    --surface-deep: #05070b;  /* deepest background / scrim base */
    --surface: #1b2540;       /* control / button surface */
    --surface-card: #0c1320;  /* raised card surface */
    --border: #2c3a5c;        /* default hairline border */
    --text-muted: #9fb0c8;    /* secondary text */
    --text-soft: #cdd7e6;     /* soft light text */
    --text-label: #6b7790;    /* uppercase captions / labels */
    --text-dim: #7e8aa0;      /* dim state text */
    --danger: #c0392b;        /* error / destructive */
  }
  :global(html), :global(body) { margin: 0; height: 100%; }
  :global(body) { background: var(--surface-deep); overflow: hidden; }

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
  .clock { color: var(--text-soft); font-weight: 700; font-variant-numeric: tabular-nums; font-size: calc(clamp(13px, 1.5vw, 19px) * var(--scale)); margin-right: 4px; }
  .badge { background: #121a2b99; border: 1px solid #25324d; border-radius: 999px; padding: 5px 14px; color: var(--text-muted); font-size: clamp(11px, 1.2vw, 14px); }
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
  .xsheadlbl { color: var(--text-label); font-size: clamp(11px, 1.1vw, 13px); text-transform: uppercase; letter-spacing: 2px; font-weight: 700; }
  .xthumb img { width: 100%; height: 100%; object-fit: cover; }
  .xthumb img.appicon { object-fit: contain; padding: 18%; box-sizing: border-box; }
  .xthumb .xemoji { font-size: calc(1.5rem * var(--scale)); }
  .xitem.focused .xthumb { box-shadow: 0 0 0 2px var(--accent), 0 8px 24px #000a; }
  .xname { font-size: calc(clamp(16px, 1.7vw, 24px) * var(--scale)); font-weight: 600; display: flex; align-items: center; gap: 10px; }
  .xitem.focused .xname { font-weight: 800; }
  .xname .xsub { color: var(--accent); font-weight: 700; font-size: .8em; }
  .xfav { font-size: .8em; }
  .swatch { width: 30px; height: 18px; border-radius: 5px; display: inline-block; border: 1px solid #ffffff44; }
  .numedit { width: 5em; background: var(--surface-card); border: 1px solid var(--accent); color: #fff; border-radius: 7px; padding: 2px 8px; font-size: .8em; font-weight: 700; }
  .textedit { width: 18em; max-width: 40vw; background: var(--surface-card); border: 1px solid var(--accent); color: #fff; border-radius: 7px; padding: 2px 8px; font-size: .8em; }
  /* Suppress the default focus ring on elements that already show focus another way (inputs'
     accent border, the in-app .focused highlight used by controller/mouse nav)... */
  .numedit:focus, .textedit:focus, .cbtn:focus,
  .badge:focus, .fpsbtn:focus,
  .xcat:focus, .xitem:focus { outline: none; }
  /* ...but show a clear accent ring for keyboard users (:focus-visible only).
     (.crow/.oskkey/.sortbtn rules live with the modal vocabulary in Modal.svelte.) */
  .numedit:focus-visible, .textedit:focus-visible, .cbtn:focus-visible,
  .badge:focus-visible, .fpsbtn:focus-visible,
  .xcat:focus-visible, .xitem:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .xempty { position: absolute; top: calc(16% + 7rem * var(--scale)); left: 30vw; right: 4vw; color: #8a96ab; font-size: clamp(15px, 1.8vw, 22px); }
  .xempty b { color: var(--accent); }

  /* z-index 70: toasts are transient alerts and must clear every overlay — the deck (40/41),
     the Now Playing transport scrim (44/45), and the boot-error panel (60). At the old
     stacking (2) a media-control error fired behind the open transport's scrim. */
  .toast { position: fixed; bottom: 7vh; left: 50%; transform: translateX(-50%); z-index: 70; background: var(--accent); color: #04121f; font-weight: 700; padding: 12px 28px; border-radius: 999px; box-shadow: 0 10px 40px color-mix(in srgb, var(--accent) 38%, transparent); font-size: clamp(14px, 1.6vw, 20px); }
  .toast.err { background: var(--danger); color: #fff; bottom: calc(7vh + 58px); box-shadow: 0 10px 40px #c0392b66; }

  /* Durable boot-failure panel (persists until retried, unlike the toasts above). */
  .boot-errors { position: fixed; top: 5vh; left: 50%; transform: translateX(-50%); z-index: 60;
    max-width: min(680px, 90vw); background: #1a0e0e; border: 2px solid var(--danger); border-radius: 16px;
    padding: 18px 24px; color: #f4e9e9; box-shadow: 0 18px 60px #00000088; }
  .boot-errors-hd { font-weight: 800; font-size: clamp(15px, 1.7vw, 21px); margin-bottom: 8px; }
  .boot-errors ul { margin: 0 0 14px; padding-left: 20px; font-size: clamp(13px, 1.4vw, 17px); line-height: 1.5; }
  .boot-retry { background: var(--danger); color: #fff; border: 0; border-radius: 999px; cursor: pointer;
    font: inherit; font-weight: 700; padding: 9px 22px; }
  .boot-retry:hover, .boot-retry:focus-visible { background: #d84a3b; outline: 2px solid #fff; }

  /* Now Playing card styles live in $lib/NowPlaying.svelte */
  /* Modal shell (.prefs*, backdrop, close) AND the shared modal-content vocabulary
     (.catlist/.crow/.cicon/.csearch/.phint/.osk* …) live in $lib/Modal.svelte — the
     in-page dialogs below (power, info, confirm, form) use those classes too. */
  .cwheel { width: 30px; height: 22px; padding: 0; border: 1px solid #ffffff55; border-radius: 5px; background: none; cursor: pointer; }
  .cwheel::-webkit-color-swatch-wrapper { padding: 0; }
  .cwheel::-webkit-color-swatch { border: none; border-radius: 4px; }

  .infogrid { display: grid; grid-template-columns: max-content 1fr; gap: 6px 18px; margin: 6px 0 8px; }
  .infogrid dt { color: var(--text-dim); font-size: clamp(12px, 1.2vw, 14px); font-weight: 700; }
  .infogrid dd { margin: 0; color: #dde5f0; font-size: clamp(12px, 1.3vw, 15px); word-break: break-word; }
  .confirm-btns { display: flex; gap: 12px; justify-content: flex-end; margin: 14px 0 4px; }
  .cbtn { background: var(--surface); border: 1px solid var(--border); color: var(--text-soft); border-radius: 10px; padding: 9px 22px; cursor: pointer; font-size: clamp(13px, 1.4vw, 16px); font-weight: 700; }
  .cbtn:hover { border-color: var(--accent); }
  .cbtn.danger { background: var(--accent); color: #04121f; border-color: transparent; }
  .frow { display: flex; align-items: center; gap: 14px; margin: 8px 0; }
  .frow label { width: 96px; flex: 0 0 auto; color: var(--text-muted); font-weight: 600; font-size: clamp(13px, 1.3vw, 15px); }
  .frow input, .frow select { flex: 1; background: var(--surface-card); border: 1px solid var(--border); color: #eef2f8; border-radius: 9px; padding: 9px 12px; font-size: clamp(13px, 1.4vw, 16px); }
  .frow input:focus, .frow select:focus { outline: none; border-color: var(--accent); }

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

  /* --- Now Playing transport overlay (controller-reachable media/app controls) --- */
  .np-scrim { position: fixed; inset: 0; z-index: 44; background: rgba(3,5,11,0.72); border: 0; }
  .np-transport { position: fixed; inset: 0; z-index: 45; display: flex; flex-direction: column;
    align-items: center; justify-content: center; gap: 22px; pointer-events: none; }
  .np-t-title { pointer-events: none; color: #fff; font-weight: 700; max-width: 80vw; text-align: center;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: calc(22px * var(--scale)); }
  .np-t-sub { color: var(--text-muted); font-weight: 500; }
  .np-t-row { display: flex; gap: 18px; pointer-events: auto; }
  .np-t-btn { display: flex; align-items: center; justify-content: center;
    width: calc(72px * var(--scale)); height: calc(72px * var(--scale)); border-radius: 18px;
    border: 2px solid rgba(255,255,255,0.10); background: linear-gradient(160deg, #141a26, #0c1119);
    color: #e7ecf6; cursor: pointer; font-size: calc(28px * var(--scale));
    transition: transform .16s cubic-bezier(.2,.7,.2,1), border-color .16s, box-shadow .16s; }
  .np-t-btn.sel { transform: translateY(-8px) scale(1.08); border-color: var(--accent);
    box-shadow: 0 16px 44px color-mix(in srgb, var(--accent) 45%, transparent); }
  .np-t-hint { pointer-events: none; color: #8a94a6; font-size: calc(14px * var(--scale)); letter-spacing: .02em; }
  @media (prefers-reduced-motion: reduce) { .np-t-btn { transition: none !important; } }

  /* Respect reduced-motion: stop the looping Now-Playing spinner/EQ and the XMB slide/scale
     transitions for vestibular-sensitive users (the UI stays fully functional, just static).
     (The deck switcher's own reduced-motion rule lives in DeckSwitcher.svelte.) */
  @media (prefers-reduced-motion: reduce) {
    .xcats, .xitems, .xbg, .xitem, .xcat .xcicon { transition: none !important; }
  }
</style>
