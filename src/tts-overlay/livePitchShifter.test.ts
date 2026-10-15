import { expect, test } from "bun:test";
import { isPitchShiftBypassed, pitchShiftCycle } from "./livePitchShifter";

const SAMPLE_RATE = 24_000;

// A delay line read at delay d(t) outputs input(t - d(t)), so its read rate is
// 1 - d'(t). The sweep must make that equal the requested ratio.
function readRate(delay: Float32Array) {
  return 1 - (delay[1] - delay[0]) * SAMPLE_RATE;
}

for (const ratio of [0.5, 0.8, 1.25, 2, 4]) {
  test(`ratio ${ratio}: delay sweep reads the input ${ratio} times as fast`, () => {
    const { delay } = pitchShiftCycle(ratio, SAMPLE_RATE);
    expect(readRate(delay)).toBeCloseTo(ratio, 2);
  });

  test(`ratio ${ratio}: lines half a cycle apart crossfade at constant gain`, () => {
    const { gain } = pitchShiftCycle(ratio, SAMPLE_RATE);
    const half = gain.length / 2;
    for (let i = 0; i < gain.length; i += 1) {
      expect(gain[i] + gain[(i + half) % gain.length]).toBeCloseTo(1, 5);
    }
  });

  test(`ratio ${ratio}: each line wraps its delay while silent`, () => {
    const { delay, gain } = pitchShiftCycle(ratio, SAMPLE_RATE);
    const last = delay.length - 1;
    expect(gain[0]).toBe(0);
    expect(Math.abs(delay[last] - delay[0])).toBeGreaterThan(0.09);
  });
}

test("ratios too close to 1 to hear bypass the shifter", () => {
  expect(isPitchShiftBypassed(1)).toBe(true);
  expect(isPitchShiftBypassed(1.005)).toBe(true);
  expect(isPitchShiftBypassed(Number.NaN)).toBe(true);
  expect(isPitchShiftBypassed(0)).toBe(true);
  expect(isPitchShiftBypassed(1.05)).toBe(false);
});
