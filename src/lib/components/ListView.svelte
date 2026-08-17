<!--
  Detailed list view for the library (appearance.layout = "list").

  Dumb renderer, same contract as GridView: the page owns `focus` and input routing
  (vertical nav is the rail's wrap, left/right stays the category axis); this component
  paints rows — thumb, name, a details subline (type/source) and last-played for games —
  and keeps the focused row scrolled into view.
-->
<script lang="ts">
  import type { Tile } from "$lib/tiles";
  import type { App } from "$lib/backend";

  let {
    items,
    focus,
    art,
    appIcons,
    iconBg,
    favorites,
    onactivate,
    onarterror,
  }: {
    items: Tile[];
    focus: number;
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
  function fmtPlayed(ts?: number): string {
    if (!ts) return "never played";
    return new Date(ts * 1000).toLocaleDateString([], { year: "numeric", month: "short", day: "numeric" });
  }
  // Compact "where this comes from" line (mirrors the info sheet's appSource).
  function appDetail(a: App): string {
    const e = a.exec;
    if (a.id === "media-library") return "Browse your media server";
    if (e[0] === "flatpak") return `Flatpak · ${e[2] ?? ""}`;
    if (e[0] === "BROWSER") {
      const u = e.find((x) => x.startsWith("--app=") || x.startsWith("http://") || x.startsWith("https://"));
      return "Web app · " + (u ? u.replace(/^--app=/, "").replace(/^https?:\/\//, "").split("/")[0] : "browser");
    }
    return "Command · " + e.join(" ");
  }
  function detail(t: Tile): string {
    return t.kind === "game"
      ? `Steam game · ${t.game.installed ? "installed" : "not installed"}`
      : appDetail(t.app);
  }
  $effect(() => {
    root?.querySelector(`[data-li="${focus}"]`)?.scrollIntoView({ block: "nearest" });
  });
</script>

<div class="lwrap" bind:this={root}>
  {#each items as t, i (t.id)}
    <button class="lrow" class:focused={i === focus} data-li={i} onclick={() => onactivate(i)}>
      <span class="lthumb" style={t.kind === "app" && appIcons[t.app.id] ? `background:${iconBg[t.app.id] ?? "#f4f5f8"}` : ""}>
        {#if t.kind === "game" && art[t.game.appid]}
          <img src={art[t.game.appid]} alt="" decoding="async" loading="lazy" onerror={() => onarterror(t.game.appid)} />
        {:else if t.kind === "app" && appIcons[t.app.id]}
          <img class="appicon" src={appIcons[t.app.id]} alt="" decoding="async" loading="lazy" />
        {:else}
          <span class="lemoji">{t.kind === "app" ? t.app.icon : "🎮"}</span>
        {/if}
      </span>
      <span class="lmain">
        <span class="lname">{tileName(t)}{#if favorites.includes(t.id)}<span class="lfav">⭐</span>{/if}</span>
        <span class="ldetail">{detail(t)}</span>
      </span>
      {#if t.kind === "game"}
        <span class="lwhen">{fmtPlayed(t.game.last_played)}</span>
      {/if}
    </button>
  {/each}
</div>

<style>
  .lwrap {
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-width: none;
    padding: 6px 14px 6px 2px;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .lwrap::-webkit-scrollbar { display: none; }
  .lrow {
    display: flex; align-items: center; gap: calc(0.9rem * var(--scale, 1));
    flex: 0 0 auto;
    border: 0; background: none; text-align: left; cursor: pointer;
    padding: calc(0.35rem * var(--scale, 1)) 12px;
    border-radius: 12px;
    /* Design tokens with the original dark palette as fallback: identical standalone,
       theme-following once tokens.css (#41) defines them (Light-theme readability). */
    color: var(--text, #c2cbdb); opacity: 0.72;
    transition: opacity 0.12s, background 0.12s, transform 0.12s;
    content-visibility: auto;
    contain-intrinsic-size: auto 64px;
  }
  .lrow.focused {
    opacity: 1; color: var(--text-bright, #eef2f8);
    /* text-tinted wash ≈ the old #ffffff10 on dark, stays visible on light surfaces */
    background: color-mix(in srgb, var(--text, #ffffff) 6%, transparent);
    box-shadow: inset 0 0 0 2px var(--accent); /* the rail's accent ring, row-shaped */
    transform: translateX(6px);
  }
  .lrow:hover { opacity: 1; }
  .lthumb {
    width: calc(2.6rem * var(--scale, 1)); height: calc(2.6rem * var(--scale, 1));
    border-radius: 9px; flex: 0 0 auto; overflow: hidden;
    display: grid; place-items: center;
    background: var(--surface-3, #1a2233); box-shadow: 0 4px 14px #0007;
  }
  .lthumb img { width: 100%; height: 100%; object-fit: cover; }
  .lthumb img.appicon { object-fit: contain; padding: 16%; box-sizing: border-box; }
  .lemoji { font-size: calc(1.2rem * var(--scale, 1)); }
  .lmain { display: flex; flex-direction: column; gap: 2px; min-width: 0; flex: 1 1 auto; }
  .lname {
    font-size: calc(clamp(14px, 1.4vw, 20px) * var(--scale, 1)); font-weight: 600;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .lrow.focused .lname { font-weight: 800; }
  .lfav { font-size: 0.8em; margin-left: 8px; }
  .ldetail {
    color: var(--text-muted, #8a96ab); font-size: calc(clamp(11px, 1vw, 14px) * var(--scale, 1));
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .lwhen {
    flex: 0 0 auto; color: var(--text-muted, #8a96ab); font-variant-numeric: tabular-nums;
    font-size: calc(clamp(11px, 1vw, 14px) * var(--scale, 1));
  }
  .lrow:focus { outline: none; }
  .lrow:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  @media (prefers-reduced-motion: reduce) {
    .lrow { transition: none; }
  }
</style>
