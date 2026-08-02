<!-- Sleep timer modal ("stop playback in N minutes" — feature-backlog parking lot).
     Preset rows arm/replace the backend timer; while one is armed, a countdown header and a
     Cancel row appear. Purely presentational (the Wizard doctrine): the page owns the open
     flag, the focus index, and all input routing (↑↓/dpad move focus, Enter/✕ activates the
     focused row, Esc/◯ closes); intents come back through the on* callbacks. Row styling is
     the shared modal vocabulary from Modal.svelte (.catlist/.crow/.cname/.ccat).
     Focus contract: indices 0..SLEEP_PRESETS.length-1 are the presets; when `remaining` is
     non-null one extra row (index SLEEP_PRESETS.length) is Cancel — clamp accordingly.
     NOT wired into +page.svelte here — the integration pass owns mounting + routing. -->
<script lang="ts" module>
  /** Preset durations in minutes. Exported so the page's focus clamp and any quick-arm
   *  affordance share one source of truth. */
  export const SLEEP_PRESETS = [15, 30, 45, 60, 90];

  /** 900 → "15:00", 65 → "1:05", 5400 → "1:30:00" — countdown-style, no unit soup. */
  export function formatRemaining(secs: number): string {
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = secs % 60;
    const mm = h ? String(m).padStart(2, "0") : String(m);
    return `${h ? `${h}:` : ""}${mm}:${String(s).padStart(2, "0")}`;
  }

  /** Wall-clock end time for a preset ("until 23:41") — the couch question is "when will it
   *  stop", not "how long is 45 minutes". Re-evaluated on every render pass, so it stays
   *  honest while the dialog sits open (any tick or focus move re-renders). */
  export function endsAt(minutes: number, now = new Date()): string {
    const end = new Date(now.getTime() + minutes * 60_000);
    return `${end.getHours()}:${String(end.getMinutes()).padStart(2, "0")}`;
  }
</script>

<script lang="ts">
  import Modal from "./Modal.svelte";

  let {
    remaining,
    focus,
    onfocus,
    onarm,
    oncancel,
    onclose,
  }: {
    /** Seconds left on the armed timer, or null when none is armed. The page owns the
     *  countdown (initial `getSleepTimer()`, then `sleep-timer-tick` / its own 1 s tick). */
    remaining: number | null;
    focus: number;
    onfocus: (i: number) => void;
    /** A preset row was activated — the page calls `setSleepTimer(minutes)`. */
    onarm: (minutes: number) => void;
    /** The Cancel row was activated — the page calls `cancelSleepTimer()`. */
    oncancel: () => void;
    onclose: () => void;
  } = $props();

  const cancelIndex = SLEEP_PRESETS.length;
</script>

<Modal labelledby="dlg-sleep" backdropLabel="Close sleep timer" closeLabel="Close sleep timer" {onclose}>
  <h2 id="dlg-sleep">Sleep timer</h2>
  {#if remaining != null}
    <div class="s-armed" class:s-soon={remaining <= 60} role="timer" aria-live="off">
      ⏾ Pausing playback in <b>{formatRemaining(remaining)}</b>
    </div>
  {:else}
    <div class="cgroup">Pause playback after…</div>
  {/if}
  <div class="catlist">
    {#each SLEEP_PRESETS as m, i (m)}
      <button type="button" class="crow" class:focused={i === focus} data-sleep={i}
        onmouseenter={() => onfocus(i)} onclick={() => { onfocus(i); onarm(m); }}>
        <span class="cname">{m} minutes</span>
        <span class="ccat">until {endsAt(m)}</span>
      </button>
    {/each}
    {#if remaining != null}
      <button type="button" class="crow" class:focused={focus === cancelIndex} data-sleep={cancelIndex}
        onmouseenter={() => onfocus(cancelIndex)} onclick={() => { onfocus(cancelIndex); oncancel(); }}>
        <span class="cname">Cancel timer</span>
        <span class="ccat">keep playing</span>
      </button>
    {/if}
  </div>
  <p class="phint">Esc/◯ close · ↑↓ select · Enter/✕ {focus === cancelIndex ? "cancel timer" : remaining != null ? "restart timer" : "start timer"}</p>
</Modal>

<style>
  .s-armed { display: flex; align-items: center; gap: 10px; margin: 6px 0 12px; padding: 12px 16px; border: 1px solid color-mix(in srgb, var(--accent) 45%, transparent); border-radius: 12px; background: var(--surface-3); color: var(--text); font-size: clamp(14px, 1.5vw, 18px); }
  .s-armed b { font-variant-numeric: tabular-nums; }
  /* Final minute (the backend is ticking): make the countdown impossible to miss. */
  .s-soon { border-color: var(--accent); animation: s-pulse 1.2s ease-in-out infinite; }
  @keyframes s-pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.55; } }
  @media (prefers-reduced-motion: reduce) {
    .s-soon { animation: none; }
  }
</style>
