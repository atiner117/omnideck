import { describe, it, expect, vi, beforeEach, type Mock } from "vitest";
import { cardActions, type NowCard, type NpAction } from "./npActions";
import type { MediaInfo } from "./bindings/MediaInfo";

// npActions is the ONE definition of the Now Playing control set, consumed by both the card
// stack and the pad overlay — so these tests pin the gating and the call arguments, which are
// exactly what silently drifted when the two surfaces each owned a copy.
vi.mock("./backend", () => ({
  mediaControl: vi.fn(() => Promise.resolve()),
  switchApp: vi.fn(() => Promise.resolve(true)),
  closeCurrentApp: vi.fn(() => Promise.resolve(true)),
}));
import * as api from "./backend";

const mediaControl = vi.mocked(api.mediaControl);
const switchApp = vi.mocked(api.switchApp);
const closeCurrentApp = vi.mocked(api.closeCurrentApp);

// `run()` fires IPC and swallows the rejection in a .catch — let those microtasks settle.
const flush = () => new Promise((r) => setTimeout(r, 0));

function card(over: Partial<NowCard> = {}): NowCard {
  return { id: "app-1", kind: "app", name: "Steam", category: "game", media: null, ...over };
}
function info(over: Partial<MediaInfo> = {}): MediaInfo {
  return { status: "Playing", title: "T", artist: "A", player: "mpv", ...over };
}
/** what a surface actually renders: the labels, in order */
const labels = (a: NpAction[]) => a.map((x) => x.label);
const pick = (a: NpAction[], label: string) => a.find((x) => x.label === label)!;

// spelled out, not ReturnType<typeof vi.fn> — the bare Mock type drops the signature and the
// callbacks then don't satisfy cardActions' options parameter.
type Opts = Parameters<typeof cardActions>[1];
let onerror: Mock<Opts["onerror"]>;
let ondismiss: Mock<Opts["ondismiss"]>;
let after: Mock<() => void>;
const opts = (over: Partial<Opts> = {}): Opts => ({ inSession: true, onerror, ondismiss, after, ...over });

beforeEach(() => {
  vi.clearAllMocks();
  mediaControl.mockResolvedValue(undefined);
  switchApp.mockResolvedValue(true);
  closeCurrentApp.mockResolvedValue(true);
  onerror = vi.fn<Opts["onerror"]>();
  ondismiss = vi.fn<Opts["ondismiss"]>();
  after = vi.fn<() => void>();
});

describe("cardActions — which controls appear", () => {
  it("gives a launched app switch + close + dismiss, in that order", () => {
    expect(labels(cardActions(card(), opts()))).toEqual(["Switch to app", "Close & return", "Dismiss card"]);
  });

  it("drops ⇄ outside a gamescope session — a desktop WM manages its own windows", () => {
    const a = cardActions(card(), opts({ inSession: false }));
    expect(labels(a)).toEqual(["Close & return", "Dismiss card"]);
    expect(a.some((x) => x.icon === "⇄")).toBe(false);
  });

  it("puts the transport group first when the card carries MPRIS metadata", () => {
    const a = cardActions(card({ media: info() }), opts());
    expect(labels(a)).toEqual(["Previous", "Play / Pause", "Next", "Switch to app", "Close & return", "Dismiss card"]);
    expect(a.slice(0, 3).every((x) => x.kind === "media")).toBe(true);
  });

  it("gives the standalone unlaunched-media card transport only — nothing to switch, close or dismiss", () => {
    // kind "media" is the synthetic card for a player omnideck never launched: it owns no
    // launch id, so ✕ must not offer to dismiss it.
    const a = cardActions(card({ kind: "media", media: info() }), opts());
    expect(labels(a)).toEqual(["Previous", "Play / Pause", "Next"]);
  });

  it("gives a non-app, non-media card (a launched movie/link) only the ✕", () => {
    expect(labels(cardActions(card({ kind: "link", media: null }), opts()))).toEqual(["Dismiss card"]);
  });

  it("flips the play/pause icon on status, and only that icon", () => {
    const playing = pick(cardActions(card({ media: info({ status: "Playing" }) }), opts()), "Play / Pause");
    const paused = pick(cardActions(card({ media: info({ status: "Paused" }) }), opts()), "Play / Pause");
    expect(playing.icon).toBe("⏸");
    expect(paused.icon).toBe("▶");
    // an unknown/stopped status must read as "not playing", not blank out the control
    expect(pick(cardActions(card({ media: info({ status: "Stopped" }) }), opts()), "Play / Pause").icon).toBe("▶");
    expect(paused.label).toBe(playing.label);
  });

  it("labels every action for the pad overlay and screen readers", () => {
    for (const a of cardActions(card({ media: info() }), opts())) {
      expect(a.label).not.toBe("");
      expect(a.title).not.toBe("");
      expect(a.aria).not.toBe("");
    }
  });
});

describe("cardActions — what run() actually calls", () => {
  it("maps the three transport buttons onto their MPRIS actions", () => {
    const a = cardActions(card({ media: info() }), opts());
    pick(a, "Previous").run();
    pick(a, "Play / Pause").run();
    pick(a, "Next").run();
    expect(mediaControl.mock.calls.map((c) => c[0])).toEqual(["previous", "play-pause", "next"]);
  });

  it("passes THIS card's launch id to switchApp — the drift this module exists to prevent", () => {
    cardActions(card({ id: "app-42" }), opts()).find((x) => x.icon === "⇄")!.run();
    expect(switchApp).toHaveBeenCalledWith("app-42");
  });

  it("dismisses by the card's id, without closing the app", () => {
    pick(cardActions(card({ id: "app-7" }), opts()), "Dismiss card").run();
    expect(ondismiss).toHaveBeenCalledWith("app-7");
    expect(closeCurrentApp).not.toHaveBeenCalled();
  });
});

describe("cardActions — the `after` hook (the overlay closes itself with it)", () => {
  it("runs after each terminal action", () => {
    for (const label of ["Switch to app", "Close & return", "Dismiss card"]) {
      after.mockClear();
      pick(cardActions(card(), opts()), label).run();
      expect(after, label).toHaveBeenCalledTimes(1);
    }
  });

  it("does NOT run on transport — the overlay stays open while you skip tracks", () => {
    const a = cardActions(card({ media: info() }), opts());
    for (const l of ["Previous", "Play / Pause", "Next"]) pick(a, l).run();
    expect(after).not.toHaveBeenCalled();
  });

  it("fires even when the IPC rejects — the overlay must not stick open on a backend error", async () => {
    closeCurrentApp.mockRejectedValueOnce(new Error("nope"));
    pick(cardActions(card(), opts()), "Close & return").run();
    expect(after).toHaveBeenCalledTimes(1);
    await flush();
    expect(onerror).toHaveBeenCalledTimes(1);
  });

  it("is optional — the card stack omits it and must not throw", () => {
    const a = cardActions(card({ media: info() }), { inSession: true, onerror, ondismiss });
    for (const x of a) expect(() => x.run()).not.toThrow();
    expect(ondismiss).toHaveBeenCalledWith("app-1");
  });
});

describe("cardActions — every failure path reaches onerror with its own context", () => {
  it.each([
    ["Play / Pause", () => mediaControl.mockRejectedValueOnce(new Error("bus down")), "Media control failed"],
    ["Switch to app", () => switchApp.mockRejectedValueOnce(new Error("no window")), "Couldn't switch app"],
    ["Close & return", () => closeCurrentApp.mockRejectedValueOnce(new Error("gone")), "Couldn't close app"],
  ])("%s → %s", async (label, arm, ctx) => {
    arm();
    pick(cardActions(card({ media: info() }), opts()), label).run();
    await flush();
    expect(onerror).toHaveBeenCalledTimes(1);
    expect(onerror.mock.calls[0][0]).toBe(ctx);
    expect(onerror.mock.calls[0][1]).toBeInstanceOf(Error);
  });

  it("stays quiet when the IPC resolves", async () => {
    for (const x of cardActions(card({ media: info() }), opts())) x.run();
    await flush();
    expect(onerror).not.toHaveBeenCalled();
  });
});
