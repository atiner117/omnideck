// A rail/list entry: either a Steam game or a catalog/custom app, tagged with the
// category it appears under. Built by the page, rendered by the rail and SearchModal.
import type { App, Game } from "./backend";

export type Tile =
  | { kind: "game"; id: string; cat: string; game: Game }
  | { kind: "app"; id: string; cat: string; app: App };
