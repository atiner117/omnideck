import { describe, it, expect } from "vitest";
import { clamp, railWindow } from "./nav";

describe("clamp", () => {
  it("passes values already in range through", () => {
    expect(clamp(5, 0, 10)).toBe(5);
  });
  it("clamps below and above the bounds", () => {
    expect(clamp(-3, 0, 10)).toBe(0);
    expect(clamp(99, 0, 10)).toBe(10);
  });
  it("handles the single-point range used for a 1-item rail", () => {
    expect(clamp(4, 0, 0)).toBe(0);
  });
});

describe("railWindow", () => {
  it("keeps a small margin above and a generous one below the focus", () => {
    expect(railWindow(1000, 100, 8, 40)).toEqual({ lo: 92, hi: 140 });
  });
  it("never returns a negative lo near the top", () => {
    expect(railWindow(1000, 2, 8, 40)).toEqual({ lo: 0, hi: 42 });
  });
  it("clamps hi to the total near the bottom (matches items.slice past the end)", () => {
    expect(railWindow(50, 45, 8, 40)).toEqual({ lo: 37, hi: 50 });
  });
  it("is safe for an empty rail", () => {
    expect(railWindow(0, 0, 8, 40)).toEqual({ lo: 0, hi: 0 });
  });
});
