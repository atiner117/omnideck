// Media-library browse/nav state (Jellyfin → MediaModal), extracted from +page.svelte
// (review #9). The class owns the modal's open/loading/view-stack/focus/poster state and
// the browse drill-down flow; the page keeps what is genuinely page-level — input routing,
// the status toast, the Now-Playing card list, and error reporting — injected as callbacks.
import * as api from "./backend";
import type { MediaItem } from "./backend";
import type { MediaRow } from "./MediaModal.svelte";

const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));

export class MediaNav {
  open = $state(false);
  loading = $state(false);
  stack = $state<{ title: string; rows: MediaRow[] }[]>([]);
  focus = $state(0);
  posters = $state<Record<string, string>>({});
  readonly view = $derived(this.stack[this.stack.length - 1]);

  constructor(
    private deps: {
      onerror: (ctx: string, e: unknown) => void;
      /** Play is a page concern: it owns the ▶ status toast and the Now-Playing cards. */
      onplay: (id: string, name: string) => void;
      /** Stop any in-progress hold-repeat when the modal opens over the rail. */
      holdstop: () => void;
    },
  ) {}

  private row(i: MediaItem, group?: string): MediaRow {
    const browse = ["Series", "Season", "Folder", "BoxSet", "CollectionFolder"].includes(i.kind);
    const pct = i.played_pct ? `${Math.round(i.played_pct)}% · ` : "";
    const mins = i.runtime_mins ? `${i.runtime_mins} min` : i.kind.toLowerCase();
    const sub = i.series ? `${pct}${i.series}` : `${pct}${mins}`;
    return { id: i.id, name: i.name, sub, group, browse };
  }

  async openLibrary() {
    this.deps.holdstop();
    this.open = true;
    this.loading = true;
    this.stack = [];
    this.focus = 0;
    try {
      const s = await api.mediaSections();
      // An item can be in BOTH resume and latest — drop the duplicate (also: a keyed
      // {#each} throws on duplicate keys, which silently blanks the whole list).
      const seen = new Set(s.resume.map((i) => i.id));
      this.stack = [{
        title: s.server_name,
        rows: [
          ...s.resume.map((i) => this.row(i, "Continue watching")),
          ...s.latest.filter((i) => !seen.has(i.id)).map((i) => this.row(i, "Latest")),
          ...s.libraries.map((l) => ({ id: l.id, name: l.name, sub: l.kind, group: "Libraries", browse: true })),
        ],
      }];
    } catch (e) {
      this.deps.onerror("Media library", e);
      this.open = false;
    }
    this.loading = false;
  }

  async activate() {
    const r = this.view?.rows[this.focus];
    if (!r || this.loading) return;
    if (r.browse) {
      this.loading = true;
      try {
        const items = await api.mediaBrowse(r.id);
        this.stack = [...this.stack, { title: r.name, rows: items.map((i) => this.row(i)) }];
        this.focus = 0;
      } catch (e) {
        this.deps.onerror("Media library", e);
      }
      this.loading = false;
    } else {
      this.open = false;
      this.deps.onplay(r.id, r.name);
    }
  }

  back() {
    if (this.stack.length > 1) {
      this.stack = this.stack.slice(0, -1);
      this.focus = 0;
    } else this.open = false;
  }

  move(d: number) {
    const n = this.view?.rows.length ?? 0;
    if (!n) return;
    this.focus = clamp(this.focus + d, 0, n - 1);
    queueMicrotask(() => document.querySelector(`[data-med="${this.focus}"]`)?.scrollIntoView({ block: "nearest" }));
  }
}
