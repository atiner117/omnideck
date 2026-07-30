<!-- Parental-controls PIN pad (roadmap #2, frontend): shown when a locked category (or a
     locked tile from search/recents) is activated. Purely presentational (the
     CatalogModal/Wizard pattern): the page's unified keyboard+gamepad router owns the
     entered digits + pad focus and calls verify_pin/set_pin; pointer presses report back
     through onpress. The pad's grid shape is exported so the router's D-pad movement uses
     the same source of truth (the OSK_ROWS idiom from osk.ts, numeric/hidden variant).
     Deterrence, not security — see the threat-model note in the backend module. -->
<script lang="ts" module>
  /** Numeric pad layout: phone order, delete/submit on the bottom row. */
  export const PIN_ROWS = [
    ["1", "2", "3"],
    ["4", "5", "6"],
    ["7", "8", "9"],
    ["⌫", "0", "⏎"],
  ];
  export const PIN_FLAT = PIN_ROWS.flat();
  export const PIN_COLS = 3;
  /** Sensible PIN length cap the page can share (Enter submits earlier lengths). */
  export const PIN_MAX = 8;
</script>

<script lang="ts">
  import Modal from "./Modal.svelte";

  let {
    title = "Enter PIN",
    entered,
    focus,
    busy = false,
    error = "",
    onfocus,
    onpress,
    onclose,
  }: {
    /** Heading: "Enter PIN" for the gate, "Set PIN" / "Confirm PIN" for setup flows. */
    title?: string;
    /** How many digits are currently entered (the digits themselves stay in the page). */
    entered: number;
    /** Focused pad-key index into PIN_FLAT (page-owned). */
    focus: number;
    /** True while the argon2 verify is in flight — pad dims, input should be ignored. */
    busy?: boolean;
    /** Inline failure line ("Wrong PIN — try again"); empty = none. */
    error?: string;
    onfocus: (i: number) => void;
    /** A pad key was pressed: a digit, "⌫", or "⏎". */
    onpress: (key: string) => void;
    onclose: () => void;
  } = $props();
</script>

<Modal labelledby="dlg-pin" backdropLabel="Cancel PIN entry" closeLabel="Cancel PIN entry" {onclose}>
  <h2 id="dlg-pin">{title}</h2>
  <div class="pin-dots" class:shake={!!error} aria-label="{entered} digit(s) entered">
    {#each Array(PIN_MAX) as _, i (i)}
      <span class="pin-dot" class:filled={i < entered}></span>
    {/each}
  </div>
  {#if error}
    <div class="pin-err" role="alert">{error}</div>
  {/if}
  <div class="pin-pad" class:busy>
    {#each PIN_FLAT as key, i (i)}
      <button
        type="button"
        class="pin-key"
        class:focused={i === focus}
        class:action={key === "⌫" || key === "⏎"}
        disabled={busy}
        onmouseenter={() => onfocus(i)}
        onclick={() => {
          onfocus(i);
          onpress(key);
        }}
      >
        {key}
      </button>
    {/each}
  </div>
  <p class="phint">digits enter · ⌫ delete · ⏎/Enter confirm · Esc cancel</p>
</Modal>

<style>
  .pin-dots {
    display: flex;
    gap: 12px;
    justify-content: center;
    padding: 10px 0 4px;
  }
  .pin-dot {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid color-mix(in srgb, var(--accent) 55%, transparent);
    background: transparent;
    transition: background 0.12s;
  }
  .pin-dot.filled {
    background: var(--accent);
  }
  .pin-dots.shake {
    animation: pin-shake 0.3s;
  }
  @keyframes pin-shake {
    20% { transform: translateX(-7px); }
    45% { transform: translateX(6px); }
    70% { transform: translateX(-4px); }
    90% { transform: translateX(2px); }
  }
  @media (prefers-reduced-motion: reduce) {
    .pin-dots.shake {
      animation: none;
    }
  }
  .pin-err {
    color: #ff9d9d;
    text-align: center;
    font-size: clamp(12px, 1.2vw, 15px);
    padding: 2px 0 4px;
  }
  .pin-pad {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
    margin: 8px auto 4px;
    width: min(280px, 80%);
  }
  .pin-pad.busy {
    opacity: 0.55;
    pointer-events: none;
  }
  .pin-key {
    background: #1b2540;
    border: 2px solid transparent;
    color: inherit;
    font: inherit;
    font-size: clamp(16px, 1.8vw, 22px);
    font-weight: 700;
    border-radius: 12px;
    padding: 12px 0;
    cursor: pointer;
  }
  .pin-key.action {
    color: var(--accent);
  }
  .pin-key.focused {
    background: #223057;
    border-color: var(--accent);
  }
</style>
