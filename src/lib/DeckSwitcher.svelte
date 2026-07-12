<!-- Deck switcher: iOS-style row of running-app cards. Purely presentational (the
     SearchModal/Wizard pattern): the page's unified keyboard+gamepad routing owns the
     open/focus state and all input; pointer interactions report back through the on*
     callbacks. Guide/B dismisses, A opens the focused card, Select closes it. -->
<script lang="ts">
  import type { LiveApp } from "./backend";

  let {
    apps,
    focus,
    iconFor,
    onfocus,
    onselect,
    onkill,
    onclose,
  }: {
    apps: LiveApp[];
    focus: number;
    iconFor: (a: LiveApp) => string;
    onfocus: (i: number) => void;
    onselect: () => void;
    onkill: () => void;
    onclose: () => void;
  } = $props();

  // Focus the selected card so a real-session keyboard reaches the deck (and for a11y) and it
  // scrolls into view. The gamepad path doesn't need this (events arrive via gilrs regardless).
  $effect(() => {
    const i = focus;
    queueMicrotask(() => (document.querySelector(`[data-deck="${i}"]`) as HTMLElement | null)?.focus());
  });
</script>

<div class="deck-scrim" role="button" tabindex="-1" aria-label="Close app switcher"
     onclick={onclose} onkeydown={(e) => { if (e.key === "Escape") onclose(); }}></div>
<section class="deck" aria-label="App switcher">
  <div class="deck-row">
    {#each apps as a, i (a.group)}
      <div class="deck-card" class:sel={i === focus}>
        <button class="deck-open" title="Open {a.name}" data-deck={i}
          onclick={() => { onfocus(i); onselect(); }} onmouseenter={() => onfocus(i)}>
          <span class="deck-icon">{iconFor(a)}</span>
          <span class="deck-name">{a.name}</span>
        </button>
        <button class="deck-x" title="Close {a.name}" aria-label="Close {a.name}"
          onclick={(e) => { e.stopPropagation(); onfocus(i); onkill(); }}>✕</button>
      </div>
    {/each}
  </div>
  <p class="deck-hint">A open · Select ✕ close · B back</p>
</section>

<style>
  .deck-scrim { position: fixed; inset: 0; z-index: 40; background: rgba(3,5,11,0.72); border: 0; }
  .deck { position: fixed; inset: 0; z-index: 41; display: flex; flex-direction: column;
    align-items: center; justify-content: center; gap: 24px; pointer-events: none; }
  .deck-row { display: flex; gap: 22px; padding: 0 6vw; max-width: 100vw; overflow-x: auto;
    align-items: center; pointer-events: auto; scrollbar-width: none; }
  .deck-row::-webkit-scrollbar { display: none; }
  .deck-card { position: relative; flex: 0 0 auto; }
  .deck-open { display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: 14px; width: calc(240px * var(--scale)); height: calc(150px * var(--scale));
    border-radius: 18px; border: 2px solid rgba(255,255,255,0.08);
    background: linear-gradient(160deg, #141a26, #0c1119); color: #e7ecf6; cursor: pointer;
    transition: transform .16s cubic-bezier(.2,.7,.2,1), border-color .16s, box-shadow .16s; }
  .deck-card.sel .deck-open { transform: translateY(-14px) scale(1.06); border-color: var(--accent);
    box-shadow: 0 18px 50px color-mix(in srgb, var(--accent) 45%, transparent); }
  .deck-icon { font-size: calc(46px * var(--scale)); line-height: 1; }
  .deck-name { font-size: calc(17px * var(--scale)); font-weight: 600; max-width: 90%;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .deck-x { position: absolute; top: -12px; right: -12px; width: 34px; height: 34px; border-radius: 50%;
    border: 2px solid rgba(255,255,255,0.14); background: #05070b; color: #c2cbdb; cursor: pointer;
    font-size: 15px; opacity: 0; transition: opacity .16s; }
  .deck-card.sel .deck-x { opacity: 1; }
  .deck-hint { pointer-events: none; color: #8a94a6; font-size: calc(14px * var(--scale));
    letter-spacing: .02em; }

  /* Respect reduced-motion (the page-level rule can't reach into this scoped component). */
  @media (prefers-reduced-motion: reduce) {
    .deck-open { transition: none !important; }
  }
</style>
