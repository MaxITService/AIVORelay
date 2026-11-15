import { expect, test } from "bun:test";
import { applyPlaybackRate, normalizePlaybackRate } from "./playbackRate";

const audioElement = () => ({
  defaultPlaybackRate: 2,
  playbackRate: 2,
  preservesPitch: false,
}) as HTMLAudioElement;

test("equidistant playback presets choose the slower rate deterministically", () => {
  expect(normalizePlaybackRate(0.625)).toBe(0.5);
  expect(normalizePlaybackRate(1.125)).toBe(1);
  expect(normalizePlaybackRate(3.5)).toBe(3);
});

test("non-finite playback speeds reset both audio rates and retain pitch preservation", () => {
  for (const rate of [Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY]) {
    const audio = audioElement();
    applyPlaybackRate(audio, rate);
    expect(audio.defaultPlaybackRate).toBe(1);
    expect(audio.playbackRate).toBe(1);
    expect(audio.preservesPitch).toBe(true);
    expect(normalizePlaybackRate(rate)).toBe(1);
  }
});

test("zero and negative playback speeds clamp to the browser's safe lower bound", () => {
  for (const rate of [0, -1, 0.001]) {
    const audio = audioElement();
    applyPlaybackRate(audio, rate);
    expect(audio.defaultPlaybackRate).toBe(0.0625);
    expect(audio.playbackRate).toBe(0.0625);
    expect(audio.preservesPitch).toBe(true);
  }
});

test("excessive playback speeds clamp to the browser's safe upper bound", () => {
  const audio = audioElement();
  applyPlaybackRate(audio, 100);
  expect(audio.defaultPlaybackRate).toBe(16);
  expect(audio.playbackRate).toBe(16);
  expect(audio.preservesPitch).toBe(true);
});
