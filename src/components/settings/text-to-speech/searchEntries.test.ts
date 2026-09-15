import { describe, expect, it } from "bun:test";
import type { AppSettings } from "@/bindings";
import { textToSpeechSearchEntries } from "./searchEntries";

const expectedIdsBySection = {
  textToSpeech: [
    "tts",
    "tts-actions",
    "tts-read-clipboard",
    "tts-read-selection",
    "tts-read-selection-direct",
    "tts-history-fallback-shortcut",
    "tts-gallery",
    "tts-presets",
    "tts-local-model",
    "tts-voice-settings",
    "tts-playback",
    "tts-overlay",
    "tts-ai-cleanup",
    "tts-ai-cleanup-benchmark",
    "tts-preprocessing",
    "tts-chunking",
    "tts-api",
    "tts-interactive-history",
  ],
  ttsFiles: [
    "tts-files",
    "tts-files-gallery",
    "tts-files-unfinished",
    "tts-files-conversion",
    "tts-files-output-format",
    "tts-files-batch",
    "tts-files-folder-automation",
    "tts-files-presets",
    "tts-files-local-model",
    "tts-files-voice-settings",
    "tts-files-playback",
    "tts-files-ai-cleanup",
    "tts-files-ai-cleanup-benchmark",
    "tts-files-preprocessing",
    "tts-files-chunking",
    "tts-files-api",
    "tts-files-history",
  ],
} as const;

describe("TTS settings search catalog", () => {
  it("covers every major Interactive and File TTS settings group", () => {
    for (const [section, expectedIds] of Object.entries(
      expectedIdsBySection,
    )) {
      expect(
        textToSpeechSearchEntries
          .filter((entry) => entry.section === section)
          .map((entry) => entry.id),
      ).toEqual(expectedIds);
    }
  });

  it("uses unique ids, direct anchors, and bilingual discovery terms", () => {
    const ids = textToSpeechSearchEntries.map((entry) => entry.id);
    expect(new Set(ids).size).toBe(ids.length);

    for (const entry of textToSpeechSearchEntries) {
      expect(entry.anchor.trim().length).toBeGreaterThan(0);
      expect(entry.keywords.some((keyword) => /[a-z]/i.test(keyword))).toBe(
        true,
      );
      expect(entry.keywords.some((keyword) => /[а-яё]/i.test(keyword))).toBe(
        true,
      );
    }
  });

  it("disables conditional destinations when the selected provider hides them", () => {
    const localModel = textToSpeechSearchEntries.find(
      (entry) => entry.id === "tts-local-model",
    )!;
    const apiSettings = textToSpeechSearchEntries.find(
      (entry) => entry.id === "tts-api",
    )!;
    const settingsFor = (provider: string) =>
      ({ tts: { provider } }) as unknown as AppSettings;

    expect(localModel.isAvailable?.(settingsFor("openai"))).toBe(false);
    expect(localModel.isAvailable?.(settingsFor("local_qwen"))).toBe(true);
    expect(apiSettings.isAvailable?.(settingsFor("windows"))).toBe(false);
    expect(apiSettings.isAvailable?.(settingsFor("elevenlabs"))).toBe(true);
  });
});
