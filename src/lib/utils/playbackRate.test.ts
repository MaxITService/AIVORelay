import assert from "node:assert";
import {
  applyPlaybackRate,
  DEFAULT_PLAYBACK_RATE,
  formatPlaybackRate,
  nextPlaybackRate,
  PLAYBACK_RATES,
} from "./playbackRate";

const cycledRates = PLAYBACK_RATES.map(nextPlaybackRate);
assert.deepEqual(cycledRates, [1, 1.25, 1.5, 1.75, 2, 2.5, 3, 0.75]);
assert.equal(nextPlaybackRate(DEFAULT_PLAYBACK_RATE), 1.25);
assert.equal(formatPlaybackRate(0.75), "0.75×");
assert.equal(formatPlaybackRate(3), "3×");

const audio = {
  defaultPlaybackRate: 1,
  playbackRate: 1,
  preservesPitch: false,
} as HTMLAudioElement;

applyPlaybackRate(audio, 2.5);
assert.equal(audio.defaultPlaybackRate, 2.5);
assert.equal(audio.playbackRate, 2.5);
assert.equal(audio.preservesPitch, true);
