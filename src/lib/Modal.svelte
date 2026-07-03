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
  /* Shared modal-content vocabulary (same :global-under-.prefs idiom as h2 above): list
     rows, group headers, the search/status line, hints, and the OSK grid. Lives here so
     every dialog — extracted (SearchModal, CatalogModal) or still in the page (power,
     info) — draws from one set of rules instead of per-component copies. */
  .prefs :global(.catlist) { max-height: 60vh; overflow-y: auto; display: flex; flex-direction: column; gap: 2px; margin: 4px 0; }
  .prefs :global(.crow) { display: flex; align-items: center; gap: 14px; padding: 9px 12px; border-radius: 10px; border: 2px solid transparent; cursor: pointer; background: none; color: inherit; font: inherit; width: 100%; text-align: left; }
  .prefs :global(.crow.focused) { background: #1b2540; border-color: var(--accent); }
  .prefs :global(.cicon) { width: 38px; height: 38px; border-radius: 9px; display: grid; place-items: center; font-size: 20px; flex: 0 0 auto; }
  .prefs :global(.cicon img.appicon) { width: 70%; height: 70%; object-fit: contain; }
  .prefs :global(.cname) { flex: 1; font-size: clamp(14px, 1.5vw, 18px); font-weight: 600; }
  .prefs :global(.ccat) { color: #6b7790; font-size: clamp(11px, 1.1vw, 13px); text-transform: uppercase; letter-spacing: 1px; }
  .prefs :global(.cstate) { color: #7e8aa0; font-weight: 700; font-size: clamp(12px, 1.3vw, 15px); min-width: 72px; text-align: right; }
  .prefs :global(.cstate.on) { color: #6ee7a8; }
  .prefs :global(.cgroup) { color: #6b7790; font-size: clamp(11px, 1.1vw, 13px); text-transform: uppercase; letter-spacing: 2px; font-weight: 700; padding: 12px 10px 4px; }
  .prefs :global(.cgroup:first-child) { padding-top: 2px; }
  .prefs :global(.chead) { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding-right: 44px; }
  .prefs :global(.sortbtn) { background: #1b2540; border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent); color: #cdd7e6; border-radius: 999px; padding: 4px 14px; cursor: pointer; font-size: clamp(11px, 1.1vw, 14px); font-weight: 700; }
  .prefs :global(.csearch) { color: #93a0b6; font-size: clamp(12px, 1.2vw, 15px); padding: 4px 2px 6px; }
  .prefs :global(.csearch.active) { color: var(--accent); font-weight: 700; }
  .prefs :global(.phint) { color: #7e8aa0; font-size: clamp(11px, 1.1vw, 13px); margin: 3px 0 0; }
  .prefs :global(.osk) { display: grid; grid-template-columns: repeat(6, 1fr); gap: 6px; margin: 8px 0 4px; }
  .prefs :global(.oskkey) { background: #0c1320; border: 2px solid #2c3a5c; color: #dde5f0; border-radius: 8px; padding: 10px 0; font-size: clamp(15px, 1.6vw, 20px); font-weight: 700; cursor: pointer; text-transform: uppercase; }
  .prefs :global(.oskkey.special) { color: var(--accent); background: #11192b; }
  .prefs :global(.oskkey.focused) { border-color: var(--accent); background: #1b2540; color: #fff; box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 60%, transparent); }
  .prefs :global(.oskkey:hover) { border-color: var(--accent); }
  /* Same focus policy as the page: rows/keys already show focus via .focused — no default
     ring, accent ring for keyboard users only. */
  .prefs :global(.crow:focus), .prefs :global(.oskkey:focus), .prefs :global(.sortbtn:focus) { outline: none; }
  .prefs :global(.crow:focus-visible), .prefs :global(.oskkey:focus-visible), .prefs :global(.sortbtn:focus-visible) { outline: 2px solid var(--accent); outline-offset: 2px; }
  .prefs-close { position: absolute; top: 14px; right: 14px; width: 34px; height: 34px; border-radius: 9px; background: #1b2540; border: 1px solid #2c3a5c; color: #9fb0c8; cursor: pointer; font-size: 15px; line-height: 1; }
  .prefs-close:hover { border-color: var(--accent); color: #fff; }
  /* Same focus policy as the page: no default ring (the accent border/hover shows state for
     pointer users), a clear accent ring for keyboard users only. */
  .prefs-close:focus { outline: none; }
  .prefs-close:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>
