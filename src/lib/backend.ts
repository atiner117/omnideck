// Typed Tauri IPC layer — the single source of truth for the Rust↔JS contract.
// Types mirror the Rust structs in src-tauri/src/{capability,library,apps,config}.rs and the
// command signatures in src-tauri/src/lib.rs. Keep them in sync when those change (a field
// rename on the Rust side should fail the build here, not silently become `undefined`).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn, type EventCallback } from "@tauri-apps/api/event";

// ---- types (mirror the Rust structs) ----
export type Tier = "gamescope-session" | "media-kiosk" | "plain-window";

export type App = {
  id: string;
  name: string;
  icon: string;
  exec: string[];
  accent: string;
  category: string; // "video" | "music" | "games" | "apps"
};

export type Game = {
  appid: string;
  name: string;
  installdir: string;
  library_path: string;
  installed: boolean;
  is_tool: boolean;
  last_played: number;
  art_box: string | null;
  art_header: string | null;
  art_hero: string | null;
  art_logo: string | null;
};

export type LibrarySummary = { path: string; label: string; available: boolean; app_count: number };
export type Library = {
  steam_root: string | null;
  games: Game[];
  libraries: LibrarySummary[];
  errors: string[];
};

export type Gpu = {
  pci: string;
  vendor: string;
  vendor_id: string;
  device_id: string;
  driver: string;
  class: string;
};
export type Capability = {
  tier: Tier;
  gpus: Gpu[];
  render_nodes: string[];
  drm_cards: string[];
  kms_connectors: string[];
  kms_active: boolean;
  vulkan_icds: string[];
  has_real_gpu: boolean;
  nvidia_present: boolean;
  nvidia_modeset_loaded: boolean;
  gamescope: boolean;
  gamescope_session_plus: boolean;
  cage: boolean;
  diagnostics: string[];
};

export type Settings = {
  grid_columns: number;
  sort: string; // "alpha" | "recent"
  show_runtimes: boolean;
  accent: string;
  steamgriddb_key: string;
  onboarded: boolean;
  ui_scale: string; // small | medium | large | huge | custom
  ui_scale_custom: number;
  bg_blur: number;
  bg_brightness: number;
  search_provider: string;
  search_mode: string;
  sound: boolean;
  sound_volume: number;
  dashboard_recents: number;
  recents_show: string; // games | apps | both
  background_default: string; // color | image
  background_color: string;
  background_image: string;
  game_backgrounds: boolean;
  app_backgrounds: boolean;
};

export type Config = {
  settings: Settings;
  apps: App[];
  favorites: string[];
  recent_apps: string[];
  config_path: string;
  config_error?: string | null; // set when config.toml failed to parse/read (UI warns)
};

export type MediaInfo = { status: string; title: string; artist: string; player: string };

// ---- command wrappers (typed returns; reject on backend Err — callers decide UX) ----
export const getCapability = () => invoke<Capability>("get_capability");
export const getLibrary = () => invoke<Library>("get_library");
export const getConfig = () => invoke<Config>("get_config");
export const getApps = () => invoke<App[]>("get_apps");
export const getCatalog = () => invoke<App[]>("get_catalog");
export const getArt = (path: string) => invoke<string | null>("get_art", { path });
export const gridArt = (appid: string) => invoke<string | null>("grid_art", { appid });
export const appIcon = (url: string) => invoke<string | null>("app_icon", { url });
export const mediaNowPlaying = () => invoke<MediaInfo | null>("media_now_playing");
export const mediaControl = (action: string) => invoke<void>("media_control", { action });
// `id` correlates a Now Playing entry with its exit event (the frontend passes the tile id).
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
export const inGamescopeSession = () => invoke<boolean>("in_gamescope_session");
export const quit = () => invoke<void>("quit");

// ---- events ----
export type GamepadEvent = { kind: string; code: string; value: number };
/** A launched app/game exited — payload is the launch id (the tile id) we passed at launch. */
export const onAppExited = (cb: EventCallback<string>): Promise<UnlistenFn> => listen<string>("app-exited", cb);
export const onGamepad = (cb: EventCallback<GamepadEvent>): Promise<UnlistenFn> =>
  listen<GamepadEvent>("gamepad-event", cb);
