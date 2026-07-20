import { describe, it, expect } from "vitest";
import { OSK_ROWS, OSK_FLAT, OSK_COLS } from "./osk";

// The page's D-pad routing assumes a rectangular grid: every row is OSK_COLS wide and
// OSK_FLAT is the row-major flattening. If someone edits the layout and breaks either
// invariant, cursor movement in the on-screen keyboard silently goes wrong — catch it here.
describe("OSK layout", () => {
  it("every row is exactly OSK_COLS wide", () => {
    for (const row of OSK_ROWS) expect(row.length).toBe(OSK_COLS);
  });
  it("OSK_FLAT is the row-major flattening", () => {
    expect(OSK_FLAT).toEqual(OSK_ROWS.flat());
    expect(OSK_FLAT.length).toBe(OSK_ROWS.length * OSK_COLS);
  });
  it("has no duplicate keys", () => {
    expect(new Set(OSK_FLAT).size).toBe(OSK_FLAT.length);
  });
  it("includes the essential action keys", () => {
    for (const k of ["␣", "⌫", "✕", "⏎"]) expect(OSK_FLAT).toContain(k);
  });
});
