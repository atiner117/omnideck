import { describe, it, expect } from "vitest";
import { BROWSE_KINDS, isBrowse, rowStartSecs, rowSub, rowSubPlain } from "./mediarow";
import type { MediaItem } from "./bindings/MediaItem";

// ts-rs types u64 fields as bigint; build items the way the IPC layer types them.
function item(over: Partial<MediaItem>): MediaItem {
  return {
    id: "x", name: "X", kind: "Movie", overview: null,
    played_pct: null, runtime_mins: null, series: null,
    position_secs: null, played: null, is_folder: null,
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

describe("rowSubPlain", () => {
  it("strips ALL progress decoration from a half-watched episode (post-toggle label)", () => {
    const i = item({
      kind: "Episode", series: "Some Show",
      played_pct: 43.0, runtime_mins: 45n, position_secs: 1620n,
    });
    expect(rowSub(i, false)).toBe("43% · Some Show · 18 min left"); // before the toggle
    expect(rowSubPlain(i)).toBe("Some Show"); // after: matches rowSub's untouched branch
  });
  it("falls back to runtime, then kind — same ladder as rowSub", () => {
    expect(rowSubPlain(item({ played_pct: 52.4, runtime_mins: 120n, position_secs: 3600n }))).toBe("120 min");
    expect(rowSubPlain(item({ kind: "Special" }))).toBe("special");
  });
  it("treats an empty SeriesName like rowSub does (truthiness, not ??)", () => {
    // Jellyfin can send SeriesName: "" — the sub-line must not go blank after a toggle.
    expect(rowSubPlain(item({ series: "", runtime_mins: 45n }))).toBe("45 min");
  });
});

describe("isBrowse", () => {
  it("trusts the server's IsFolder over the kind name, in BOTH directions", () => {
    // The gap this closes: a container kind the name list never heard of. Before, this
    // read as playable — Enter handed a folder id to mpv and W offered to mark the whole
    // album watched, clearing every child's resume point with no undo.
    expect(isBrowse(item({ kind: "MusicAlbum", is_folder: true }))).toBe(true);
    expect(isBrowse(item({ kind: "SomethingJellyfinAddsIn2027", is_folder: true }))).toBe(true);
    // And the other way: a server that calls something a Folder-ish name but says it plays.
    expect(isBrowse(item({ kind: "Folder", is_folder: false }))).toBe(false);
  });

  it("falls back to the kind list when the server omitted IsFolder", () => {
    expect(isBrowse(item({ kind: "Series" }))).toBe(true);
    expect(isBrowse(item({ kind: "Movie" }))).toBe(false);
    expect(isBrowse(item({ kind: "Episode" }))).toBe(false);
  });

  it("does not treat a missing IsFolder as false (?? not ||, null is the unknown case)", () => {
    // is_folder: false is a real answer and must survive; null must NOT.
    expect(isBrowse(item({ kind: "Series", is_folder: null }))).toBe(true);
    expect(isBrowse(item({ kind: "Series", is_folder: false }))).toBe(false);
  });
});

describe("BROWSE_KINDS", () => {
  it("lists only containers — a playable kind here would break Enter on every server", () => {
    // The fallback list can never be complete (that's why isBrowse prefers is_folder), but
    // a FALSE POSITIVE here is the damaging direction: it would make a real movie undrillable.
    for (const playable of ["Movie", "Episode", "Video", "Audio", "MusicVideo", "Trailer"])
      expect(BROWSE_KINDS.has(playable)).toBe(false);
    // The original five must not have been dropped while widening the list.
    for (const container of ["BoxSet", "CollectionFolder", "Folder", "Season", "Series"])
      expect(BROWSE_KINDS.has(container)).toBe(true);
  });
});
