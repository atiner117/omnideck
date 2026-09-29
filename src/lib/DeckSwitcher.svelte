<!-- Controller task overview: Home is always first; app cards preserve the backend's MRU order.
     The page owns keyboard/gamepad routing and all IPC. -->
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
    onselect: (i: number) => void;
    onkill: (i: number) => void;
    onclose: () => void;
  } = $props();

  // Focus the highlighted action for keyboard/a11y, even after the last app disappears.
  $effect(() => {
    const i = focus;
    const count = apps.length;
    queueMicrotask(() => {
      if (i > count) return;
      const active = document.activeElement as HTMLElement | null;
      // Tabbing to Close updates the selected card but must not steal keyboard focus back
      // to Open; gamepad navigation still focuses the newly highlighted Open action.
      if (active?.dataset.deckClose !== String(i)) {
        const card = document.querySelector(`[data-deck="${i}"]`) as HTMLElement | null;
        card?.focus();
      }
      const card = document.querySelector(`[data-deck="${i}"]`) as HTMLElement | null;
      card?.scrollIntoView({ block: "nearest", inline: "nearest" });
    });
  });
</script>

<div class="deck-scrim" role="button" tabindex="-1" aria-label="Dismiss task overview"
     onclick={onclose} onkeydown={(e) => { if (e.key === "Escape") onclose(); }}></div>
<section class="deck" aria-label="Task overview">
  <h2>Recent apps</h2>
  <div class="deck-row">
    <div class="deck-card" class:sel={focus === 0}>
      <button class="deck-open" data-deck={0} aria-label="Go Home" aria-current={focus === 0 ? "true" : undefined}
        onclick={() => onselect(0)} onmouseenter={() => onfocus(0)}>
        <span class="deck-icon" aria-hidden="true">⌂</span>
        <span class="deck-name">Home</span>
      </button>
    </div>
    {#each apps as a, i (a.group)}
      <div class="deck-card" class:sel={i + 1 === focus}>
        <button class="deck-open" data-deck={i + 1} aria-label="Open {a.name}" aria-current={i + 1 === focus ? "true" : undefined}
          onclick={() => onselect(i + 1)} onmouseenter={() => onfocus(i + 1)}>
          <span class="deck-icon" aria-hidden="true">{iconFor(a)}</span>
          <span class="deck-name">{a.name}</span>
        </button>
        <button class="deck-x" title="Close {a.name}" aria-label="Close {a.name}" data-deck-close={i + 1}
          onfocus={() => onfocus(i + 1)}
          onclick={(e) => { e.stopPropagation(); onkill(i + 1); }}>✕ <span class="close-label">Close</span></button>
      </div>
    {/each}
  </div>
  {#if apps.length === 0}
    <p class="deck-empty">No running apps. Choose Home or press B to go back.</p>
  {/if}
  <p class="deck-hint">← → choose · A / Enter open · Select / Delete close app · B / Esc back</p>
</section>

<style>
  .deck-scrim { position: fixed; inset: 0; z-index: 40; background: rgba(3,5,11,0.78); border: 0; }
  .deck { position: fixed; inset: 0; z-index: 41; display: flex; flex-direction: column;
    align-items: center; justify-content: center; gap: 22px; pointer-events: none; color: #e7ecf6; }
  h2 { margin: 0; font-size: calc(25px * var(--scale)); font-weight: 650; }
  .deck-row { display: flex; gap: 22px; padding: 22px 6vw; max-width: 100vw; overflow-x: auto;
    align-items: center; pointer-events: auto; scrollbar-width: thin; }
  .deck-card { position: relative; flex: 0 0 auto; }
  .deck-open { display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: 14px; width: calc(240px * var(--scale)); height: calc(150px * var(--scale));
    border-radius: 18px; border: 2px solid rgba(255,255,255,0.25);
    background: linear-gradient(160deg, #141a26, #0c1119); color: #e7ecf6; cursor: pointer;
    transition: transform .16s cubic-bezier(.2,.7,.2,1), border-color .16s, box-shadow .16s; }
  .deck-card.sel .deck-open, .deck-open:focus-visible { transform: translateY(-8px) scale(1.03); border-color: var(--accent);
    box-shadow: 0 12px 36px color-mix(in srgb, var(--accent) 40%, transparent); outline: 2px solid var(--accent); outline-offset: 3px; }
  .deck-icon { font-size: calc(46px * var(--scale)); line-height: 1; }
  .deck-name { font-size: calc(17px * var(--scale)); font-weight: 600; max-width: 90%;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .deck-x { position: absolute; top: -14px; right: -12px; min-width: 88px; min-height: 44px; padding: 0 10px;
    border-radius: 22px; border: 2px solid rgba(255,255,255,0.5); background: #141a26;
    color: #fff; cursor: pointer; font-size: 15px; font-weight: 600; }
  .deck-x:focus-visible { outline: 3px solid var(--accent); outline-offset: 3px; }
  .deck-empty, .deck-hint { margin: 0; pointer-events: none; color: #c2cbdb; text-align: center;
    font-size: calc(14px * var(--scale)); letter-spacing: .02em; }
  .deck-hint { padding: 0 5vw; }
  @media (prefers-reduced-motion: reduce) {
    .deck-open { transition: none !important; }
  }
</style>
