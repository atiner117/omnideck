import { describe, it, expect } from "vitest";
import { normalizeLayout, isGridLayout, gridColumns, gridMoveRow, gridMoveCol } from "./layouts";

describe("normalizeLayout", () => {
  it("passes every known mode through", () => {
    for (const id of ["rail", "grid", "grid-compact", "list"]) {
      expect(normalizeLayout(id)).toBe(id);
    }
  });
  it("resets unknown or missing values to the rail default", () => {
    expect(normalizeLayout("mosaic")).toBe("rail");
    expect(normalizeLayout(undefined)).toBe("rail");
    expect(normalizeLayout("")).toBe("rail");
  });
});

describe("isGridLayout", () => {
  it("is true for exactly the two grid modes", () => {
    expect(isGridLayout("grid")).toBe(true);
    expect(isGridLayout("grid-compact")).toBe(true);
    expect(isGridLayout("rail")).toBe(false);
    expect(isGridLayout("list")).toBe(false);
  });
});

describe("gridColumns", () => {
  it("defaults to 6 when the knob is unset or 0", () => {
    expect(gridColumns("grid", undefined)).toBe(6);
    expect(gridColumns("grid", 0)).toBe(6);
  });
  it("clamps the knob to 1-12 (the backend's normalize range)", () => {
    expect(gridColumns("grid", 1)).toBe(1);
    expect(gridColumns("grid", -5)).toBe(1);
    expect(gridColumns("grid", 99)).toBe(12);
  });
  it("packs the compact grid ~1.5x denser, capped at 18", () => {
    expect(gridColumns("grid-compact", 6)).toBe(9);
    expect(gridColumns("grid-compact", 12)).toBe(18);
    expect(gridColumns("grid-compact", 1)).toBe(2);
  });
});

// A 12-item grid at 4 columns: rows are [0-3], [4-7], [8-11].
describe("gridMoveRow", () => {
  it("moves a row down / up preserving the column", () => {
    expect(gridMoveRow(1, 1, 12, 4)).toBe(5);
    expect(gridMoveRow(5, -1, 12, 4)).toBe(1);
  });
  it("wraps top<->bottom preserving the column", () => {
    expect(gridMoveRow(1, -1, 12, 4)).toBe(9);
    expect(gridMoveRow(9, 1, 12, 4)).toBe(1);
  });
  it("clamps to the last item when landing on a shorter last row", () => {
    // 10 items, 4 cols: last row is [8, 9]. Column 3 has no cell there.
    expect(gridMoveRow(7, 1, 10, 4)).toBe(9);
    expect(gridMoveRow(3, -1, 10, 4)).toBe(9); // wrap up into the short row clamps too
  });
  it("returns focus unchanged on a single-row grid (the page skips the move blip)", () => {
    expect(gridMoveRow(2, 1, 3, 4)).toBe(2);
    expect(gridMoveRow(2, -1, 3, 4)).toBe(2);
  });
  it("is safe for an empty grid or zero columns", () => {
    expect(gridMoveRow(0, 1, 0, 4)).toBe(0);
    expect(gridMoveRow(5, 1, 12, 0)).toBe(5);
  });
});

describe("gridMoveCol", () => {
  it("moves within the focused row", () => {
    expect(gridMoveCol(5, 1, 12, 4)).toBe(6);
    expect(gridMoveCol(6, -1, 12, 4)).toBe(5);
  });
  it("returns null when falling off the row's edge (the page switches category)", () => {
    expect(gridMoveCol(3, 1, 12, 4)).toBeNull(); // end of row 0 -> row 1 = off the edge
    expect(gridMoveCol(4, -1, 12, 4)).toBeNull(); // start of row 1 -> row 0
    expect(gridMoveCol(0, -1, 12, 4)).toBeNull(); // before the first item
    expect(gridMoveCol(9, 1, 10, 4)).toBeNull(); // past the last item on a short row
  });
  it("is safe for an empty grid or zero columns", () => {
    expect(gridMoveCol(0, 1, 0, 4)).toBeNull();
    expect(gridMoveCol(0, 1, 12, 0)).toBeNull();
  });
});
