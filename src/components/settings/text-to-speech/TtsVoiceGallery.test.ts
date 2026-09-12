import { describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { TtsProvider } from "@/lib/tts/ttsProviderMetadata";

import {
  TTS_VOICE_GALLERY,
  TTS_VOICE_GALLERY_MANIFEST,
  synthesisConfigForGalleryVoice,
} from "./TtsVoiceGallery";

describe("TTS voice gallery", () => {
  test("preserves the destination model's saved non-voice settings", () => {
    const entry = TTS_VOICE_GALLERY.find((voice) => voice.id === "openai-marin")!;
    const current = {
      provider: "soniox" as TtsProvider,
      model: "tts-rt-v1",
      preprocessing_enabled: false,
      preprocessing_rules: [] as Array<{ from: string; to: string }>,
      target_chars: 200,
      retry_count: 3,
      retry_base_delay_ms: 500,
      inter_chunk_pause_ms: 0,
      paragraph_pause_ms: 0,
    };
    const remembered = {
      ...current,
      provider: "openai" as TtsProvider,
      model: "gpt-4o-mini-tts",
      voice: "cedar",
      speed: 1.25,
      voice_instructions: "Read slowly.",
      voice_prompt_preset_id: "slow",
      preprocessing_enabled: true,
      preprocessing_rules: [{ from: "Dr.", to: "Doctor" }],
      target_chars: 900,
      retry_count: 5,
      retry_base_delay_ms: 1_000,
      inter_chunk_pause_ms: 100,
      paragraph_pause_ms: 300,
    };
    const saved = [current, remembered];
    const before = structuredClone(saved);
    const applied = synthesisConfigForGalleryVoice(entry, current, saved);

    expect(applied).toMatchObject({
      provider: "openai",
      model: "gpt-4o-mini-tts",
      voice: "marin",
      speed: 1,
      voice_instructions: "Speak naturally and clearly.",
      voice_prompt_preset_id: "",
      preprocessing_enabled: true,
      preprocessing_rules: [{ from: "Dr.", to: "Doctor" }],
      target_chars: 900,
      retry_count: 5,
      retry_base_delay_ms: 1_000,
      inter_chunk_pause_ms: 100,
      paragraph_pause_ms: 300,
    });
    expect(saved).toEqual(before);
  });

  test("uses current non-voice settings when the destination model has no memory", () => {
    const entry = TTS_VOICE_GALLERY.find((voice) => voice.id === "murf-miles")!;
    const current = {
      provider: "openai" as TtsProvider,
      model: "gpt-4o-mini-tts",
      target_chars: 600,
      voice_instructions: "Whisper.",
    };
    const otherModel = {
      ...current,
      provider: "murf" as TtsProvider,
      model: "gen2",
      target_chars: 1_500,
    };

    const applied = synthesisConfigForGalleryVoice(entry, current, [otherModel]);
    expect(applied).toMatchObject({
      provider: "murf",
      model: "falcon-2",
      voice: "en-US-miles",
      language: "en-US",
      speed: 1,
      murf_rate: 0,
      murf_pitch: 0,
      murf_variation: 1,
      murf_style: "AIAgent",
      voice_instructions: "",
      voice_prompt_preset_id: "",
      target_chars: 600,
    });
  });

  test("keeps the intended representative provider mix", () => {
    const counts = TTS_VOICE_GALLERY.reduce<Record<string, number>>(
      (result, entry) => ({
        ...result,
        [entry.provider]: (result[entry.provider] ?? 0) + 1,
      }),
      {},
    );

    expect(counts).toEqual({
      soniox: 3,
      deepgram: 6,
      openai: 2,
      murf: 6,
      local_qwen: 1,
      local_kokoro: 1,
    });
    expect(new Set(TTS_VOICE_GALLERY.map((entry) => entry.id)).size).toBe(
      TTS_VOICE_GALLERY.length,
    );
  });

  test("ships one compact Ogg Opus preview for every card", () => {
    expect(TTS_VOICE_GALLERY_MANIFEST.schemaVersion).toBe(1);
    expect(TTS_VOICE_GALLERY_MANIFEST.previewEncoding).toEqual({
      container: "ogg",
      codec: "opus",
      bitrateKbps: 80,
    });

    for (const entry of TTS_VOICE_GALLERY) {
      const preview = readFileSync(
        join(process.cwd(), "public", entry.asset.path),
      );

      expect(preview.subarray(0, 4).toString()).toBe("OggS");
      expect(preview.includes(Buffer.from("OpusHead"))).toBe(true);
      expect(preview.byteLength).toBeLessThan(100_000);
      expect(preview.byteLength).toBe(entry.asset.bytes);
      expect(createHash("sha256").update(preview).digest("hex")).toBe(
        entry.asset.sha256,
      );
      expect(entry.transcript).toContain(entry.voiceName);
      expect(entry.transcript).toContain(entry.providerLabel);
    }
  });

  test("records a source and license policy for each provider", () => {
    for (const provider of Object.values(
      TTS_VOICE_GALLERY_MANIFEST.providers,
    )) {
      expect(provider.label.length).toBeGreaterThan(0);
      expect(provider.license.name.length).toBeGreaterThan(0);
      expect(provider.license.url).toMatch(/^https:\/\//);
      expect(provider.license.checkedOn).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    }
  });
});
