<!--
  Inline layout picker for the Settings "Library layout" row (appearance.layout).

  Self-contained so a future Appearance settings section can slot it in unchanged: it takes
  the current mode + an onchange and renders the four modes as clickable segments plus the
  current label. It lives INSIDE the settings row's <button> (same pattern as the accent
  row's color wheel), so segments are role="radio" spans — not nested <button>s — and every
  handler stops propagation so a segment click doesn't also cycle the row.
-->
<script lang="ts">
  import { LAYOUT_MODES, type LayoutId } from "./layouts";

  let { value, onchange }: { value: string; onchange: (v: LayoutId) => void } = $props();

  function pick(e: Event, v: LayoutId) {
    e.stopPropagation();
    onchange(v);
  }
</script>

<span class="lp" role="radiogroup" aria-label="Library layout">
  {#each LAYOUT_MODES as m (m.id)}
    <!-- Mouse-only by design: keyboard/gamepad cycle the enclosing settings row via the
         page router (which preventDefaults Enter/Space globally), and tabindex=-1 keeps
         the span unfocusable — a keydown handler here would be dead code. -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <span
      class="lpseg"
      class:on={value === m.id}
      role="radio"
      aria-checked={value === m.id}
      tabindex="-1"
      title={m.label}
      onclick={(e) => pick(e, m.id)}
    >
      {#if m.id === "rail"}
        <!-- XMB cascade: offset bars, the focused one larger -->
        <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="3" y="1.5" width="9" height="2.6" rx="1" /><rect x="1" y="6.2" width="14" height="3.6" rx="1.2" /><rect x="3" y="11.9" width="9" height="2.6" rx="1" /></svg>
      {:else if m.id === "grid"}
        <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="1" y="1" width="6.2" height="6.2" rx="1.4" /><rect x="8.8" y="1" width="6.2" height="6.2" rx="1.4" /><rect x="1" y="8.8" width="6.2" height="6.2" rx="1.4" /><rect x="8.8" y="8.8" width="6.2" height="6.2" rx="1.4" /></svg>
      {:else if m.id === "grid-compact"}
        <svg viewBox="0 0 16 16" aria-hidden="true">{#each [1, 6, 11] as y}{#each [1, 6, 11] as x}<rect x={x} y={y} width="4" height="4" rx="1" />{/each}{/each}</svg>
      {:else}
        <svg viewBox="0 0 16 16" aria-hidden="true">{#each [2, 6.8, 11.6] as y}<circle cx="2.4" cy={y + 1.2} r="1.3" /><rect x="5.4" y={y} width="9.6" height="2.4" rx="1" />{/each}</svg>
      {/if}
    </span>
  {/each}
  <span class="lpcur">{LAYOUT_MODES.find((m) => m.id === value)?.label ?? "Rail"}</span>
</span>

<style>
  .lp { display: inline-flex; align-items: center; gap: 5px; margin-left: 12px; vertical-align: middle; }
  .lpseg {
    display: inline-grid; place-items: center;
    width: 27px; height: 23px; border-radius: 7px;
    background: #ffffff12; color: #8a96ab;
    cursor: pointer; transition: background 0.12s, color 0.12s;
  }
  .lpseg:hover { color: #dfe7f2; background: #ffffff22; }
  .lpseg.on { background: var(--accent); color: #04121f; }
  .lpseg svg { width: 14px; height: 14px; fill: currentColor; }
  .lpcur { color: var(--accent); font-weight: 700; font-size: 0.8em; margin-left: 5px; }
  @media (prefers-reduced-motion: reduce) {
    .lpseg { transition: none; }
  }
</style>
