<!-- Shared modal shell: backdrop (click = close), centered dialog with a11y semantics
     (role=dialog, aria-modal, focus moved in on open / restored on close), and the ✕ close
     button. Content comes in as a snippet and renders in the caller's style scope; give the
     dialog's <h2> the id you pass as `labelledby`. The confirm dialog hides the ✕
     (showClose={false}) — its two explicit buttons are the whole point. -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { dialogFocus } from "./dialog";

  let {
    labelledby,
    backdropLabel,
    closeLabel = "Close (Esc)",
    showClose = true,
    onclose,
    children,
  }: {
    labelledby: string;
    backdropLabel: string;
    closeLabel?: string;
    showClose?: boolean;
    onclose: () => void;
    children: Snippet;
  } = $props();
</script>

<button class="prefs-backdrop" aria-label={backdropLabel} onclick={onclose}></button>
<div class="prefs" role="dialog" aria-modal="true" aria-labelledby={labelledby} tabindex="-1" use:dialogFocus>
  {#if showClose}
    <button class="prefs-close" title={closeLabel} aria-label={closeLabel} onclick={onclose}>✕</button>
  {/if}
  {@render children()}
</div>

<style>
  .prefs-backdrop { position: fixed; inset: 0; background: rgba(4,6,10,.6); border: 0; padding: 0; cursor: pointer; z-index: 10; }
  .prefs { position: fixed; z-index: 11; top: 50%; left: 50%; transform: translate(-50%, -50%); width: min(620px, 92vw); background: #121826; border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent); border-radius: 18px; padding: 22px 26px; box-shadow: 0 30px 80px #000c; display: flex; flex-direction: column; gap: 4px; }
  .prefs :global(h2) { margin: 0 0 10px; font-size: clamp(20px, 2.2vw, 26px); }
  .prefs-close { position: absolute; top: 14px; right: 14px; width: 34px; height: 34px; border-radius: 9px; background: #1b2540; border: 1px solid #2c3a5c; color: #9fb0c8; cursor: pointer; font-size: 15px; line-height: 1; }
  .prefs-close:hover { border-color: var(--accent); color: #fff; }
  /* Same focus policy as the page: no default ring (the accent border/hover shows state for
     pointer users), a clear accent ring for keyboard users only. */
  .prefs-close:focus { outline: none; }
  .prefs-close:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>
