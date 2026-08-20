// Pure row-labeling logic for the media modal (extracted from MediaNav.row() so the
// sub-label rules are unit-testable — same pattern as nav.ts/osk.ts/layouts.ts).
//
// The resume rules live HERE, once:
//   - browse rows (a series/season/folder) have no position of their own to resume from,
//     so they get neither a seek target nor a remaining-time label — Jellyfin does
//     populate aggregate UserData on containers, and a "min left" label on a 📁 row
//     would promise something activate() deliberately refuses to do (it drills in);
//   - on a resumable row, what's LEFT is the useful number — and that includes
//     episodes, which carry a series name AND are the dominant Continue Watching
//     content ("43% · Some Show · 18 min left");
//   - both runtime_mins and position_secs are independently floored upstream, so a
//     nearly-finished item can compute 0 minutes left while a real position remains —
//     render "<1 min left" rather than falling back to the total runtime, which would
//     make the row look untouched on the one row that's about to resume into credits.
import type { MediaItem } from "./bindings/MediaItem";

/** Item kinds that drill down instead of playing. Drives both navigation (activate()
 *  browses) and resume gating (no seek target) — one list, module-level, no per-row
 *  array allocation. */
export const BROWSE_KINDS = new Set(["Series", "Season", "Folder", "BoxSet", "CollectionFolder"]);

/** The row's resume point in seconds, or undefined when activating should play from the
 *  top (browse rows never resume). ts-rs types u64 as bigint, so arithmetic needs Number. */
export function rowStartSecs(i: MediaItem, browse: boolean): number | undefined {
  return !browse && i.position_secs != null ? Number(i.position_secs) : undefined;
}

/** The sub-label under the row name: watched %, series, and remaining/total runtime. */
export function rowSub(i: MediaItem, browse: boolean): string {
  const pct = i.played_pct ? `${Math.round(i.played_pct)}% · ` : "";
  const runtime = i.runtime_mins != null ? Number(i.runtime_mins) : undefined;
  const startSecs = rowStartSecs(i, browse);
  if (startSecs !== undefined && runtime !== undefined) {
    const left = runtime - Math.floor(startSecs / 60);
    const leftLabel = left > 0 ? `${left} min left` : "<1 min left";
    return i.series ? `${pct}${i.series} · ${leftLabel}` : `${pct}${leftLabel}`;
  }
  const mins = runtime ? `${runtime} min` : i.kind.toLowerCase();
  return i.series ? `${pct}${i.series}` : `${pct}${mins}`;
}
