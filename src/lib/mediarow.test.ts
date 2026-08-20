import { describe, it, expect } from "vitest";
import { BROWSE_KINDS, rowStartSecs, rowSub } from "./mediarow";
import type { MediaItem } from "./bindings/MediaItem";

// ts-rs types u64 fields as bigint; build items the way the IPC layer types them.
function item(over: Partial<MediaItem>): MediaItem {
  return {
    id: "x", name: "X", kind: "Movie", overview: null,
    played_pct: null, runtime_mins: null, series: null,
    position_secs: null, played: null,
    ...over,
  };
}

describe("rowStartSecs", () => {
  it("passes a playable row's position through as a number", () => {
    expect(rowStartSecs(item({ position_secs: 2850n }), false)).toBe(2850);
  });
  it("is undefined with no position, and ALWAYS undefined on browse rows", () => {
    expect(rowStartSecs(item({}), false)).toBeUndefined();
    // Jellyfin populates aggregate UserData on containers — a folder must never seek.
    expect(rowStartSecs(item({ kind: "Series", position_secs: 900n }), true)).toBeUndefined();
  });
});

describe("rowSub", () => {
  it("shows time left on a resumable movie", () => {
    const i = item({ played_pct: 52.4, runtime_mins: 120n, position_secs: 3600n });
    expect(rowSub(i, false)).toBe("52% · 60 min left");
  });
  it("shows time left on a resumable EPISODE too (the dominant Continue Watching row)", () => {
    const i = item({
      kind: "Episode", series: "Some Show",
      played_pct: 43.0, runtime_mins: 45n, position_secs: 1620n, // 27 min in
    });
    expect(rowSub(i, false)).toBe("43% · Some Show · 18 min left");
  });
  it("never claims the full runtime on a nearly-finished item (double flooring)", () => {
    // 90-min runtime resumed at 90:05 — left computes 0; the row must not read "90 min".
    const i = item({ runtime_mins: 90n, position_secs: 5405n });
    expect(rowSub(i, false)).toBe("<1 min left");
  });
  it("keeps the original labels for untouched items", () => {
    expect(rowSub(item({ runtime_mins: 120n }), false)).toBe("120 min");
    expect(rowSub(item({ kind: "Episode", series: "Some Show", played_pct: 10 }), false)).toBe("10% · Some Show");
    expect(rowSub(item({ kind: "Special" }), false)).toBe("special");
  });
  it("never shows a resume label on a browse row, even when the server sends a position", () => {
    const i = item({ kind: "Season", series: "Some Show", runtime_mins: 400n, position_secs: 900n });
    expect(rowSub(i, true)).toBe("Some Show");
  });
  it("falls back to the plain label when a resumable item has no runtime", () => {
    expect(rowSub(item({ position_secs: 300n }), false)).toBe("movie");
  });
});

describe("BROWSE_KINDS", () => {
  it("contains exactly the drill-down kinds", () => {
    expect([...BROWSE_KINDS].sort()).toEqual(
      ["BoxSet", "CollectionFolder", "Folder", "Season", "Series"].sort());
  });
});
