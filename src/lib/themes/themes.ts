// Theme system (NOTES-FEATURE-BACKLOG-2026-07-12 Lane A).
//
// A theme is a named set of overrides for the design tokens in src/lib/tokens.css
// (--bg, --surface*, --border, --text*). All the color work lives in themes.css as
// `:root[data-theme="<id>"]` blocks; this module only owns the registry and flips the
// `data-theme` attribute on <html>, so switching is a single attribute write — instant,
// live, and with zero style recalculation done in JS.
//
// The user's accent (settings.accent, applied as --accent on <main>) rides on top of
// any theme unchanged — it IS the "optional accent override".
//
// Ids are persisted in config.toml as `settings.theme` and validated on load by
// Settings::normalize() in src-tauri/src/config.rs — keep the two lists in sync.

export type ThemeId = "omnidark" | "oled" | "light" | "high-contrast" | "crt" | "deck";

export const DEFAULT_THEME: ThemeId = "omnidark";

/** Cycle order for the Settings row (default first). */
export const THEMES: ReadonlyArray<{ id: ThemeId; label: string }> = [
  { id: "omnidark", label: "OmniDark" },
  { id: "oled", label: "OLED Black" },
  { id: "light", label: "Light" },
  { id: "high-contrast", label: "High Contrast" },
  { id: "crt", label: "Retro CRT" },
  { id: "deck", label: "Deck" },
];

/** Clamp an arbitrary (possibly hand-edited) config value to a known theme id. */
export function normalizeTheme(id: string | undefined): ThemeId {
  return (THEMES.find((t) => t.id === id)?.id as ThemeId) ?? DEFAULT_THEME;
}

export function themeLabel(id: string | undefined): string {
  return THEMES.find((t) => t.id === normalizeTheme(id))!.label;
}

/** Next theme after `cur` in registry order (wraps; unknown ids restart the cycle). */
export function nextTheme(cur: string | undefined): ThemeId {
  const i = THEMES.findIndex((t) => t.id === cur);
  return THEMES[(i < 0 ? 0 : i + 1) % THEMES.length].id;
}

/**
 * Apply a theme by stamping `data-theme` on <html>. tokens.css supplies the OmniDark
 * base values; themes.css overrides them per id. Safe to call with any string
 * (unknown → default) and no-ops during SSR.
 */
export function applyTheme(id: string | undefined): void {
  if (typeof document === "undefined") return;
  document.documentElement.dataset.theme = normalizeTheme(id);
}
