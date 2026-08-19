// Theme system (NOTES-FEATURE-BACKLOG-2026-07-12 Lane A).
//
// A theme is a named set of overrides for the design tokens (--surface-deep, --surface,
// --surface-card, --border, --text-*, --danger). The BASE values live in +page.svelte's
// `:global(:root)` block (review #16 put them there rather than a standalone tokens.css);
// every override lives in themes.css as a `:root[data-theme="<id>"]` block, which outranks
// the base `:root` on specificity alone — so import order does not matter.
//
// This module only owns the registry and flips the `data-theme` attribute on <html>, so
// switching is a single attribute write — instant, live, and with zero style recalculation
// done in JS (NOTES-PERFORMANCE: nothing here touches the gamepad rAF loop).
//
// The user's accent (settings.accent, applied as --accent on <main>) rides on top of any
// theme unchanged — it IS the "optional accent override".
//
// Ids are persisted in config.toml as `settings.theme` and validated on load by
// Settings::normalize() in src-tauri/src/config.rs — the theme_ids_match_frontend test
// fails CI if this registry, that whitelist, and themes.css ever drift apart.

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
 * Apply a theme by stamping `data-theme` on <html>. The base token values in +page.svelte
 * supply OmniDark; themes.css overrides them per id. Safe to call with any string (unknown →
 * default, so a hand-edited config can never render unstyled) and no-ops during SSR.
 */
export function applyTheme(id: string | undefined): void {
  if (typeof document === "undefined") return;
  document.documentElement.dataset.theme = normalizeTheme(id);
}
