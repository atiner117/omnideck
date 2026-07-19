<!-- Continue Watching row (Jellyfin resume rail). SELF-CONTAINED on purpose — it fetches its
     own items/posters through the typed backend layer, so mounting it anywhere is one import
     plus one tag; the +page integration pass owns where it lands and any input-router focus
     wiring (the row is pointer/keyboard-usable on its own via real <button>s).

     Card click resumes playback from `position_secs` (backend threads it to mpv --start);
     the small ✓ affordance marks the item watched and drops it from the row. Renders nothing
     at all when no server is configured or the rail is empty — safe to mount unconditionally. -->
<script lang="ts">
  import { onMount } from "svelte";
  import * as api from "./backend";
  import type { MediaItem } from "./backend";

  let {
    onplayed = undefined,
    onerror = undefined,
  }: {
    /** Playback was handed to mpv (e.g. close the overlay the row lives in). */
    onplayed?: (item: MediaItem) => void;
    /** Surface a failure toast — errors are otherwise shown inline in the row. */
    onerror?: (message: string) => void;
  } = $props();

  let items = $state<MediaItem[]>([]);
  let posters = $state<Record<string, string>>({});
  let loading = $state(true);
  let error = $state("");

  // Same omnideck:// wrapping as +page.svelte's artUrl: the asset protocol serves the
  // backend's cached poster file; each path segment is percent-encoded.
  function artUrl(path: string): string {
    return "omnideck://localhost" + path.split("/").map(encodeURIComponent).join("/");
  }

  /** Re-fetch the rail (exported so a parent can refresh after playback exits). */
  export async function refresh(): Promise<void> {
    error = "";
    try {
      if (!(await api.mediaAvailable())) {
        items = [];
        return;
      }
      items = await api.getContinueWatching();
      for (const it of items) {
        if (posters[it.id] !== undefined) continue;
        posters[it.id] = ""; // inflight marker (fallback glyph renders meanwhile)
        api.mediaPoster(it.id)
          .then((p) => { if (p) posters = { ...posters, [it.id]: artUrl(p) }; })
          .catch(() => { /* poster is decoration; the card works without it */ });
      }
    } catch (e) {
      error = String(e);
      onerror?.(error);
    } finally {
      loading = false;
    }
  }

  onMount(() => { void refresh(); });

  /** 0–100 progress for the card's bar (server pct first, ticks-derived fallback). */
  function progressPct(it: MediaItem): number {
    if (it.played_pct != null) return Math.min(100, Math.max(0, it.played_pct));
    const pos = it.position_secs != null ? Number(it.position_secs) : 0;
    const total = it.runtime_mins != null ? Number(it.runtime_mins) * 60 : 0;
    return total > 0 ? Math.min(100, (pos / total) * 100) : 0;
  }

  /** "42 min left" — only when both sides of the subtraction are known. */
  function remaining(it: MediaItem): string {
    if (it.position_secs == null || it.runtime_mins == null) return "";
    const left = Number(it.runtime_mins) - Math.floor(Number(it.position_secs) / 60);
    return left > 0 ? `${left} min left` : "";
  }

  function subtitle(it: MediaItem): string {
    return it.series ?? (it.kind === "Movie" ? remaining(it) : it.kind);
  }

  async function play(it: MediaItem) {
    try {
      const start = it.position_secs != null ? Number(it.position_secs) : undefined;
      await api.mediaPlay(it.id, it.name, start);
      onplayed?.(it);
    } catch (e) {
      error = String(e);
      onerror?.(error);
    }
  }

  async function markWatched(it: MediaItem) {
    try {
      await api.markWatched(it.id);
      // Watched = no longer "continue watching": drop the card, matching what the server
      // will say on the next refresh (Jellyfin clears the resume point with the flag).
      items = items.filter((x) => x.id !== it.id);
    } catch (e) {
      error = String(e);
      onerror?.(error);
    }
  }
</script>

{#if !loading && (items.length || error)}
  <section class="cwrow" aria-label="Continue watching">
    <h3>Continue Watching</h3>
    {#if error}
      <p class="cwerr" role="alert">{error}</p>
    {/if}
    <div class="cwcards">
      {#each items as it (it.id)}
        <div class="cwcard">
          <button
            type="button"
            class="cwposter"
            title={remaining(it) ? `${it.name} — ${remaining(it)}` : it.name}
            aria-label={`Resume ${it.name}`}
            onclick={() => play(it)}
          >
            {#if posters[it.id]}
              <img src={posters[it.id]} alt="" loading="lazy" />
            {:else}
              <span class="cwglyph">🎬</span>
            {/if}
            <span class="cwbar" style:width={`${progressPct(it)}%`}></span>
          </button>
          <button
            type="button"
            class="cwdone"
            title="Mark watched"
            aria-label={`Mark ${it.name} watched`}
            onclick={() => markWatched(it)}
          >✓</button>
          <div class="cwname">{it.name}</div>
          {#if subtitle(it)}<div class="cwsub">{subtitle(it)}</div>{/if}
        </div>
      {/each}
    </div>
  </section>
{/if}

<style>
  /* Colors read the design tokens where they exist (integration branch / Lane A themes) and
     fall back to today's literal values, so the row looks identical standalone but follows
     the active theme + user accent once mounted under the tokenized tree. */
  .cwrow h3 { margin: 0 0 8px; font-size: 15px; font-weight: 600; opacity: 0.85; }
  .cwerr { margin: 0 0 8px; font-size: 13px; color: #f0a0a0; }
  .cwcards { display: flex; gap: 12px; overflow-x: auto; padding-bottom: 6px; scrollbar-width: thin; }
  .cwcard { position: relative; flex: 0 0 132px; }
  /* Poster button: 2:3 like real box art (MediaModal's .mposter, row-card sized). */
  .cwposter { position: relative; display: grid; place-items: center; width: 132px; height: 198px;
    padding: 0; border: 0; border-radius: 10px; overflow: hidden; cursor: pointer;
    background: var(--surface-3, #22304a); }
  .cwposter img { width: 100%; height: 100%; object-fit: cover; }
  .cwposter:focus-visible { outline: 3px solid var(--accent, #7aa2ff); outline-offset: 2px; }
  .cwglyph { font-size: 34px; }
  /* Resume-progress bar pinned to the poster's bottom edge. */
  .cwbar { position: absolute; left: 0; bottom: 0; height: 4px; background: var(--accent, #7aa2ff); }
  .cwdone { position: absolute; top: 6px; right: 6px; width: 26px; height: 26px;
    border: 0; border-radius: 50%; cursor: pointer; font-size: 14px; line-height: 1;
    color: var(--text, #dfe7f5); background: rgba(10, 16, 28, 0.75); }
  .cwdone:hover, .cwdone:focus-visible { background: var(--accent, #7aa2ff); color: #0a101c; outline: none; }
  .cwname { margin-top: 6px; font-size: 13px; font-weight: 600; white-space: nowrap;
    overflow: hidden; text-overflow: ellipsis; }
  .cwsub { font-size: 12px; opacity: 0.65; white-space: nowrap; overflow: hidden;
    text-overflow: ellipsis; }
</style>
