// Per-launch instance ids. Format contract: `<baseId>#<seq>` — the seq makes each launch
// unique, so relaunching a tile while an earlier instance is still alive gives each its own
// Now Playing card / exit-correlation key (sharing the bare tile id meant the older
// instance's exit event cleared the newer card too). The backend mints media launch keys in
// the same shape (`media-{itemId}#{seq}`, commands.rs media_play_blocking) — baseId()
// parses both; keep the two minters' format in step.

let seq = 0;

/** Mint a fresh, unique per-launch id for `base` (a tile id). */
export function mintLaunchId(base: string): string {
  return `${base}#${++seq}`;
}

/** The base (tile) id a launch id was minted from — the favorites/recents/icon lookup key. */
export function baseId(launchId: string): string {
  return launchId.split("#")[0];
}
