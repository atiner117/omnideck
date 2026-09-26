// Synthesized ambient background music (no shipped audio assets — same rule as sfx.ts).
//
// A very quiet, slowly breathing pad in the PS3/Wii-menu tradition: four partials over a
// drifting root (root, fifth, octave, a slightly-sharp twelfth for shimmer), each voiced as
// a detuned pair so it reads as a pad rather than a test tone, behind a gently sweeping
// lowpass. The root glides between four neighbouring keys every ~35 s, so it never loops
// audibly and never demands attention. Master level tops out well below the nav sounds.
//
// Voicing rules (learned on the living-room system, 2026-09-26): the pad lives in the
// A3–B3 octave and everything passes a 160 Hz highpass. PipeWire upmixes a stereo stream
// onto a 5.1 HDMI sink and routes anything under its ~150 Hz LFE crossover to the
// subwoofer — the original A2-rooted sine pad came out of a soundbar+sub as a pure hum,
// and its deep amplitude LFOs (±45 %) read as "oscillating". Breathing is now shallow.
//
// Driven by the page: ambientApply(on, volume) is idempotent and cheap to call from a
// settings $effect; stop ramps out and tears the graph down.

let ctx: AudioContext | null = null;
let master: GainNode | null = null;
let oscs: OscillatorNode[] = [];
let lfos: OscillatorNode[] = [];
let drift: ReturnType<typeof setInterval> | undefined;
let currentVol = 0;

/** Partial ratios over the root: root · fifth · octave · slightly-sharp twelfth. */
export const RATIOS = [1, 1.5, 2, 3.02];
export const PARTIAL_LEVEL = [1, 0.55, 0.4, 0.18];
/** A3 · G3 · F3 · B3 — close, consonant neighbours, one octave above the sub range. */
export const ROOTS = [220, 196, 174.61, 246.94];
/** Highpass corner: keeps the pad clear of PipeWire's default 150 Hz LFE crossover. */
export const HIGHPASS_HZ = 160;
/** Detune of each partial's second voice, in cents (±). Pad chorus, not vibrato. */
export const DETUNE_CENTS = 4;
/** Amplitude "breathing" depth per partial (fraction of its level). Shallow on purpose. */
export const BREATH_DEPTH = 0.12;
const level = (v: number) => v * 0.055; // "smooth", not "present": whisper under everything

/** The partial frequencies (Hz) the pad voices for a given root — pure, for tests. */
export function voicingHz(root: number): number[] {
  return RATIOS.map((r) => root * r);
}

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

    // Highpass first: nothing below the sub crossover ever reaches the output.
    const highpass = ctx.createBiquadFilter();
    highpass.type = "highpass";
    highpass.frequency.value = HIGHPASS_HZ;
    highpass.Q.value = 0.7;

    const filter = ctx.createBiquadFilter();
    filter.type = "lowpass";
    filter.frequency.value = 1200;
    filter.Q.value = 0.4;
    const sweep = ctx.createOscillator();
    const sweepDepth = ctx.createGain();
    sweep.frequency.value = 0.012; // one filter breath per ~80 s
    sweepDepth.gain.value = 200;
    sweep.connect(sweepDepth).connect(filter.frequency);
    sweep.start();
    lfos.push(sweep);

    let root = ROOTS[0];
    for (let i = 0; i < RATIOS.length; i++) {
      const g = ctx.createGain();
      g.gain.value = PARTIAL_LEVEL[i];
      // Per-partial breathing so the chord is alive without throbbing: gain stays within
      // ±BREATH_DEPTH of its level, on a sub-0.1 Hz cycle unique to each partial.
      const lfo = ctx.createOscillator();
      const depth = ctx.createGain();
      lfo.frequency.value = 0.02 + i * 0.011;
      depth.gain.value = PARTIAL_LEVEL[i] * BREATH_DEPTH;
      lfo.connect(depth).connect(g.gain);
      lfo.start();
      lfos.push(lfo);
      // Two sines a few cents apart per partial: slow beating = pad shimmer.
      for (const cents of [-DETUNE_CENTS, DETUNE_CENTS]) {
        const osc = ctx.createOscillator();
        osc.type = "sine";
        osc.frequency.value = root * RATIOS[i];
        osc.detune.value = cents;
        osc.connect(g);
        osc.start();
        oscs.push(osc);
      }
      g.connect(highpass);
    }
    highpass.connect(filter).connect(master).connect(ctx.destination);

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
        o.frequency.exponentialRampToValueAtTime(root * RATIOS[Math.floor(i / 2)], t + 10);
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
