<script lang="ts">
  // First-run wizard (3 steps: welcome → accent → controls). Purely presentational —
  // stepping/accent changes are driven by the page's unified keyboard+gamepad routing,
  // which must own every open overlay to gate base navigation correctly.
  import { dialogFocus } from "$lib/dialog";

  let {
    step,
    tier,
    gamesCount,
    catalogCount,
    accents,
    accent,
  }: {
    step: number;
    tier: string | null;
    gamesCount: number;
    catalogCount: number;
    accents: string[];
    accent: string;
  } = $props();
</script>

<div class="wizard" role="dialog" aria-modal="true" aria-label="OmniDeck setup" tabindex="-1" use:dialogFocus>
  {#if step === 0}
    <div class="wstep">
      <div class="wlogo">OMNIDECK</div><h2>Welcome 👋</h2>
      <p class="wlead">Your living-room launcher for games, streaming, music, and your own media.</p>
      <ul class="wfacts"><li>Mode: <b>{tier ?? "…"}</b></li><li>Games: <b>{gamesCount}</b></li><li>Apps to add: <b>{catalogCount}</b></li></ul>
      <div class="wnav">Press <b>Enter / ✕</b> to continue</div>
    </div>
  {:else if step === 1}
    <div class="wstep">
      <h2>Pick a theme</h2><p class="wlead">Choose an accent — change it anytime in Settings.</p>
      <div class="wswatches">{#each accents as a}<span class="wsw" class:sel={accent === a} style="background:{a}"></span>{/each}</div>
      <div class="wnav"><b>◀ ▶</b> change · <b>Enter / ✕</b> next · <b>Esc / ◯</b> back</div>
    </div>
  {:else}
    <div class="wstep">
      <h2>You're set! 🎮</h2>
      <ul class="wfacts"><li><b>← →</b> category · <b>↑ ↓</b> items</li><li><b>Enter / ✕</b> launch · <b>□ / F</b> favorite</li><li><b>△ / A</b> add apps · <b>Start / H</b> home · <b>P</b> settings · <b>/ Select</b> search · <b>i / R1</b> info</li></ul>
      <div class="wnav">Press <b>Enter / ✕</b> to start</div>
    </div>
  {/if}
</div>

<style>
  .wizard { position: fixed; inset: 0; z-index: 20; display: grid; place-items: center; background: radial-gradient(1200px 800px at 50% 30%, #1a2236 0%, #05070b 70%); }
  .wstep { width: min(640px, 90vw); text-align: center; display: flex; flex-direction: column; align-items: center; gap: 16px; padding: 32px; }
  .wlogo { font-size: clamp(26px, 3.4vw, 48px); font-weight: 800; letter-spacing: 4px; color: var(--accent); }
  .wstep h2 { margin: 0; font-size: clamp(26px, 3.4vw, 44px); }
  .wlead { margin: 0; color: #aab6c9; font-size: clamp(15px, 1.7vw, 21px); max-width: 34em; line-height: 1.5; }
  .wfacts { list-style: none; padding: 0; margin: 6px 0; display: flex; flex-direction: column; gap: 8px; color: #cdd7e6; font-size: clamp(14px, 1.6vw, 20px); }
  .wfacts b { color: #fff; }
  .wswatches { display: flex; gap: 16px; margin: 8px 0; }
  .wsw { width: 56px; height: 56px; border-radius: 14px; border: 3px solid transparent; box-shadow: 0 6px 20px #0008; }
  .wsw.sel { border-color: #fff; transform: scale(1.12); }
  .wnav { margin-top: 14px; color: #7e8aa0; font-size: clamp(13px, 1.4vw, 17px); }
  .wnav b { color: var(--accent); }
</style>
