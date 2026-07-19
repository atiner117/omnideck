<script lang="ts">
  // Now Playing card stack (bottom right): one card per launch-tracked entry, enriched
  // with live MPRIS metadata when the entry is a media app, plus a standalone card for
  // media OmniDeck didn't launch. Pure presentation — the control set (and its IPC) comes
  // from $lib/npActions, shared with the page's pad-navigable transport overlay; the
  // launch list itself lives in the page (it's shared with launch/exit tracking).
  import { cardActions, type NowCard } from "$lib/npActions";

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
</script>

{#if cards.length}
  <div class="nowstack">
    {#each cards as c (c.id)}
      {@const actions = cardActions(c, { inSession, onerror, ondismiss })}
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
            {#each actions.filter((a) => a.kind === "media") as act (act.aria)}
              <button class="np-c" title={act.title} aria-label={act.aria} onclick={act.run}>{act.icon}</button>
            {/each}
          </span>
        {/if}
        {#each actions.filter((a) => a.kind === "app") as act (act.aria)}
          <button class="np-c" title={act.title} aria-label={act.aria} onclick={act.run}>{act.icon}</button>
        {/each}
        {#each actions.filter((a) => a.kind === "dismiss") as act (act.aria)}
          <button class="np-x" title={act.title} aria-label={act.aria} onclick={act.run}>{act.icon}</button>
        {/each}
      </div>
    {/each}
  </div>
{/if}

<style>
  .nowstack { position: fixed; z-index: 12; right: 2.4vw; bottom: 8vh; display: flex; flex-direction: column; gap: 10px; align-items: flex-end; }
  .nowplaying { display: flex; align-items: center; gap: 16px; background: #0c1320e8; border: 1px solid color-mix(in srgb, var(--accent) 45%, transparent); border-radius: 16px; padding: 14px 20px; box-shadow: 0 20px 60px #000b; max-width: 42vw; }
  .np-spinner { width: 22px; height: 22px; border-radius: 50%; border: 3px solid var(--border); border-top-color: var(--accent); animation: np-spin 0.9s linear infinite; flex: 0 0 auto; }
  @keyframes np-spin { to { transform: rotate(360deg); } }
  .np-icon { font-size: 20px; color: var(--accent); flex: 0 0 auto; }
  .np-eq { display: flex; align-items: flex-end; gap: 2px; height: 20px; flex: 0 0 auto; }
  .np-eq i { width: 4px; background: var(--accent); border-radius: 2px; animation: np-eq 0.9s ease-in-out infinite; }
  .np-eq i:nth-child(1) { animation-delay: 0s; } .np-eq i:nth-child(2) { animation-delay: 0.3s; } .np-eq i:nth-child(3) { animation-delay: 0.6s; }
  @keyframes np-eq { 0%, 100% { height: 6px; } 50% { height: 18px; } }
  .np-label { font-size: clamp(13px, 1.4vw, 17px); color: var(--text-muted); line-height: 1.3; min-width: 0; }
  .np-label b { color: #fff; font-size: 1.1em; }
  .np-sub { color: var(--text-muted); }
  .np-x { background: var(--surface); border: 1px solid var(--border); color: var(--text-muted); border-radius: 8px; width: 30px; height: 30px; cursor: pointer; font-size: 14px; flex: 0 0 auto; }
  .np-x:hover { border-color: var(--accent); color: #fff; }
  .np-controls { display: flex; gap: 6px; flex: 0 0 auto; }
  .np-c { background: var(--surface); border: 1px solid var(--border); color: var(--text-soft); border-radius: 8px; width: 32px; height: 32px; cursor: pointer; font-size: 14px; }
  .np-c:hover { border-color: var(--accent); color: #fff; }
  .np-c:focus, .np-x:focus { outline: none; }
  .np-c:focus-visible, .np-x:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  @media (prefers-reduced-motion: reduce) {
    .np-spinner, .np-eq i { animation: none; }
  }
</style>
