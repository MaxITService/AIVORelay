import { expect, test } from "bun:test";
import type { AppSettings, SttModelSelection } from "@/bindings";
import { getPostProcessingAvailability } from "./postProcessingAvailability";

const settings = (overrides: Partial<AppSettings>): AppSettings =>
  overrides as AppSettings;

const remote = (preset: string, model: string): AppSettings => settings({
  transcription_provider: "remote_openai_compatible",
  remote_stt: {
    provider_preset: preset,
    model_id: model,
    base_url: "https://speech.example.test/v1",
  },
});

const allowed = { available: true, reason: null };
const directOutput = { available: false, reason: "direct_realtime_output" };

test("OpenAI direct live transcription reports why post-processing is unavailable", () => {
  for (const model of ["gpt-live-transcribe", "gpt-realtime-whisper"]) {
    expect(getPostProcessingAvailability(remote("openai", model))).toEqual(directOutput);
  }
});

test("flattening OpenAI realtime output allows complete-transcript post-processing", () => {
  for (const model of ["gpt-live-transcribe", "gpt-realtime-whisper"]) {
    expect(getPostProcessingAvailability({
      ...remote("openai", model),
      openai_realtime_whisper_flatten_enabled: true,
    })).toEqual(allowed);
  }
});

test("OpenAI realtime model detection tolerates saved model capitalization", () => {
  expect(getPostProcessingAvailability(remote("openai", "GPT-LIVE-TRANSCRIBE")))
    .toEqual(directOutput);
});

test("custom endpoints using an OpenAI realtime model name retain generic post-processing", () => {
  expect(getPostProcessingAvailability(remote("custom", "gpt-live-transcribe")))
    .toEqual(allowed);
});

test("Soniox live preview keeps a cloud realtime transcript reversible", () => {
  expect(getPostProcessingAvailability(settings({
    transcription_provider: "remote_soniox",
    soniox_model: "stt-rt-v5",
    soniox_live_enabled: true,
    soniox_live_preview_enabled: true,
  }))).toEqual(allowed);
});

test("Deepgram live output is blocked only while it writes directly to the target", () => {
  const current = settings({
    transcription_provider: "remote_deepgram",
    deepgram_model: "nova-3",
    deepgram_live_enabled: true,
  });
  expect(getPostProcessingAvailability(current)).toEqual(directOutput);
  expect(getPostProcessingAvailability({ ...current, preview_output_only_enabled: true }))
    .toEqual(allowed);
  expect(getPostProcessingAvailability({ ...current, deepgram_live_enabled: false }))
    .toEqual(allowed);
});

test("an empty Deepgram model cannot be mistaken for active direct streaming", () => {
  expect(getPostProcessingAvailability(settings({
    transcription_provider: "remote_deepgram",
    deepgram_model: " \t ",
    deepgram_live_enabled: true,
  }))).toEqual(allowed);
});

test("a cloud model override determines output safety instead of the saved global model", () => {
  const live: SttModelSelection = {
    provider: "remote_openai_compatible",
    provider_preset: "google",
    model_id: "gemini-3.5-transcribe-live",
  };
  expect(getPostProcessingAvailability(remote("groq", "whisper-large-v3-turbo"), {
    sttSelection: live,
  })).toEqual(directOutput);
  expect(getPostProcessingAvailability(remote("google", "gemini-3.5-transcribe-live"), {
    sttSelection: { ...live, model_id: "gemini-3.5-transcribe" },
  })).toEqual(allowed);
});

test("an explicit profile preview override can disable a saved global preview route", () => {
  const current = { ...remote("google", "gemini-3.5-transcribe-live"), preview_output_only_enabled: true };
  expect(getPostProcessingAvailability(current)).toEqual(allowed);
  expect(getPostProcessingAvailability(current, { profilePreviewOutputOnlyEnabled: false }))
    .toEqual(directOutput);
});

test("Gemini live output honors the active custom profile's preview setting", () => {
  const current = settings({
    ...remote("vercel", "google/gemini-3.5-transcribe-live"),
    active_profile_id: "preview-profile",
    preview_output_only_enabled: false,
    transcription_profiles: [{
      id: "preview-profile",
      preview_output_only_enabled: true,
    } as NonNullable<AppSettings["transcription_profiles"]>[number]],
  });
  expect(getPostProcessingAvailability(current)).toEqual(allowed);
  expect(getPostProcessingAvailability(current, { profilePreviewOutputOnlyEnabled: false }))
    .toEqual(directOutput);
});
