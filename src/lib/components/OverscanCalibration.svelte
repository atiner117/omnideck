<!-- Full-screen TV overscan calibration (the console-style "adjust until the markers are
     visible" screen). Owns the DRAFT inset value: every nudge reports it via `onpreview`
     so the page can apply it live to the whole UI; nothing is persisted until `onconfirm`.
     Input arrives through the page's overlay roster (this component exports `nudge` /
     `confirm` / `cancel` / `onkey` for the roster entry to forward into) — it never
     installs its own listeners, so overlay precedence stays in one place.

     Because the page shrinks <main> into the safe rect while calibrating (and a transformed
     <main> is the containing block for position:fixed descendants), this overlay's
     `inset: 0` IS the safe-area boundary: the user grows the inset until the corner
     markers and frame are fully visible on their panel. -->
<script lang="ts">
  import { dialogFocus } from "../dialog";

  let {
    pct: initial = 0,
    onpreview,
    onconfirm,
    oncancel,
  }: {
    /** Currently-saved inset (%, per edge) — the draft starts here. */
    pct?: number;
    /** Live preview: fired on every adjustment with the draft value. */
    onpreview: (pct: number) => void;
    /** A / Enter — persist the draft. */
    onconfirm: (pct: number) => void;
    /** B / Esc — discard the draft (the page reverts the preview). */
    oncancel: () => void;
  } = $props();

  const STEP = 0.5;
  const MAX = 10;
  const snap = (v: number) => Math.min(MAX, Math.max(0, Math.round(v / STEP) * STEP));
  // The draft deliberately captures `pct` at mount: the page remounts this component per
  // open ({#if overscanOpen}), and the prop must not fight the user's adjustments.
  // svelte-ignore state_referenced_locally
  let pct = $state(snap(initial));

  /** Grow (dir > 0) or shrink (dir < 0) the inset by one step. */
  export function nudge(dir: number) {
    pct = snap(pct + Math.sign(dir) * STEP);
    onpreview(pct);
  }
  export function confirm() {
    onconfirm(pct);
  }
  export function cancel() {
    oncancel();
  }
  // Keyboard: arrows auto-repeat natively (no page-side hold-repeat like the D-pad path),
  // so gate to the same ~9 steps/s the D-pad gets — a held arrow walks, not teleports.
  let lastKey = 0;
  export function onkey(e: KeyboardEvent) {
    const dir = { ArrowUp: 1, ArrowRight: 1, ArrowDown: -1, ArrowLeft: -1 }[e.key];
    if (dir !== undefined) {
      const n = performance.now();
      if (n - lastKey >= 110) {
        lastKey = n;
        nudge(dir);
      }
    } else if (e.key === "Enter") confirm();
    else if (e.key === "Escape") cancel();
  }
</script>

<div class="ovcal" role="dialog" aria-modal="true" aria-label="TV calibration" tabindex="-1" use:dialogFocus>
  <div class="frame" aria-hidden="true"></div>
  <span class="mark tl" aria-hidden="true">◤</span>
  <span class="mark tr" aria-hidden="true">◥</span>
  <span class="mark bl" aria-hidden="true">◣</span>
  <span class="mark br" aria-hidden="true">◢</span>

  <div class="card">
    <h2>TV calibration</h2>
    <p class="val" aria-live="polite">{pct.toFixed(1)}%</p>
    <p class="lead">
      Adjust until the frame and all four corner markers are fully visible,
      just touching the edges of your screen.
    </p>
    <div class="btns">
      <button class="cbtn" onclick={() => nudge(-1)} aria-label="Shrink safe area">−</button>
      <button class="cbtn" onclick={() => nudge(1)} aria-label="Grow safe area">+</button>
      <button class="cbtn save" onclick={confirm}>Save</button>
      <button class="cbtn" onclick={cancel}>Cancel</button>
    </div>
    <p class="hint"><b>▲▶</b> grow · <b>▼◀</b> shrink · <b>Enter/✕</b> save · <b>Esc/◯</b> cancel</p>
  </div>
</div>

<style>
  /* z 50: above every other fixed surface — deck is 41, deck-scrim/error-banner 40,
     Wizard 20, Modal 10/11. The banner is NOT a roster overlay (it can co-occur with
     calibration, e.g. a Steam library error at boot), so a z tie loses to its later
     DOM position; calibration's whole job is clean edges, so it must paint on top.
     If a new surface ever needs to beat this, bump deliberately — don't tie. */
  .ovcal {
    position: fixed; inset: 0; z-index: 50;
    background: #05070b; /* opaque: the frame/markers must be the only thing near the edges */
    display: grid; place-items: center;
    color: #eef2f8; outline: none;
  }
  .frame {
    position: absolute; inset: 3px;
    border: 3px solid var(--accent, #4cc2ff); border-radius: 6px;
    pointer-events: none;
  }
  .mark {
    position: absolute; font-size: clamp(28px, 4vmin, 52px); line-height: 1;
    color: var(--accent, #4cc2ff); pointer-events: none; user-select: none;
  }
  .mark.tl { top: 12px; left: 12px; }
  .mark.tr { top: 12px; right: 12px; }
  .mark.bl { bottom: 12px; left: 12px; }
  .mark.br { bottom: 12px; right: 12px; }
  .card {
    max-width: min(560px, 80vw); text-align: center;
    background: #121826; border: 1px solid color-mix(in srgb, var(--accent, #4cc2ff) 40%, transparent);
    border-radius: 18px; padding: 26px 34px; box-shadow: 0 30px 80px #000c;
  }
  h2 { margin: 0; font-size: clamp(20px, 2.2vw, 26px); }
  .val {
    margin: 10px 0 4px; font-size: clamp(34px, 4.5vw, 56px); font-weight: 800;
    font-variant-numeric: tabular-nums; color: var(--accent, #4cc2ff);
  }
  .lead { margin: 6px 0 14px; color: #cdd7e6; font-size: clamp(13px, 1.4vw, 17px); }
  .btns { display: flex; justify-content: center; gap: 10px; margin-bottom: 12px; }
  .cbtn {
    background: #1b2540; border: 1px solid color-mix(in srgb, var(--accent, #4cc2ff) 40%, transparent);
    color: #eef2f8; border-radius: 10px; padding: 8px 18px; min-width: 52px;
    font: inherit; font-weight: 700; cursor: pointer;
  }
  .cbtn:hover, .cbtn:focus-visible { border-color: var(--accent, #4cc2ff); }
  .cbtn.save { background: var(--accent, #4cc2ff); color: #04121f; border-color: transparent; }
  .hint { margin: 0; color: #7e8aa0; font-size: clamp(11px, 1.1vw, 14px); }
</style>
