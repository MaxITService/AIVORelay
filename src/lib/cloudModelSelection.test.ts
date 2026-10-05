import { expect, test } from "bun:test";
import type { AppSettings } from "@/bindings";
import {
  globalSttSelection,
  isGeminiLiveSelection,
  legacyLiveSttSelection,
  sttCatalog,
  sttModelCapabilities,
  sttModelDropdownOptions,
  sttProviderId,
  sttSelectionKey,
  sttSupports,
  type SttModelSelection,
} from "./sttModelSelection";

const settings = (overrides: Partial<AppSettings>): AppSettings =>
  overrides as AppSettings;

const compatible = (preset: string, model: string): SttModelSelection => ({
  provider: "remote_openai_compatible",
  provider_preset: preset,
  model_id: model,
});

test("missing Soniox model restores the default realtime cloud model", () => {
  expect(globalSttSelection(settings({ transcription_provider: "remote_soniox" })))
    .toEqual({ provider: "remote_soniox", model_id: "stt-rt-v5", provider_preset: "" });
});

test("global Deepgram selection preserves the user's saved model", () => {
  expect(globalSttSelection(settings({
    transcription_provider: "remote_deepgram",
    deepgram_model: "nova-2",
    soniox_model: "stt-rt-v5",
  }))).toEqual({ provider: "remote_deepgram", model_id: "nova-2", provider_preset: "" });
});

test("empty Deepgram model restores the Nova 3 default", () => {
  expect(globalSttSelection(settings({
    transcription_provider: "remote_deepgram",
    deepgram_model: "",
  }))).toEqual({ provider: "remote_deepgram", model_id: "nova-3", provider_preset: "" });
});

test("custom API selection preserves its model instead of using unrelated provider settings", () => {
  expect(globalSttSelection(settings({
    transcription_provider: "remote_openai_compatible",
    remote_stt: {
      provider_preset: "custom",
      model_id: "my-transcriber",
      base_url: "https://speech.example.test/v1",
    },
    selected_model: "local-model",
    soniox_model: "stt-rt-v5",
  }))).toEqual(compatible("custom", "my-transcriber"));
});

test("dedicated cloud provider IDs ignore a leftover compatible API preset", () => {
  expect(sttProviderId({ provider: "remote_soniox", provider_preset: "openai" })).toBe("soniox");
  expect(sttProviderId({ provider: "remote_deepgram", provider_preset: "google" })).toBe("deepgram");
  expect(sttProviderId({ provider: "remote_openai_compatible" })).toBe("custom");
});

test("legacy system live selection restores Soniox independently of global dictation", () => {
  expect(legacyLiveSttSelection(settings({
    live_sound_transcription_provider: "system",
    transcription_provider: "remote_deepgram",
    deepgram_model: "nova-2",
    soniox_model: "stt-rt-v5",
  }))).toEqual({ provider: "remote_soniox", model_id: "stt-rt-v5", provider_preset: "" });
});

test("legacy Deepgram live selection falls back when its model is missing", () => {
  expect(legacyLiveSttSelection(settings({
    live_sound_transcription_provider: "remote_deepgram",
  }))).toEqual({ provider: "remote_deepgram", model_id: "nova-3", provider_preset: "" });
});

test("legacy Deepgram live selection retains its saved model", () => {
  expect(legacyLiveSttSelection(settings({
    live_sound_transcription_provider: "remote_deepgram",
    deepgram_model: "nova-2",
  }))).toEqual({ provider: "remote_deepgram", model_id: "nova-2", provider_preset: "" });
});

test("legacy Vercel live selection retains the gateway-specific Gemini model ID", () => {
  expect(legacyLiveSttSelection(settings({
    live_sound_transcription_provider: "remote_openai_compatible",
    remote_stt: {
      provider_preset: "vercel",
      model_id: "google/gemini-3.5-transcribe-live",
      base_url: "https://gateway.example.test/v4/ai",
    },
  }))).toEqual(compatible("vercel", "google/gemini-3.5-transcribe-live"));
});

test("legacy live monitor does not silently migrate OpenAI dictation to another provider", () => {
  expect(legacyLiveSttSelection(settings({
    live_sound_transcription_provider: "remote_openai_compatible",
    remote_stt: {
      provider_preset: "openai",
      model_id: "gpt-live-transcribe",
      base_url: "https://speech.example.test/v1",
    },
  }))).toBeNull();
});

test("selection keys distinguish cloud routes even when their model IDs match", () => {
  expect(sttSelectionKey(compatible("google", "shared-model")))
    .not.toBe(sttSelectionKey(compatible("vercel", "shared-model")));
  expect(sttSelectionKey({ provider: "remote_soniox", model_id: "shared-model" }))
    .not.toBe(sttSelectionKey({ provider: "remote_deepgram", model_id: "shared-model" }));
});

test("ready cloud models have clean dropdown labels and no error styling", () => {
  const catalog = sttCatalog("file", []);
  const readiness = new Map(catalog.map(option => [sttSelectionKey(option.selection), null]));
  expect(sttModelDropdownOptions(catalog, readiness)).toEqual(catalog.map(option => ({
    value: sttSelectionKey(option.selection),
    label: option.modelLabel,
    sortLabel: option.modelLabel,
    className: undefined,
    title: undefined,
  })));
});

test("readiness warnings stay isolated to the selected API route", () => {
  const template = sttCatalog("file", []).find(option => option.providerId === "google")!;
  const google = { ...template, selection: compatible("google", "shared-model") };
  const vercel = { ...template, selection: compatible("vercel", "shared-model") };
  const options = sttModelDropdownOptions([google, vercel], new Map([
    [sttSelectionKey(google.selection), "Google API key is missing"],
  ]));
  expect(options[0].title).toBe("Google API key is missing");
  expect(options[1].title).toBeUndefined();
  expect(options[1].label).toBe(vercel.modelLabel);
  expect(options[1].className).toBeUndefined();
});

test("warning rendering does not mutate cloud catalogs or readiness results", () => {
  const catalog = sttCatalog("file", []);
  const snapshot = JSON.stringify(catalog);
  const key = sttSelectionKey(catalog[0].selection);
  const readiness = new Map([[key, "API key is missing"]]);
  sttModelDropdownOptions(catalog, readiness);
  expect(JSON.stringify(catalog)).toBe(snapshot);
  expect([...readiness]).toEqual([[key, "API key is missing"]]);
});

test("cloud catalog capabilities agree with capabilities resolved from each selection", () => {
  for (const workflow of ["dictation", "file", "live"] as const) {
    for (const option of sttCatalog(workflow, [])) {
      expect(option.capabilities).toEqual(sttModelCapabilities(option.selection));
      expect(option.capabilities.workflows).toContain(workflow);
      expect(sttProviderId(option.selection)).toBe(option.providerId);
    }
  }
});

test("Soniox exposes live diarization but reserves timestamps and language hints for supported workflows", () => {
  const selection: SttModelSelection = { provider: "remote_soniox", model_id: "stt-rt-v5" };
  expect(sttSupports(selection, "diarization", "live")).toBe(true);
  expect(sttSupports(selection, "timestamps", "file")).toBe(true);
  expect(sttSupports(selection, "timestamps", "live")).toBe(false);
  expect(sttSupports(selection, "languageHints", "live")).toBe(false);
  expect(sttSupports(selection, "vocabulary", "file")).toBe(false);
});

test("Deepgram exposes file language hints and live diarization without generic chunking controls", () => {
  const selection: SttModelSelection = { provider: "remote_deepgram", model_id: "nova-3" };
  expect(sttSupports(selection, "languageHints", "dictation")).toBe(true);
  expect(sttSupports(selection, "languageHints", "file")).toBe(true);
  expect(sttSupports(selection, "diarization", "live")).toBe(true);
  expect(sttSupports(selection, "timestamps", "live")).toBe(false);
  expect(sttSupports(selection, "chunking", "file")).toBe(false);
});

test("Gemini live routes do not inherit batch diarization, timestamps, or chunking controls", () => {
  for (const selection of [
    compatible("google", "gemini-3.5-transcribe-live"),
    compatible("vercel", "google/gemini-3.5-transcribe-live"),
  ]) {
    expect(isGeminiLiveSelection(selection)).toBe(true);
    expect(sttSupports(selection, "diarization", "live")).toBe(false);
    expect(sttSupports(selection, "timestamps", "live")).toBe(false);
    expect(sttSupports(selection, "chunking", "file")).toBe(false);
    expect(sttModelCapabilities(selection).workflows).not.toContain("file");
  }
});

test("crossed Google and Vercel model IDs do not acquire Gemini-specific capabilities", () => {
  for (const selection of [
    compatible("google", "google/gemini-3.5-transcribe"),
    compatible("vercel", "gemini-3.5-transcribe"),
    compatible("google", "google/gemini-3.5-transcribe-live"),
    compatible("vercel", "gemini-3.5-transcribe-live"),
  ]) {
    expect(isGeminiLiveSelection(selection)).toBe(false);
    expect(sttSupports(selection, "vocabulary", "file")).toBe(false);
    expect(sttSupports(selection, "diarization", "file")).toBe(false);
    expect(sttSupports(selection, "timestamps", "file")).toBe(false);
    expect(sttModelCapabilities(selection).workflows).not.toContain("live");
  }
});

test("generic OpenAI, Groq, and custom models keep file language hints without unsupported Gemini controls", () => {
  for (const selection of [
    compatible("openai", "gpt-transcribe"),
    compatible("groq", "whisper-large-v3-turbo"),
    compatible("custom", "my-transcriber"),
  ]) {
    expect(sttSupports(selection, "languageHints", "file")).toBe(true);
    expect(sttSupports(selection, "vocabulary", "file")).toBe(false);
    expect(sttSupports(selection, "diarization", "file")).toBe(false);
    expect(sttSupports(selection, "timestamps", "file")).toBe(false);
    expect(sttModelCapabilities(selection).workflows).not.toContain("live");
  }
});
