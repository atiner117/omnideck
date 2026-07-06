// Synthesized ambient background music (no shipped audio assets — same rule as sfx.ts).
//
// A very quiet, slowly breathing pad in the PS3/Wii-menu tradition: four sine partials
// over a drifting root (root, fifth, octave, a slightly-sharp twelfth for shimmer), each
// with its own sub-0.1 Hz amplitude LFO, the whole thing behind a gently sweeping lowpass.
// The root glides between four neighbouring keys every ~35 s, so it never loops audibly
// and never demands attention. Master level tops out well below the nav sounds.
//
// Driven by the page: ambientApply(on, volume) is idempotent and cheap to call from a
// settings $effect; stop ramps out and tears the graph down.

let ctx: AudioContext | null = null;
let master: GainNode | null = null;
let oscs: OscillatorNode[] = [];
let lfos: OscillatorNode[] = [];
let drift: ReturnType<typeof setInterval> | undefined;
let currentVol = 0;

const RATIOS = [1, 1.5, 2, 3.02];
const PARTIAL_LEVEL = [1, 0.5, 0.33, 0.16];
const ROOTS = [110, 98, 87.31, 123.47]; // A2 · G2 · F2 · B2 — close, consonant neighbours
const level = (v: number) => v * 0.055; // "smooth", not "present": whisper under everything

export function ambientApply(on: boolean, volume: number) {
  if (!on) { ambientStop(); return; }
  if (ctx && master) {
    if (volume !== currentVol) {
      currentVol = volume;
      master.gain.setTargetAtTime(level(volume), ctx.currentTime, 0.4);
    }
    return;
  }
  try {
    ctx = new AudioContext();
    if (ctx.state === "suspended") ctx.resume();
    const t0 = ctx.currentTime;
    currentVol = volume;

    master = ctx.createGain();
    master.gain.setValueAtTime(0.0001, t0);
    master.gain.exponentialRampToValueAtTime(Math.max(0.0001, level(volume)), t0 + 5); // slow fade-in

    const filter = ctx.createBiquadFilter();
    filter.type = "lowpass";
    filter.frequency.value = 850;
    filter.Q.value = 0.4;
    const sweep = ctx.createOscillator();
    const sweepDepth = ctx.createGain();
    sweep.frequency.value = 0.018; // one filter breath per ~55 s
    sweepDepth.gain.value = 260;
    sweep.connect(sweepDepth).connect(filter.frequency);
    sweep.start();
    lfos.push(sweep);

    let root = ROOTS[0];
    for (let i = 0; i < RATIOS.length; i++) {
      const osc = ctx.createOscillator();
      osc.type = "sine";
      osc.frequency.value = root * RATIOS[i];
      const g = ctx.createGain();
      g.gain.value = PARTIAL_LEVEL[i];
      // Per-partial breathing so the chord shimmers instead of droning.
      const lfo = ctx.createOscillator();
      const depth = ctx.createGain();
      lfo.frequency.value = 0.03 + i * 0.017;
      depth.gain.value = PARTIAL_LEVEL[i] * 0.45;
      lfo.connect(depth).connect(g.gain);
      lfo.start();
      osc.connect(g).connect(filter);
      osc.start();
      oscs.push(osc);
      lfos.push(lfo);
    }
    filter.connect(master).connect(ctx.destination);

    // Root drift: glide everything to a neighbouring key, 10 s per glide, every ~35 s.
    let step = 0;
    drift = setInterval(() => {
      if (!ctx) return;
      step = (step + 1 + Math.floor(Math.random() * 2)) % ROOTS.length;
      root = ROOTS[step];
      const t = ctx.currentTime;
      oscs.forEach((o, i) => {
        o.frequency.cancelScheduledValues(t);
        o.frequency.setValueAtTime(o.frequency.value, t);
        o.frequency.exponentialRampToValueAtTime(root * RATIOS[i], t + 10);
      });
    }, 35000);
  } catch {
    ambientStop(); // AudioContext unavailable/blocked — music is strictly optional
  }
}

export function ambientStop() {
  clearInterval(drift);
  drift = undefined;
  if (ctx && master) {
    const c = ctx, m = master, os = oscs, ls = lfos;
    m.gain.setTargetAtTime(0.0001, c.currentTime, 0.5);
    setTimeout(() => {
      os.forEach((o) => { try { o.stop(); } catch { /* already stopped */ } });
      ls.forEach((o) => { try { o.stop(); } catch { /* already stopped */ } });
      c.close().catch(() => {});
    }, 1800);
  }
  ctx = null;
  master = null;
  oscs = [];
  lfos = [];
  currentVol = 0;
}
