// The Now Playing card's control set — ONE definition consumed by both surfaces that render
// it: the pointer-clickable corner card stack (NowPlaying.svelte) and the pad-navigable
// transport overlay (+page.svelte). The switchApp(c.id) fix once had to land in both files;
// building the list here makes that class of silent drift impossible.
import * as api from "./backend";
import type { MediaInfo } from "./backend";

/** One Now Playing entry: a launch-tracked app/game/media instance, enriched with live
 * MPRIS metadata when it's a media app (or the standalone unlaunched-media card). */
export type NowCard = { id: string; kind: string; name: string; category: string; media: MediaInfo | null };

export type NpAction = {
  /** media = transport group; app = switch/close; dismiss = the ✕ (styled apart on the card) */
  kind: "media" | "app" | "dismiss";
  icon: string;
  /** short name — the overlay's rows and focus hint */
  label: string;
  /** card-stack tooltip (may carry extra hotkey hints) */
  title: string;
  aria: string;
  run: () => void;
};

/** Build `c`'s gated action list. `after` runs after the terminal actions (switch, close,
 * dismiss) — the overlay closes itself with it; the card stack omits it. */
export function cardActions(
  c: NowCard,
  o: {
    /** gamescope session? gates the ⇄ switch (desktop WMs manage their own windows) */
    inSession: boolean;
    onerror: (ctx: string, e: unknown) => void;
    ondismiss: (id: string) => void;
    after?: () => void;
  },
): NpAction[] {
  const after = o.after ?? (() => {});
  // no re-poll needed: the player's PropertiesChanged fires a `media-changed` event
  const ctl = (action: string) => api.mediaControl(action).catch((e) => o.onerror("Media control failed", e));
  const a: NpAction[] = [];
  if (c.media) {
    a.push({ kind: "media", icon: "⏮", label: "Previous", title: "Previous", aria: "Previous track", run: () => ctl("previous") });
    a.push({
      kind: "media",
      icon: c.media.status === "Playing" ? "⏸" : "▶",
      label: "Play / Pause",
      title: "Play / Pause",
      aria: "Play or pause",
      run: () => ctl("play-pause"),
    });
    a.push({ kind: "media", icon: "⏭", label: "Next", title: "Next", aria: "Next track", run: () => ctl("next") });
  }
  // ⇄ only in the gamescope session: on a desktop, unmap would hide the window from the real
  // WM. Passes the card's launch id so only THIS app comes forward (not every hidden one).
  if (c.kind === "app" && o.inSession)
    a.push({
      kind: "app",
      icon: "⇄",
      label: "Switch to app",
      title: "Switch to the app",
      aria: "Switch to app",
      run: () => { api.switchApp(c.id).catch((e) => o.onerror("Couldn't switch app", e)); after(); },
    });
  if (c.kind === "app")
    a.push({
      kind: "app",
      icon: "↩",
      label: "Close & return",
      title: "Close & return (Guide hold / Ctrl+Alt+End)",
      aria: "Close app and return",
      run: () => { api.closeCurrentApp().catch((e) => o.onerror("Couldn't close app", e)); after(); },
    });
  if (c.kind !== "media")
    a.push({
      kind: "dismiss",
      icon: "✕",
      label: "Dismiss card",
      title: "Dismiss (doesn't close the app)",
      aria: "Dismiss card",
      run: () => { o.ondismiss(c.id); after(); },
    });
  return a;
}
