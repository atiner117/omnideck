// Table-driven Settings model (review item #17).
//
// Every row of the Settings column is one SettingDef: label, kind, how to read its display
// value, how to mutate it (cycle / numeric set / text set), and when it's visible. The page
// component walks this table instead of dispatching on string keys across four functions
// (settingValue / adjustSetting / cycleSetting / NUM_META+setNum+setText). Adding a setting
// is now one row here — the compiler links each row to the Settings fields it touches.
//
// Defs are pure over `Settings`: they build and return a `Partial<Settings>` patch and never
// save. The page applies patches via its `patchSettings` (which mutates the reactive cfg and
// persists). The one deliberate side effect is the volume-preview blip, kept inline to match
// the previous behavior exactly.
import type { Appearance, Settings } from "./backend";
import { isGridLayout } from "./components/layouts";
import { blip } from "./sfx";

// ---- option lists ----
export const ACCENTS = ["#4cc2ff", "#b14cff", "#6ee7a8", "#ff8a3d", "#ff5d6c", "#ffd166"];
export const SEARCH_MODES = [
  { mode: "duckduckgo", label: "DuckDuckGo", url: "https://duckduckgo.com/?q=" },
  { mode: "google", label: "Google", url: "https://www.google.com/search?q=" },
  { mode: "brave", label: "Brave", url: "https://search.brave.com/search?q=" },
  { mode: "bing", label: "Bing", url: "https://www.bing.com/search?q=" },
  { mode: "searxng", label: "SearXNG", url: "" }, // self-hosted: user supplies the URL
  { mode: "custom", label: "Custom", url: "" },
];
const SIZE_MODES = ["small", "medium", "large", "huge", "custom"];
const BG_DEFAULTS = ["color", "image"];
const BG_COLORS = ["#05070b", "#0d1117", "#161b26", "#1a1a2e", "#000000", "#14110a"];
const RECENTS_MODES = ["both", "games", "apps"];
// Volume presets (mirrors the Size presets): Off / Low / Med / High / Custom.
const SOUND_PRESETS = [
  { label: "Off", on: false, vol: 0 },
  { label: "Low", on: true, vol: 0.3 },
  { label: "Medium", on: true, vol: 0.6 },
  { label: "High", on: true, vol: 1.0 },
];

// ---- helpers ----
const round2 = (v: number) => Math.round(v * 100) / 100;
const cap1 = (s: string) => (s ? s[0].toUpperCase() + s.slice(1) : s);
const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));
// Next entry after `cur` in `list`; when `cur` isn't in the list, continue from `fallbackIdx`.
const nextIn = (list: readonly string[], cur: string, fallbackIdx: number) => {
  const c = list.indexOf(cur);
  return list[((c < 0 ? fallbackIdx : c) + 1) % list.length];
};
function soundLabel(s: Settings): string {
  if (!s.sound) return "Off";
  return SOUND_PRESETS.find((p) => p.on && Math.abs(p.vol - (s.sound_volume ?? 0.6)) < 0.001)?.label ?? "Custom";
}

// ---- row shapes ----
// Section headers are rows too (type "header"): the settings column positions rows by
// focus × --ih, so anything between rows must occupy exactly one row slot. Navigation
// skips them (moveItem) and entry lands past them (resetFocus).
type BaseDef = {
  key: string;
  label: string;
  /** Hide the row when this returns false (rows that only apply to a current selection).
   *  Appearance is passed too for rows gated on the library layout (grid columns). */
  visible?: (s: Settings, a?: Appearance) => boolean;
};
export type HeaderDef = BaseDef & { type: "header" };
export type ActionDef = BaseDef & { type: "action" };
export type CycleDef = BaseDef & {
  type: "cycle";
  /** Display string for the row's current value. */
  value: (s: Settings) => string;
  /** Patch that advances the setting to its next state. */
  cycle: (s: Settings) => Partial<Settings>;
};
export type NumDef = BaseDef & {
  type: "num";
  value: (s: Settings) => string;
  /** Current numeric value (tolerates missing cfg so the edit <input> can render defaults). */
  get: (s: Settings | undefined) => number;
  /** Patch for a new (already clamped/rounded) value. */
  set: (v: number) => Partial<Settings>;
  lo: number;
  hi: number;
  /** Step for the typed <input type=number>. */
  step: number;
  /** D-pad ◀▶ step when it differs from the input step (blur nudges by 2). */
  adjustStep?: number;
  int?: boolean;
  /** Side effect after a D-pad adjust is applied (volume preview blip). */
  adjusted?: () => void;
};
export type TextDef = BaseDef & {
  type: "text";
  value: (s: Settings) => string;
  get: (s: Settings | undefined) => string;
  /** Patch for a new (already trimmed) value. */
  set: (v: string) => Partial<Settings>;
};
export type SettingDef = HeaderDef | ActionDef | CycleDef | NumDef | TextDef;

/** Clamp + round a raw numeric input according to the row's meta. */
export function normalizeNum(d: NumDef, raw: number): number {
  const v = clamp(raw, d.lo, d.hi);
  return d.int ? Math.round(v) : round2(v);
}

// ---- the table ----
export const SETTING_DEFS: SettingDef[] = [
  { key: "hdr-look", label: "Appearance", type: "header" },
  {
    key: "size", label: "Size", type: "cycle",
    value: (s) => cap1(s.ui_scale ?? "medium"),
    cycle: (s) => ({ ui_scale: nextIn(SIZE_MODES, s.ui_scale ?? "medium", 1) }),
  },
  {
    key: "custom", label: "Custom size", type: "num",
    visible: (s) => s.ui_scale === "custom",
    value: (s) => `${s.ui_scale_custom ?? 1.6}×`,
    get: (s) => s?.ui_scale_custom ?? 1.6,
    set: (v) => ({ ui_scale_custom: v }),
    lo: 0.8, hi: 3.5, step: 0.05,
  },
  {
    // appearance.layout, not a Settings field: the page intercepts this key in
    // cycleSetting (Enter/A advances the mode) and renders LayoutPicker inline
    // (mouse picks a mode directly) — same inline-extra pattern as the accent row.
    key: "layout", label: "Library layout", type: "cycle",
    value: () => "", // the inline picker shows the current mode
    cycle: () => ({}),
  },
  {
    key: "gridcols", label: "Grid columns", type: "num",
    visible: (_s, a) => isGridLayout(a?.layout ?? "rail"),
    value: (s) => `${s.grid_columns ?? 6}`,
    get: (s) => s?.grid_columns ?? 6,
    set: (v) => ({ grid_columns: v }),
    lo: 3, hi: 12, step: 1, int: true,
  },
  {
    key: "accent", label: "Accent", type: "cycle",
    value: () => "", // the row shows a live swatch + color wheel instead of text
    cycle: (s) => ({ accent: nextIn(ACCENTS, s.accent ?? "#4cc2ff", 0) }),
  },
  { key: "hdr-bg", label: "Background", type: "header" },
  {
    key: "livewp", label: "Live wallpaper", type: "cycle",
    value: (s) => ((s.live_wallpaper ?? "waves") === "waves" ? "Waves" : "Off"),
    cycle: (s) => ({ live_wallpaper: (s.live_wallpaper ?? "waves") === "waves" ? "off" : "waves" }),
  },
  {
    key: "bgdefault", label: "Default background", type: "cycle",
    value: (s) => ({ color: "Solid color", image: "Custom image" }[s.background_default as string] ?? "Solid color"),
    cycle: (s) => ({ background_default: nextIn(BG_DEFAULTS, s.background_default ?? "color", 0) }),
  },
  {
    key: "bgcolor", label: "Background color", type: "cycle",
    visible: (s) => (s.background_default ?? "color") === "color",
    value: (s) => s.background_color ?? "#05070b",
    cycle: (s) => ({ background_color: nextIn(BG_COLORS, s.background_color ?? BG_COLORS[0], -1) }),
  },
  {
    key: "bgimage", label: "Background image", type: "text",
    visible: (s) => s.background_default === "image",
    value: (s) => (s.background_image ? (s.background_image.split("/").pop() ?? "(none)") : "(none)"),
    get: (s) => s?.background_image ?? "",
    set: (v) => ({ background_image: v }),
  },
  {
    key: "gamebg", label: "Game backgrounds", type: "cycle",
    value: (s) => (s.game_backgrounds ? "on" : "off"),
    cycle: (s) => ({ game_backgrounds: !s.game_backgrounds }),
  },
  {
    key: "appbg", label: "App backgrounds", type: "cycle",
    value: (s) => (s.app_backgrounds ? "on" : "off"),
    cycle: (s) => ({ app_backgrounds: !s.app_backgrounds }),
  },
  // blur/brightness only matter when something is overlaid (game art or app wash)
  {
    key: "blur", label: "Background blur", type: "num",
    visible: (s) => (s.game_backgrounds ?? true) || (s.app_backgrounds ?? true) || s.background_default === "image",
    value: (s) => `${s.bg_blur ?? 0}px`,
    get: (s) => s?.bg_blur ?? 0,
    set: (v) => ({ bg_blur: v }),
    lo: 0, hi: 24, step: 1, adjustStep: 2, int: true,
  },
  {
    key: "bright", label: "Background brightness", type: "num",
    visible: (s) => (s.game_backgrounds ?? true) || (s.app_backgrounds ?? true) || s.background_default === "image",
    value: (s) => `${Math.round((s.bg_brightness ?? 0.82) * 100)}%`,
    get: (s) => s?.bg_brightness ?? 0.82,
    set: (v) => ({ bg_brightness: v }),
    lo: 0.3, hi: 1.0, step: 0.05,
  },
  { key: "hdr-home", label: "Home & Library", type: "header" },
  {
    key: "recents", label: "Home recents", type: "num",
    value: (s) => { const n = s.dashboard_recents ?? 8; return n ? `${n}` : "off"; },
    get: (s) => s?.dashboard_recents ?? 8,
    set: (v) => ({ dashboard_recents: v }),
    lo: 0, hi: 20, step: 1, int: true,
  },
  {
    key: "recents_show", label: "Recents show", type: "cycle",
    value: (s) => cap1(s.recents_show ?? "both"),
    cycle: (s) => ({ recents_show: nextIn(RECENTS_MODES, s.recents_show ?? "both", 0) }),
  },
  {
    key: "sort", label: "Sort", type: "cycle",
    value: (s) => s.sort,
    cycle: (s) => ({ sort: s.sort === "recent" ? "alpha" : "recent" }),
  },
  {
    key: "runtimes", label: "Show runtimes", type: "cycle",
    value: (s) => (s.show_runtimes ? "on" : "off"),
    cycle: (s) => ({ show_runtimes: !s.show_runtimes }),
  },
  { key: "hdr-sound", label: "Sound", type: "header" },
  {
    key: "sound", label: "Navigation sounds", type: "cycle",
    value: (s) => soundLabel(s),
    cycle: (s) => {
      // cycle Off → Low → Medium → High (Custom is reached via the Sound volume row)
      const i = SOUND_PRESETS.findIndex((p) => p.label === soundLabel(s));
      const next = SOUND_PRESETS[(i < 0 ? 0 : i + 1) % SOUND_PRESETS.length];
      if (next.on) blip(620, 0.06, 0.42, "sine", true);
      return { sound: next.on, sound_volume: next.vol };
    },
  },
  {
    key: "soundvol", label: "Sound volume", type: "num",
    visible: (s) => soundLabel(s) === "Custom",
    value: (s) => `${Math.round((s.sound_volume ?? 0.6) * 100)}%`,
    get: (s) => s?.sound_volume ?? 0.6,
    set: (v) => ({ sound_volume: v, sound: v > 0 }),
    lo: 0, hi: 1, step: 0.05,
    adjusted: () => blip(620, 0.06, 0.42, "sine", true),
  },
  {
    key: "ambient", label: "Ambient music", type: "cycle",
    value: (s) => (s.ambient ? "on" : "off"),
    cycle: (s) => ({ ambient: !s.ambient }),
  },
  {
    key: "ambientvol", label: "Ambient volume", type: "num",
    visible: (s) => s.ambient,
    value: (s) => `${Math.round((s.ambient_volume ?? 0.35) * 100)}%`,
    get: (s) => s?.ambient_volume ?? 0.35,
    set: (v) => ({ ambient_volume: v }),
    lo: 0, hi: 1, step: 0.05,
  },
  { key: "hdr-search", label: "Search", type: "header" },
  {
    key: "search", label: "Search provider", type: "cycle",
    value: (s) => SEARCH_MODES.find((m) => m.mode === (s.search_mode ?? "duckduckgo"))?.label ?? "DuckDuckGo",
    cycle: (s) => {
      const c = SEARCH_MODES.findIndex((m) => m.mode === (s.search_mode ?? "duckduckgo"));
      const next = SEARCH_MODES[((c < 0 ? 0 : c) + 1) % SEARCH_MODES.length];
      const patch: Partial<Settings> = { search_mode: next.mode };
      if (next.url) patch.search_provider = next.url; // preset
      else if (SEARCH_MODES.some((m) => m.url === s.search_provider)) patch.search_provider = ""; // entering searxng/custom → clear for the URL field
      return patch;
    },
  },
  {
    key: "searchurl", label: "Search URL", type: "text",
    visible: (s) => s.search_mode === "searxng" || s.search_mode === "custom",
    value: (s) => s.search_provider || "(not set)",
    get: (s) => s?.search_provider ?? "",
    set: (v) => ({ search_provider: v }),
  },
  { key: "hdr-launchers", label: "Launchers", type: "header" },
  { key: "addcustom", label: "Add custom launcher", type: "action" },
];
