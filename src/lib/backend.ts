// Typed Tauri IPC layer — the single source of truth for the Rust↔JS contract.
// The types are GENERATED from the Rust structs by ts-rs into ./bindings/ (a field rename on
// the Rust side fails the build here instead of silently becoming `undefined`). Regenerate:
//   cd src-tauri && TS_RS_EXPORT_DIR=$PWD/../src/lib/bindings cargo test export_bindings
// CI fails if the committed bindings drift from the Rust structs.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn, type EventCallback } from "@tauri-apps/api/event";

// ---- types (generated — see ./bindings/) ----
import type { App } from "./bindings/App";
import type { Capability } from "./bindings/Capability";
import type { Config } from "./bindings/Config";
import type { Game } from "./bindings/Game";
import type { GamepadEvent } from "./bindings/GamepadEvent";
import type { Gpu } from "./bindings/Gpu";
import type { Library } from "./bindings/Library";
import type { LibrarySummary } from "./bindings/LibrarySummary";
import type { MediaInfo } from "./bindings/MediaInfo";
import type { MediaItem } from "./bindings/MediaItem";
import type { MediaLibrary } from "./bindings/MediaLibrary";
import type { MediaSections } from "./bindings/MediaSections";
import type { Settings } from "./bindings/Settings";
import type { Tier } from "./bindings/Tier";
export type { App, Capability, Config, Game, GamepadEvent, Gpu, Library, LibrarySummary, MediaInfo, MediaItem, MediaLibrary, MediaSections, Settings, Tier };

// ---- command wrappers (typed returns; reject on backend Err — callers decide UX) ----
export const getCapability = () => invoke<Capability>("get_capability");
export const getLibrary = () => invoke<Library>("get_library");
export const getConfig = () => invoke<Config>("get_config");
export const getApps = () => invoke<App[]>("get_apps");
export const getCatalog = () => invoke<App[]>("get_catalog");
export const getArt = (path: string) => invoke<string | null>("get_art", { path });
/** Downscaled, cached copy of the custom wallpaper; resolves to an on-disk path for an omnideck:// URL (null → caller falls back to getArt). */
export const bgImage = (path: string) => invoke<string | null>("bg_image", { path });
export const gridArt = (appid: string) => invoke<string | null>("grid_art", { appid });
export const appIcon = (url: string) => invoke<string | null>("app_icon", { url });
export const mediaNowPlaying = () => invoke<MediaInfo | null>("media_now_playing");
export const mediaControl = (action: string) => invoke<void>("media_control", { action });
// `id` correlates a Now Playing entry with its exit event. The frontend passes a per-launch
// token (`tileId::N`, unique per launch) so a stale exit can't drop a quick relaunch's card;
// the backend treats it as opaque (echoed in `app-exited` and on LiveApp.id).
export const launchGame = (appid: string, name: string, id?: string) =>
  invoke<void>("launch_game", { appid, name, id });
export const launchCommand = (exec: string[], name: string, id?: string) =>
  invoke<void>("launch_command", { exec, name, id });
export const saveSettings = (settings: Settings) => invoke<void>("save_settings", { settings });
export const saveApps = (apps: App[]) => invoke<void>("save_apps", { apps });
export const saveFavorites = (favorites: string[]) => invoke<void>("save_favorites", { favorites });
export const saveRecentApps = (recentApps: string[]) => invoke<void>("save_recent_apps", { recentApps });
export const gameProperties = (appid: string) => invoke<void>("game_properties", { appid });
export const powerAction = (action: string) => invoke<void>("power_action", { action });
export const closeCurrentApp = () => invoke<boolean>("close_current_app");
/** Hide/show the launched app without closing it (session app switcher). */
export const switchApp = () => invoke<boolean>("switch_app");
export const inGamescopeSession = () => invoke<boolean>("in_gamescope_session");
export const quit = () => invoke<void>("quit");

// ---- deck switcher (iOS-style app cards — switcher.rs / watchdog.rs) ----
/** A live launched app the deck shows as a card. */
export type LiveApp = { group: number; name: string; id: string | null };
/** Open the deck: hides all apps so the overlay shows, returns the live-app cards. */
export const deckOpen = () => invoke<LiveApp[]>("deck_open");
/** Re-fetch the live-app cards without changing window state. */
export const deckList = () => invoke<LiveApp[]>("deck_list");
/** Bring one app group to the front (a card was chosen). */
export const deckShow = (group: number) => invoke<void>("deck_show", { group });
/** Close one app group (a card's close / Select). */
export const deckClose = (group: number) => invoke<void>("deck_close", { group });

// ---- media server (Jellyfin browse/play — media_server.rs) ----
export const mediaAvailable = () => invoke<boolean>("media_available");
export const mediaSections = () => invoke<MediaSections>("media_sections");
export const mediaBrowse = (parent: string) => invoke<MediaItem[]>("media_browse", { parent });
/** Fetch+cache an item's poster; resolves to the on-disk path for an omnideck:// URL. */
export const mediaPoster = (id: string) => invoke<string | null>("media_poster", { id });
export const mediaPlay = (id: string, name: string) => invoke<void>("media_play", { id, name });

// ---- events ----
/** A launched app/game exited — payload is the launch id (the tile id) we passed at launch. */
export const onAppExited = (cb: EventCallback<string>): Promise<UnlistenFn> => listen<string>("app-exited", cb);
export const onGamepad = (cb: EventCallback<GamepadEvent>): Promise<UnlistenFn> =>
  listen<GamepadEvent>("gamepad-event", cb);
/** MPRIS state changed (track/status/player) — pushed by the backend watcher, no polling. */
export const onMediaChanged = (cb: EventCallback<MediaInfo | null>): Promise<UnlistenFn> =>
  listen<MediaInfo | null>("media-changed", cb);
/** Guide button tapped (or Ctrl+Alt+Home) — the frontend toggles the deck switcher. */
export const onGuideTap = (cb: EventCallback<null>): Promise<UnlistenFn> => listen<null>("guide-tap", cb);
