// Synthesized navigation sounds (no shipped audio assets). The page registers a prefs
// getter once (reads its reactive settings at call time), then nav code just calls
// sfxMove()/sfxEnter(); a settings preview passes force=true to sound while toggling.
let prefs: () => { on: boolean; volume: number } = () => ({ on: false, volume: 0.6 });
export function initSfx(getter: typeof prefs) {
  prefs = getter;
}

let actx: AudioContext | null = null;
export function blip(freq: number, dur = 0.05, base = 0.2, type: OscillatorType = "sine", force = false) {
  const { on, volume } = prefs();
  if (!force && !on) return;
  try {
    actx ??= new AudioContext();
    if (actx.state === "suspended") actx.resume();
    const o = actx.createOscillator(), g = actx.createGain();
    o.type = type; o.frequency.value = freq;
    g.gain.setValueAtTime(Math.max(0.0001, base * volume), actx.currentTime);
    g.gain.exponentialRampToValueAtTime(0.0001, actx.currentTime + dur);
    o.connect(g).connect(actx.destination);
    o.start(); o.stop(actx.currentTime + dur);
  } catch { /* AudioContext unavailable/blocked (autoplay policy) — nav sound is non-essential */ }
}
export const sfxMove = () => blip(420, 0.04, 0.32, "triangle");
export const sfxEnter = () => { blip(620, 0.06, 0.42); setTimeout(() => blip(880, 0.07, 0.38), 45); };
