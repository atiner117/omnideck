<!-- Global search dialog: your games + apps with a web-search fallback row, plus the
     on-screen keyboard for controller text entry. Purely presentational (the Wizard
     pattern): the page's unified keyboard+gamepad routing owns the open/focus/query state
     and all input; pointer interactions report back through the on* callbacks. Row/OSK
     styling comes from the shared modal vocabulary in Modal.svelte. -->
<script lang="ts">
  import Modal from "./Modal.svelte";
  import { OSK_FLAT } from "./osk";
  import type { Tile } from "./tiles";

  let {
    query,
    focus,
    results,
    oskFocus,
    oskDim,
    appIcons,
    iconBg,
    engineIcon,
    onfocus,
    onactivate,
    onwebsearch,
    onoskfocus,
    onoskpress,
    onclose,
  }: {
    query: string;
    focus: number;
    results: Tile[];
    oskFocus: number;
    oskDim: boolean;
    appIcons: Record<string, string>;
    iconBg: Record<string, string>;
    engineIcon: string;
    onfocus: (i: number) => void;
    onactivate: () => void;
    onwebsearch: () => void;
    onoskfocus: (i: number) => void;
    onoskpress: (key: string) => void;
    onclose: () => void;
  } = $props();
</script>

<Modal labelledby="dlg-search" backdropLabel="Close search" closeLabel="Close search" {onclose}>
  <h2 id="dlg-search">Search</h2>
  <div class="csearch active">{query ? `🔎 ${query}` : "Type to search your games, apps & the web…"}</div>
  <div class="catlist">
    {#if query && !results.length}<div class="cgroup">no library matches — ⏎ searches the web</div>{/if}
    {#each results as t, i (t.id)}
      <button type="button" class="crow" class:focused={i === focus} data-sr={i} onmouseenter={() => onfocus(i)} onclick={() => { onfocus(i); onactivate(); }}>
        <span class="cicon" style="background:{t.kind === 'app' && appIcons[t.app.id] ? (iconBg[t.app.id] ?? '#f4f5f8') : t.kind === 'app' ? t.app.accent : '#22304a'}">{#if t.kind === "app" && appIcons[t.app.id]}<img class="appicon" src={appIcons[t.app.id]} alt="" />{:else}{t.kind === "app" ? t.app.icon : "🎮"}{/if}</span>
        <span class="cname">{t.kind === "app" ? t.app.name : t.game.name}</span>
        <span class="ccat">{t.cat}</span>
      </button>
    {/each}
    <button type="button" class="crow" class:focused={focus === results.length} data-sr={results.length} onmouseenter={() => onfocus(results.length)} onclick={() => onwebsearch()}>
      <span class="cicon" style="background:#3a3f4a">{#if engineIcon}<img class="appicon" src={engineIcon} alt="" />{:else}🌐{/if}</span>
      <span class="cname">Search the web{query ? ` for “${query}”` : "…"}</span>
    </button>
  </div>
  <!-- The OSK is controller furniture: it recedes while a physical keyboard is doing the
       typing and comes back the moment the D-pad touches it (the page tracks the source). -->
  <div class="osk" class:dim={oskDim} role="group" aria-label="On-screen keyboard">
    {#each OSK_FLAT as k, i}
      <button class="oskkey" class:focused={i === oskFocus} class:special={"␣⌫✕⏎".includes(k)}
        onmouseenter={() => onoskfocus(i)} onclick={() => { onoskfocus(i); onoskpress(k); }}>{k}</button>
    {/each}
  </div>
  <p class="phint">keyboard: type · ↑↓ select · Enter open — controller: D-pad + ✕ to type · bumpers pick result · ⏎ go · ◯ clear/close</p>
</Modal>
