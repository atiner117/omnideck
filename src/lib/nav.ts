// Pure navigation helpers, split out of +page.svelte so they're unit-testable (the page
// itself is a large reactive component). No Svelte/DOM/Tauri imports here on purpose — this
// is the first slice of the "extract pure logic" decomposition, and the regression net.

/** Clamp `v` into `[lo, hi]`. Used everywhere a focus index is moved. */
export function clamp(v: number, lo: number, hi: number): number {
  return Math.max(lo, Math.min(hi, v));
}

/** Half-open `[lo, hi)` slice of a `total`-length rail to actually render around `focus`:
 * a small margin above and a generous one below, so a keypress costs O(window) not O(library).
 * Mirrors `items.slice(lo, hi)` (hi past the end is fine — clamped to `total` here). */
export function railWindow(
  total: number,
  focus: number,
  above: number,
  below: number,
): { lo: number; hi: number } {
  const lo = Math.max(0, focus - above);
  const hi = Math.min(Math.max(total, 0), focus + below);
  return { lo, hi };
}
