<!-- Media library browser (Jellyfin): Continue Watching / Latest / libraries at the root,
     then drill into libraries → series → seasons → episodes; Enter on a playable row hands
     it to mpv. Purely presentational (the Wizard doctrine): the page owns the view stack,
     focus, and all input routing; pointer intents come back through the on* callbacks.
     Row styling comes from the shared modal vocabulary in Modal.svelte; the poster thumb
     is the one media-specific addition (2:3, served via omnideck://). -->
<script lang="ts">
  import Modal from "./Modal.svelte";

  export type MediaRow = {
    id: string;
    name: string;
    sub: string;
    group?: string;
    browse: boolean;
  };

  let {
    title,
    rows,
    focus,
    posters,
    loading,
    depth,
    onfocus,
    onactivate,
    onclose,
  }: {
    title: string;
    rows: MediaRow[];
    focus: number;
    posters: Record<string, string>;
    loading: boolean;
    depth: number;
    onfocus: (i: number) => void;
    onactivate: () => void;
    onclose: () => void;
  } = $props();
</script>

<Modal labelledby="dlg-media" backdropLabel="Close media library" closeLabel="Close media library" {onclose}>
  <h2 id="dlg-media">{title}</h2>
  {#if loading}
    <div class="cgroup">loading…</div>
  {:else if !rows.length}
    <div class="cgroup">nothing here</div>
  {/if}
  <div class="catlist">
    {#each rows as r, i (`${r.group ?? ""}:${r.id}`)}
      {#if r.group && rows[i - 1]?.group !== r.group}<div class="cgroup">{r.group}</div>{/if}
      <button type="button" class="crow" class:focused={i === focus} data-med={i}
        onmouseenter={() => onfocus(i)} onclick={() => { onfocus(i); onactivate(); }}>
        <span class="mposter">{#if posters[r.id]}<img src={posters[r.id]} alt="" loading="lazy" />{:else}{r.browse ? "📁" : "🎬"}{/if}</span>
        <span class="cname">{r.name}</span>
        <span class="ccat">{r.sub}</span>
      </button>
    {/each}
  </div>
  <p class="phint">{depth > 1 ? "Esc/◯ back" : "Esc/◯ close"} · ↑↓ select · Enter/✕ {rows[focus]?.browse ? "open" : "play"}</p>
</Modal>

<style>
  /* Poster thumb: 2:3 like real box art (the shared .cicon is square, built for icons). */
  .mposter { width: 44px; height: 66px; border-radius: 7px; flex: 0 0 auto; overflow: hidden; display: grid; place-items: center; font-size: 20px; background: #22304a; }
  .mposter img { width: 100%; height: 100%; object-fit: cover; }
</style>
