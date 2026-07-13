<!--
  Poster grid for the library (appearance.layout = "grid" | "grid-compact").

  Dumb renderer: the page owns `focus` and all input routing (D-pad/stick/keyboard land in
  the page's moveItem/horiz, which use the 2D math in ./layouts.ts); this component only
  paints tiles and keeps the focused one scrolled into view. Focused-tile treatment mirrors
  the rail (accent ring + scale + full opacity). Tiles use content-visibility so a large
  library doesn't pay layout/paint for offscreen rows; art is windowed by the page.
-->
<script lang="ts">
  import type { Tile } from "$lib/tiles";

  let {
    items,
    focus,
    cols,
    compact = false,
    art,
    appIcons,
    iconBg,
    favorites,
    onactivate,
    onarterror,
  }: {
    items: Tile[];
    focus: number;
    cols: number;
    compact?: boolean;
    art: Record<string, string>;
    appIcons: Record<string, string>;
    iconBg: Record<string, string>;
    favorites: string[];
    onactivate: (i: number) => void;
    onarterror: (appid: string) => void;
  } = $props();

  let root = $state<HTMLElement | null>(null);
  function tileName(t: Tile) {
    return t.kind === "app" ? t.app.name : t.game.name;
  }
  // Keep the focused tile visible ("nearest" = no jumpy centering; instant, so
  // reduced-motion users get no scroll animation either).
  $effect(() => {
    root?.querySelector(`[data-gi="${focus}"]`)?.scrollIntoView({ block: "nearest" });
  });
</script>

<div class="gwrap" class:compact bind:this={root} style="--gcols:{cols}">
  <div class="ggrid">
    {#each items as t, i (t.id)}
      {@const hasArt = t.kind === "game" ? !!art[t.game.appid] : !!appIcons[t.app.id]}
      <button
        class="gtile"
        class:focused={i === focus}
        data-gi={i}
        onclick={() => onactivate(i)}
        aria-label={tileName(t)}
      >
        {#if t.kind === "game" && art[t.game.appid]}
          <img src={art[t.game.appid]} alt="" decoding="async" loading="lazy" onerror={() => onarterror(t.game.appid)} />
        {:else if t.kind === "app" && appIcons[t.app.id]}
          <span class="gicon" style="background:{iconBg[t.app.id] ?? '#f4f5f8'}">
            <img class="appicon" src={appIcons[t.app.id]} alt="" decoding="async" loading="lazy" />
          </span>
        {:else}
          <span class="gfall" style={t.kind === "app" && t.app.accent ? `background:${t.app.accent}` : ""}>
            <span class="gemoji">{t.kind === "app" ? t.app.icon : "🎮"}</span>
            <span class="gfname">{tileName(t)}</span>
          </span>
        {/if}
        {#if hasArt}
          <span class="gname">{tileName(t)}{#if favorites.includes(t.id)} ⭐{/if}</span>
        {:else if favorites.includes(t.id)}
          <span class="gfav">⭐</span>
        {/if}
      </button>
    {/each}
  </div>
</div>

<style>
  .gwrap {
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-width: none; /* couch UI: no scrollbar chrome */
    /* breathing room so the focused tile's scale + ring never clip on the edges */
    padding: 10px 14px 10px 6px;
    box-sizing: border-box;
  }
  .gwrap::-webkit-scrollbar { display: none; }
  .ggrid {
    display: grid;
    grid-template-columns: repeat(var(--gcols), 1fr);
    gap: calc(0.9rem * var(--scale, 1));
    padding: 6px;
  }
  .compact .ggrid { gap: calc(0.55rem * var(--scale, 1)); }
  .gtile {
    position: relative;
    aspect-ratio: 2 / 3; /* Steam box-art portrait */
    border: 0;
    padding: 0;
    background: #1a2233;
    border-radius: 12px;
    overflow: hidden;
    cursor: pointer;
    opacity: 0.82;
    box-shadow: 0 4px 14px #0007;
    transition: opacity 0.12s, transform 0.12s, box-shadow 0.12s;
    /* skip layout/paint for offscreen rows in big libraries */
    content-visibility: auto;
    contain-intrinsic-size: auto 240px;
  }
  .compact .gtile { border-radius: 9px; }
  .gtile.focused {
    opacity: 1;
    transform: scale(1.06);
    box-shadow: 0 0 0 2px var(--accent), 0 8px 24px #000a; /* same ring as the rail's focused thumb */
    z-index: 1;
  }
  .gtile:hover { opacity: 1; }
  .gtile > img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; }
  .gicon { position: absolute; inset: 0; display: grid; place-items: center; }
  .gicon img.appicon { width: 58%; height: 58%; object-fit: contain; }
  .gfall {
    position: absolute; inset: 0;
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: 8px; padding: 8px; box-sizing: border-box; background: #1a2233;
  }
  .gemoji { font-size: calc(1.8rem * var(--scale, 1)); }
  .compact .gemoji { font-size: calc(1.3rem * var(--scale, 1)); }
  .gfname {
    color: #c2cbdb; font-weight: 600; text-align: center;
    font-size: clamp(11px, 1vw, 15px); line-height: 1.25;
    overflow: hidden; display: -webkit-box; -webkit-line-clamp: 3; line-clamp: 3; -webkit-box-orient: vertical;
    word-break: break-word;
  }
  .compact .gfname { font-size: clamp(9px, 0.8vw, 12px); -webkit-line-clamp: 2; line-clamp: 2; }
  /* name strip over art: visible on focus/hover only, so the grid stays clean */
  .gname {
    position: absolute; left: 0; right: 0; bottom: 0;
    padding: 18px 8px 7px;
    background: linear-gradient(transparent, #000d);
    color: #fff; font-weight: 700; text-align: left;
    font-size: clamp(12px, 1.1vw, 16px);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
    opacity: 0; transition: opacity 0.12s;
  }
  .compact .gname { font-size: clamp(10px, 0.9vw, 13px); padding: 12px 6px 5px; }
  .gtile.focused .gname, .gtile:hover .gname { opacity: 1; }
  .gfav { position: absolute; top: 5px; right: 6px; font-size: 0.85em; filter: drop-shadow(0 1px 2px #000c); }
  .gtile:focus { outline: none; }
  .gtile:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  @media (prefers-reduced-motion: reduce) {
    .gtile { transition: none; }
  }
</style>
