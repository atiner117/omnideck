<script lang="ts">
  // OLED burn-in screensaver overlay (roadmap #1, frontend half). Self-contained and
  // layout-mounted so it never touches +page.svelte: a fixed full-viewport layer above
  // everything that walks three stages while the user is idle —
  //   dim   (default 60 s):  fade the whole UI down behind a dark veil
  //   art   (default 180 s): near-black with a slow accent-gradient drift (the rails,
  //                          clock and Now Playing card are fully covered — nothing
  //                          static keeps burning)
  //   blank (default 600 s): true black + a small clock that changes position each
  //                          minute (the classic OLED pixel-shuffle)
  // Any input instantly restores. Timings/enable come from the `[screensaver]` config
  // table when the backend ships it (read defensively — this component works with or
  // without that backend: it also runs its own DOM+gamepad idle timer, so keyboard and
  // mouse idle count on desktops where the backend only watches the pad).
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import * as api from "$lib/backend";

  type SaverConf = { enabled: boolean; idle_dim_secs: number; ken_burns_secs: number; blank_secs: number };
  const DEFAULTS: SaverConf = { enabled: true, idle_dim_secs: 60, ken_burns_secs: 180, blank_secs: 600 };

  let conf = $state<SaverConf>(DEFAULTS);
  let accent = $state("#4cc2ff");
  let stage = $state<"off" | "dim" | "art" | "blank">("off");
  let playing = $state(false); // MPRIS says media is playing → never engage
  let clockText = $state("");
  let clockPos = $state({ x: 42, y: 46 }); // vw/vh percent-ish coordinates
  let reduced = $state(false);

  let lastActive = Date.now();
  let lastShuffle = 0;

  function wake() {
    lastActive = Date.now();
    stage = "off";
  }

  function shuffleClock(now: number) {
    lastShuffle = now;
    clockText = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    // Keep the clock inside a comfortable band (10–75%) so it never clips.
    clockPos = { x: 10 + Math.random() * 65, y: 10 + Math.random() * 65 };
  }

  function tick() {
    if (!conf.enabled || playing) {
      if (stage !== "off") stage = "off";
      lastActive = Date.now();
      return;
    }
    const now = Date.now();
    const idle = (now - lastActive) / 1000;
    const next = idle >= conf.blank_secs ? "blank" : idle >= conf.ken_burns_secs ? "art" : idle >= conf.idle_dim_secs ? "dim" : "off";
    if (next !== stage) stage = next;
    // Refresh + reposition the blank-stage clock once a minute (pixel shuffle).
    if (stage === "blank" && now - lastShuffle >= 60_000) shuffleClock(now);
  }

  // While engaged, the waking key/click must only WAKE — swallow it before the page's
  // input router navigates the XMB or a button underneath gets clicked. (Gamepad wake
  // goes through the page's own listener, so the first pad press may also navigate —
  // acceptable v1, the backend idle events land before the UI event either way.)
  function onWindowKey(e: KeyboardEvent) {
    if (stage !== "off") {
      e.preventDefault();
      e.stopPropagation();
    }
    wake();
  }
  function onWindowPointer() {
    wake();
  }

  onMount(() => {
    reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    shuffleClock(Date.now());

    // Config: the `[screensaver]` table exists once the backend half lands; until then
    // (or on parse trouble) the defaults above apply. Read defensively on purpose.
    api
      .getConfig()
      .then((cfg) => {
        accent = cfg.settings?.accent || accent;
        const ss = (cfg as unknown as Record<string, unknown>).screensaver as Partial<SaverConf> | undefined;
        if (ss && typeof ss === "object") conf = { ...DEFAULTS, ...ss };
      })
      .catch(() => {}); // no config → defaults; the overlay must never break boot

    api
      .mediaNowPlaying()
      .then((m) => (playing = m?.status === "Playing"))
      .catch(() => {});

    const unsubs: Array<Promise<() => void>> = [
      // Backend idle detection (gamepad thread) — trust it when present.
      listen("idle", () => {
        if (!playing && conf.enabled) lastActive = Math.min(lastActive, Date.now() - conf.idle_dim_secs * 1000);
      }),
      listen("active", () => wake()),
      // Pad input also resets the local timer (desktop keyboards/mice are covered by
      // the window listeners below; this covers the controller path everywhere).
      api.onGamepad(() => wake()),
      // Never dim over a movie: MPRIS pushes state changes, no polling.
      api.onMediaChanged((e) => {
        playing = e.payload?.status === "Playing";
        if (playing) wake();
      }),
    ];

    window.addEventListener("keydown", onWindowKey, true);
    window.addEventListener("pointerdown", onWindowPointer, true);
    window.addEventListener("pointermove", onWindowPointer, true);
    window.addEventListener("wheel", onWindowPointer, true);

    const timer = setInterval(tick, 1000);
    return () => {
      clearInterval(timer);
      window.removeEventListener("keydown", onWindowKey, true);
      window.removeEventListener("pointerdown", onWindowPointer, true);
      window.removeEventListener("pointermove", onWindowPointer, true);
      window.removeEventListener("wheel", onWindowPointer, true);
      for (const u of unsubs) u.then((f) => f()).catch(() => {});
    };
  });
</script>

{#if stage !== "off"}
  <div
    class="saver s-{stage}"
    class:reduced
    style="--sv-accent: {accent}"
    aria-hidden="true"
    data-testid="screensaver"
  ></div>
  {#if stage === "blank"}
    <div class="sv-clock" style="left: {clockPos.x}vw; top: {clockPos.y}vh" aria-hidden="true">{clockText}</div>
  {/if}
{/if}

<style>
  .saver {
    position: fixed;
    inset: 0;
    z-index: 200; /* above every modal/card — the point is covering ALL static pixels */
    background: #000;
    opacity: 0;
    animation: sv-fade 2s ease forwards;
    pointer-events: none; /* input goes to the window listeners; nothing here to click */
  }
  .s-dim {
    --sv-target: 0.82;
  }
  .s-art,
  .s-blank {
    --sv-target: 1;
  }
  @keyframes sv-fade {
    to {
      opacity: var(--sv-target, 0.82);
    }
  }
  /* Stage b: a very dark, slowly drifting accent wash — visibly "alive" so it reads as a
     screensaver rather than a crash, but at ~6% intensity nothing static remains. */
  .s-art::after {
    content: "";
    position: absolute;
    inset: -20%;
    background: radial-gradient(
      42% 42% at 50% 50%,
      color-mix(in srgb, var(--sv-accent) 14%, transparent),
      transparent 70%
    );
    animation: sv-drift 48s linear infinite alternate;
  }
  .reduced.s-art::after {
    animation: none; /* prefers-reduced-motion: static wash, still covers the pixels */
  }
  @keyframes sv-drift {
    from {
      transform: translate(-8%, -6%) scale(1);
    }
    to {
      transform: translate(8%, 6%) scale(1.15);
    }
  }
  .sv-clock {
    position: fixed;
    z-index: 201;
    color: #9aa7b8;
    font-size: 2.2vmin;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.08em;
    opacity: 0.55;
    pointer-events: none;
    user-select: none;
  }
</style>
