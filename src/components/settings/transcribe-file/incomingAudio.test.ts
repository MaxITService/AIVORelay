import { expect, test } from "bun:test";
import { isSupportedAudioPath, queueIncomingAudio, shouldQueueIncomingAudio, shouldRouteAudioDrop, takeIncomingAudio } from "./incomingAudio";

test("routes supported audio only outside file and speech workspaces", () => {
  expect(isSupportedAudioPath("C:\\folder.wav\\voice.MP3")).toBe(true);
  expect(isSupportedAudioPath("C:\\folder.wav\\no-extension")).toBe(false);
  expect(isSupportedAudioPath("/tmp/.wav")).toBe(false);
  expect(shouldRouteAudioDrop("general", ["notes.txt", "/tmp/voice.wav"])).toBe(true);
  expect(shouldRouteAudioDrop("general", ["notes.txt"])).toBe(false);
  for (const section of ["transcribeFile", "textToSpeech", "ttsFiles"]) {
    expect(shouldRouteAudioDrop(section, ["voice.wav"])).toBe(false);
  }
});

test("queues incoming files instead of replacing a selection or interrupting work", () => {
  expect(shouldQueueIncomingAudio(1, false, "idle", false)).toBe(false);
  expect(shouldQueueIncomingAudio(2, false, "idle", false)).toBe(true);
  expect(shouldQueueIncomingAudio(1, true, "completed", false)).toBe(true);
  expect(shouldQueueIncomingAudio(1, false, "transcribing", false)).toBe(true);
  expect(shouldQueueIncomingAudio(1, false, "cancelling", false)).toBe(true);
  expect(shouldQueueIncomingAudio(1, false, "idle", true)).toBe(true);
});

test("preserves successive drops until they are consumed exactly once", () => {
  takeIncomingAudio();
  queueIncomingAudio(["one.wav", "notes.txt"]);
  queueIncomingAudio(["two.mp3"]);
  expect(takeIncomingAudio()).toEqual(["one.wav", "notes.txt", "two.mp3"]);
  expect(takeIncomingAudio()).toEqual([]);
});
