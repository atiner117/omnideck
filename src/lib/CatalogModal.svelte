<!-- "Add apps & media" catalog browser: grouped/A–Z sorted rows with add/remove state.
     Purely presentational (the Wizard pattern): the page's unified keyboard+gamepad
     routing owns the open/focus/query/sort state and all input; pointer interactions
     report back through the on* callbacks. Row styling comes from the shared modal
     vocabulary in Modal.svelte. -->
<script lang="ts">
  import Modal from "./Modal.svelte";
  import type { App } from "./backend";

  let {
    entries,
    focus,
    query,
    sort,
    appIcons,
    iconBg,
    isAdded,
    onfocus,
    ontoggle,
    onsortswap,
    onclose,
  }: {
    entries: App[];
    focus: number;
    query: string;
    sort: "group" | "alpha";
    appIcons: Record<string, string>;
    iconBg: Record<string, string>;
    isAdded: (id: string) => boolean;
    onfocus: (i: number) => void;
    ontoggle: (i: number) => void;
    onsortswap: () => void;
    onclose: () => void;
  } = $props();
</script>

<Modal labelledby="dlg-catalog" backdropLabel="Close add apps" closeLabel="Close add apps" {onclose}>
  <div class="chead">
    <h2 id="dlg-catalog">Add apps &amp; media</h2>
    <button class="sortbtn" onclick={onsortswap}>{sort === "group" ? "Grouped" : "A–Z"}</button>
  </div>
  <div class="csearch" class:active={query}>{query ? `🔎 ${query}` : "Type to search…  ·  Tab: sort"}</div>
  <div class="catlist">
    {#each entries as c, i (c.id)}
      {#if sort === "group" && (i === 0 || entries[i - 1].category !== c.category)}<div class="cgroup">{c.category ?? "apps"}</div>{/if}
      <button type="button" class="crow" class:focused={i === focus} data-cat={i} onmouseenter={() => onfocus(i)} onclick={() => { onfocus(i); ontoggle(i); }}>
        <span class="cicon" style="background:{appIcons[c.id] ? (iconBg[c.id] ?? '#f4f5f8') : c.accent}">{#if appIcons[c.id]}<img class="appicon" src={appIcons[c.id]} alt="" />{:else}{c.icon}{/if}</span>
        <span class="cname">{c.name}</span>
        <span class="cstate" class:on={isAdded(c.id)}>{isAdded(c.id) ? "✓ Added" : "+ Add"}</span>
      </button>
    {/each}
    {#if !entries.length}<div class="cgroup">no matches for “{query}”</div>{/if}
  </div>
  <p class="phint">type to search · Tab sort · ↑↓ select · Enter/✕ toggle · Esc clear/close</p>
</Modal>
