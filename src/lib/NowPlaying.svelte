<script lang="ts">
  // Now Playing card stack (bottom right): one card per launch-tracked entry, enriched
  // with live MPRIS metadata when the entry is a media app, plus a standalone card for
  // media OmniDeck didn't launch. Pure presentation + direct media/app IPC — the launch
  // list itself lives in the page (it's shared with launch/exit tracking).
  import * as api from "$lib/backend";
  import type { MediaInfo } from "$lib/backend";

  export type NowCard = { id: string; kind: string; name: string; category: string; media: MediaInfo | null };

  let {
    cards,
    inSession,
    ondismiss,
    onerror,
  }: {
    cards: NowCard[];
    /** gamescope session? gates the ⇄ switch button (desktop WMs manage their own windows) */
    inSession: boolean;
    ondismiss: (id: string) => void;
    onerror: (ctx: string, e: unknown) => void;
  } = $props();

  function mediaControl(action: string) {
    // no re-poll needed: the player's PropertiesChanged fires a `media-changed` event
    api.mediaControl(action).catch((e) => onerror("Media control failed", e));
  }
</script>

{#if cards.length}
  <div class="nowstack">
    {#each cards as c (c.id)}
      <div class="nowplaying">
        {#if c.media && c.media.status === "Playing"}<span class="np-eq"><i></i><i></i><i></i></span>
        {:else if c.media}<span class="np-icon">⏸</span>
        {:else}<span class="np-spinner"></span>{/if}
        <span class="np-label">
          {c.media ? "Now playing" : c.kind === "game" ? "Game running" : "Running"}<br />
          {#if c.media && c.media.title}<b>{c.media.title}</b>{#if c.media.artist}<span class="np-sub"> — {c.media.artist}</span>{/if}
          {:else}<b>{c.kind === "game" ? "🎮 " : "▶ "}{c.name}</b>{/if}
        </span>
        {#if c.media}
          <span class="np-controls">
            <button class="np-c" title="Previous" aria-label="Previous track" onclick={() => mediaControl("previous")}>⏮</button>
            <button class="np-c" title="Play / Pause" aria-label="Play or pause" onclick={() => mediaControl("play-pause")}>{c.media.status === "Playing" ? "⏸" : "▶"}</button>
            <button class="np-c" title="Next" aria-label="Next track" onclick={() => mediaControl("next")}>⏭</button>
          </span>
        {/if}
        <!-- ⇄ only in the gamescope session: on a desktop, unmap would hide the window from the real WM -->
        {#if c.kind === "app" && inSession}<button class="np-c" title="Switch to the app (Guide press / Ctrl+Alt+Home)" aria-label="Switch to app" onclick={() => api.switchApp().catch((e) => onerror("Couldn't switch app", e))}>⇄</button>{/if}
        {#if c.kind === "app"}<button class="np-c" title="Close &amp; return (Guide hold / Ctrl+Alt+End)" aria-label="Close app and return" onclick={() => api.closeCurrentApp().catch((e) => onerror("Couldn't close app", e))}>↩</button>{/if}
        {#if c.kind !== "media"}<button class="np-x" title="Dismiss (doesn't close the app)" aria-label="Dismiss card" onclick={() => ondismiss(c.id)}>✕</button>{/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  .nowstack { position: fixed; z-index: 12; right: 2.4vw; bottom: 8vh; display: flex; flex-direction: column; gap: 10px; align-items: flex-end; }
  .nowplaying { display: flex; align-items: center; gap: 16px; background: #0c1320e8; border: 1px solid color-mix(in srgb, var(--accent) 45%, transparent); border-radius: 16px; padding: 14px 20px; box-shadow: 0 20px 60px #000b; max-width: 42vw; }
  .np-spinner { width: 22px; height: 22px; border-radius: 50%; border: 3px solid #2c3a5c; border-top-color: var(--accent); animation: np-spin 0.9s linear infinite; flex: 0 0 auto; }
  @keyframes np-spin { to { transform: rotate(360deg); } }
  .np-icon { font-size: 20px; color: var(--accent); flex: 0 0 auto; }
  .np-eq { display: flex; align-items: flex-end; gap: 2px; height: 20px; flex: 0 0 auto; }
  .np-eq i { width: 4px; background: var(--accent); border-radius: 2px; animation: np-eq 0.9s ease-in-out infinite; }
  .np-eq i:nth-child(1) { animation-delay: 0s; } .np-eq i:nth-child(2) { animation-delay: 0.3s; } .np-eq i:nth-child(3) { animation-delay: 0.6s; }
  @keyframes np-eq { 0%, 100% { height: 6px; } 50% { height: 18px; } }
  .np-label { font-size: clamp(13px, 1.4vw, 17px); color: #9fb0c8; line-height: 1.3; min-width: 0; }
  .np-label b { color: #fff; font-size: 1.1em; }
  .np-sub { color: #9fb0c8; }
  .np-x { background: #1b2540; border: 1px solid #2c3a5c; color: #9fb0c8; border-radius: 8px; width: 30px; height: 30px; cursor: pointer; font-size: 14px; flex: 0 0 auto; }
  .np-x:hover { border-color: var(--accent); color: #fff; }
  .np-controls { display: flex; gap: 6px; flex: 0 0 auto; }
  .np-c { background: #1b2540; border: 1px solid #2c3a5c; color: #cdd7e6; border-radius: 8px; width: 32px; height: 32px; cursor: pointer; font-size: 14px; }
  .np-c:hover { border-color: var(--accent); color: #fff; }
  .np-c:focus, .np-x:focus { outline: none; }
  .np-c:focus-visible, .np-x:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  @media (prefers-reduced-motion: reduce) {
    .np-spinner, .np-eq i { animation: none; }
  }
</style>
