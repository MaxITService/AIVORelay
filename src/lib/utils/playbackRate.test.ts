import assert from "node:assert";
import {
  applyPlaybackRate,
  DEFAULT_PLAYBACK_RATE,
  formatPlaybackRate,
  normalizePlaybackRate,
  nextPlaybackRate,
  PLAYBACK_RATES,
} from "./playbackRate";

const cycledRates = PLAYBACK_RATES.map(nextPlaybackRate);
assert.deepEqual(cycledRates, [0.75, 1, 1.25, 1.5, 1.75, 2, 2.5, 3, 4, 0.5]);
assert.equal(nextPlaybackRate(DEFAULT_PLAYBACK_RATE), 1.25);
assert.equal(formatPlaybackRate(0.75), "0.75×");
assert.equal(formatPlaybackRate(4), "4×");
assert.equal(normalizePlaybackRate(0.1), 0.5);
assert.equal(normalizePlaybackRate(1.14), 1.25);
assert.equal(normalizePlaybackRate(9), 4);
assert.equal(normalizePlaybackRate(Number.NaN), DEFAULT_PLAYBACK_RATE);

const audio = {
  defaultPlaybackRate: 1,
  playbackRate: 1,
  preservesPitch: false,
} as HTMLAudioElement;

applyPlaybackRate(audio, 2.5);
assert.equal(audio.defaultPlaybackRate, 2.5);
assert.equal(audio.playbackRate, 2.5);
assert.equal(audio.preservesPitch, true);
