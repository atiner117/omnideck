<!-- Audio output switcher (roadmap #3, frontend): pick the default sink — headset,
     soundbar, HDMI — without leaving the launcher. Purely presentational (the
     CatalogModal/Wizard pattern): the page's unified keyboard+gamepad routing owns
     open/focus state and calls the audio_outputs/audio_set_output commands; pointer
     interactions report back through the on* callbacks. Row styling comes from the
     shared modal vocabulary in Modal.svelte. -->
<script lang="ts" module>
  /** Mirror of the backend's AudioSink (bindings/AudioSink.ts once the backend PR lands —
   *  kept local so the two branches merge in either order). */
  export type AudioSink = { name: string; description: string; is_default: boolean };
</script>

<script lang="ts">
  import Modal from "./Modal.svelte";

  let {
    sinks,
    focus,
    busy = false,
    error = "",
    onfocus,
    onselect,
    onrefresh,
    onclose,
  }: {
    sinks: AudioSink[];
    /** Focused row index (page-owned, same convention as CatalogModal). */
    focus: number;
    /** True while a set-default call is in flight — rows dim, input should be ignored. */
    busy?: boolean;
    /** Enumeration/set failure to surface inline (empty = none). */
    error?: string;
    onfocus: (i: number) => void;
    onselect: (i: number) => void;
    onrefresh: () => void;
    onclose: () => void;
  } = $props();
</script>

<Modal labelledby="dlg-audio" backdropLabel="Close audio output" closeLabel="Close audio output" {onclose}>
  <div class="chead">
    <h2 id="dlg-audio">Audio output</h2>
    <button class="sortbtn" onclick={onrefresh} disabled={busy}>↻ Refresh</button>
  </div>
  {#if error}
    <div class="aerr" role="alert">{error}</div>
  {/if}
  <div class="catlist" class:busy>
    {#each sinks as s, i (s.name)}
      <button
        type="button"
        class="crow"
        class:focused={i === focus}
        disabled={busy}
        onmouseenter={() => onfocus(i)}
        onclick={() => {
          onfocus(i);
          onselect(i);
        }}
      >
        <span class="cicon" style="background:#1b2540">{s.is_default ? "🔊" : "🔈"}</span>
        <span class="cname">
          {s.description || s.name}
          {#if s.description && s.description !== s.name}<span class="asub">{s.name}</span>{/if}
        </span>
        <span class="cstate" class:on={s.is_default}>{s.is_default ? "✓ Current" : "Select"}</span>
      </button>
    {/each}
    {#if !sinks.length && !error}
      <div class="cgroup">no audio outputs found (is PipeWire/PulseAudio running?)</div>
    {/if}
  </div>
  <p class="phint">↑↓ select · Enter set output · ↻ refresh · Esc close</p>
</Modal>

<style>
  .catlist.busy {
    opacity: 0.55;
    pointer-events: none;
  }
  .asub {
    display: block;
    color: #6b7790;
    font-size: clamp(10px, 1vw, 12px);
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 46ch;
  }
  .aerr {
    color: #ff9d9d;
    background: #3a161a66;
    border: 1px solid #ff9d9d55;
    border-radius: 10px;
    padding: 8px 12px;
    font-size: clamp(12px, 1.2vw, 15px);
  }
</style>
