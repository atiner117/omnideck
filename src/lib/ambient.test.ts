import { describe, expect, it } from "vitest";
import { BREATH_DEPTH, HIGHPASS_HZ, PARTIAL_LEVEL, RATIOS, ROOTS, voicingHz } from "./ambient";

// The pad is heard through PipeWire's stereo→5.1 upmix on the TV: anything under the
// ~150 Hz LFE crossover lands on the subwoofer as a hum. Every voiced partial, at every
// root, must sit above the highpass corner so nothing is even asked of the sub.
describe("ambient voicing", () => {
  it("keeps every partial above the sub crossover for every root", () => {
    for (const root of ROOTS) {
      for (const hz of voicingHz(root)) expect(hz).toBeGreaterThan(HIGHPASS_HZ);
    }
  });

  it("stays a low-mid pad, not a whistle", () => {
    for (const root of ROOTS) expect(Math.max(...voicingHz(root))).toBeLessThan(1500);
  });

  it("roots are consonant neighbours in one octave", () => {
    const lo = Math.min(...ROOTS), hi = Math.max(...ROOTS);
    expect(hi / lo).toBeLessThan(2);
  });

  it("breathing can never drive a partial gain negative", () => {
    expect(BREATH_DEPTH).toBeLessThan(1);
    expect(RATIOS.length).toBe(PARTIAL_LEVEL.length);
    for (const p of PARTIAL_LEVEL) expect(p - p * BREATH_DEPTH).toBeGreaterThan(0);
  });
});
