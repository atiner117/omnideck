// Library view modes (appearance.layout) + the pure 2D-nav math the grid modes share.
//
// The page stays the single owner of `focus` and of input routing; grid views are dumb
// renderers. These helpers keep the row/column arithmetic out of the page and testable:
//   - vertical moves wrap top<->bottom preserving the column (the rail's modulo wrap,
//     generalized to 2D),
//   - horizontal moves stay within the focused row and report "fell off the edge" with
//     null so the caller can keep the XMB rule that left/right always reaches the
//     category axis (you can never get trapped in a view).
export const LAYOUT_MODES = [
  { id: "rail", label: "Rail" },
  { id: "grid", label: "Large grid" },
  { id: "grid-compact", label: "Compact grid" },
  { id: "list", label: "List" },
] as const;
export type LayoutId = (typeof LAYOUT_MODES)[number]["id"];

/** Known mode or the default — a hand-edited config can hold anything. */
export function normalizeLayout(v: string | undefined): LayoutId {
  return (LAYOUT_MODES.some((m) => m.id === v) ? v : "rail") as LayoutId;
}

export const isGridLayout = (l: string) => l === "grid" || l === "grid-compact";

/**
 * Columns for a grid mode. `base` is settings.grid_columns (the user's density knob,
 * backend-clamped 1–12); the compact grid packs ~1.5x more per row.
 */
export function gridColumns(layout: string, base: number | undefined): number {
  const b = Math.max(1, Math.min(12, Math.round(base || 6)));
  return layout === "grid-compact" ? Math.min(18, Math.round(b * 1.5)) : b;
}

/**
 * Vertical grid move: wrap top<->bottom (like the rail's modulo wrap) but preserve the
 * column. Landing on a shorter last row clamps to its last item.
 */
export function gridMoveRow(focus: number, d: number, count: number, cols: number): number {
  if (count <= 0 || cols <= 0) return focus;
  const rows = Math.ceil(count / cols);
  const col = focus % cols;
  const row = (Math.floor(focus / cols) + d + rows) % rows;
  return Math.min(row * cols + col, count - 1);
}

/**
 * Horizontal grid move within the focused row. Returns the new index, or null when the
 * move falls off the row's edge (start/end of row, or past the last item) — the caller
 * treats that as "leave the grid" and switches category.
 */
export function gridMoveCol(focus: number, d: number, count: number, cols: number): number | null {
  if (count <= 0 || cols <= 0) return null;
  const t = focus + d;
  if (t < 0 || t >= count) return null;
  return Math.floor(t / cols) === Math.floor(focus / cols) ? t : null;
}
