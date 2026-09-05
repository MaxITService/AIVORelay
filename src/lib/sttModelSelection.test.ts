import { describe, expect, it } from "bun:test";
import type { SttModelSelection } from "./sttModelSelection";
import {
  globalSttSelection,
  legacyLiveSttSelection,
  sttCatalog,
  sttModelDropdownOptions,
  sttModelCapabilities,
  sttSelectionKey,
  sttSupports,
} from "./sttModelSelection";

describe("workflow-specific STT model menus", () => {
  it("keeps Live Monitor limited to technically compatible live models", () => {
    const options = sttCatalog("live", []);

    expect(options.map(option => option.id)).toEqual([
      "soniox:stt-rt-v5",
      "deepgram:nova-3",
      "vercel:google/gemini-3.5-transcribe-live",
      "google:gemini-3.5-transcribe-live",
    ]);
    expect(options.every(option => option.capabilities.workflows.includes("live"))).toBe(true);
    expect(options.some(option => option.selection.provider === "local")).toBe(false);
  });

  it("keeps Transcribe File file-compatible and exposes route-specific Gemini controls", () => {
    const localModel = {
      id: "local-test-model",
      name: "Local Test Model",
    } as Parameters<typeof sttCatalog>[1][number];
    const optionIds = sttCatalog("file", [localModel]).map(option => option.id);

    expect(optionIds).toContain("local:local-test-model");
    expect(optionIds).toContain("soniox:stt-async-v5");
    expect(optionIds).toContain("vercel:google/gemini-3.5-transcribe");
    expect(optionIds).toContain("google:gemini-3.5-transcribe");
    expect(optionIds.some(id => id.includes("transcribe-live"))).toBe(false);

    const googleGemini: SttModelSelection = {
      provider: "remote_openai_compatible",
      provider_preset: "google",
      model_id: "gemini-3.5-transcribe",
    };
    const vercelGemini: SttModelSelection = {
      provider: "remote_openai_compatible",
      provider_preset: "vercel",
      model_id: "google/gemini-3.5-transcribe",
    };
    const liveGemini: SttModelSelection = {
      provider: "remote_openai_compatible",
      provider_preset: "vercel",
      model_id: "google/gemini-3.5-transcribe-live",
    };

    expect(sttSupports(googleGemini, "diarization", "file")).toBe(true);
    expect(sttSupports(vercelGemini, "diarization", "file")).toBe(false);
    expect(sttSupports(liveGemini, "languageHints", "live")).toBe(true);
    expect(sttSupports(liveGemini, "vocabulary", "live")).toBe(true);
    expect(sttModelCapabilities(liveGemini).workflows).not.toContain("file");
  });

  it("keeps unprepared compatible models selectable and explains their warning", () => {
    const catalog = sttCatalog("file", []);
    const unprepared = catalog.find(option => option.id === "google:gemini-3.5-transcribe")!;
    const reason = "API key is missing";
    const options = sttModelDropdownOptions(
      catalog,
      new Map([[sttSelectionKey(unprepared.selection), reason]]),
    );

    expect(options).toHaveLength(catalog.length);
    expect(options.find(option => option.value === sttSelectionKey(unprepared.selection))).toEqual({
      value: sttSelectionKey(unprepared.selection),
      label: `⚠ ${unprepared.modelLabel}`,
      className: "text-red-400",
      title: reason,
    });
    expect(
      options
        .find(option => option.value !== sttSelectionKey(unprepared.selection))
        ?.label.startsWith("⚠ "),
    ).toBe(false);
  });
});

describe("STT selection compatibility", () => {
  it("builds Dictation from the live and file catalogs without duplicate selections", () => {
    const localModel = {
      id: "local-test-model",
      name: "Local Test Model",
    } as Parameters<typeof sttCatalog>[1][number];
    const catalog = sttCatalog("dictation", [localModel]);
    const selectionKeys = catalog.map(option =>
      sttSelectionKey(option.selection),
    );

    expect(catalog[0].id).toBe("local:local-test-model");
    expect(catalog.map(option => option.id)).toContain("soniox:stt-rt-v5");
    expect(catalog.map(option => option.id)).toContain("soniox:stt-async-v5");
    expect(catalog.map(option => option.id)).toContain("deepgram:nova-3");
    expect(new Set(selectionKeys).size).toBe(selectionKeys.length);
  });

  it("restores the saved global provider and applies safe defaults for incomplete settings", () => {
    expect(globalSttSelection(undefined)).toEqual({
      provider: "local",
      model_id: "",
      provider_preset: "",
    });
    expect(
      globalSttSelection({
        transcription_provider: "remote_soniox",
        soniox_model: "stt-async-v5",
      } as Parameters<typeof globalSttSelection>[0]),
    ).toEqual({
      provider: "remote_soniox",
      model_id: "stt-async-v5",
      provider_preset: "",
    });
    expect(
      globalSttSelection({
        transcription_provider: "remote_openai_compatible",
        remote_stt: {},
      } as Parameters<typeof globalSttSelection>[0]),
    ).toEqual({
      provider: "remote_openai_compatible",
      model_id: "whisper-large-v3-turbo",
      provider_preset: "groq",
    });
  });

  it("keeps a compatible legacy Gemini live selection without silently switching batch models to Soniox", () => {
    const liveSettings = {
      live_sound_transcription_provider: "remote_openai_compatible",
      remote_stt: {
        provider_preset: "google",
        model_id: "gemini-3.5-transcribe-live",
      },
    } as Parameters<typeof legacyLiveSttSelection>[0];
    const fileSettings = {
      live_sound_transcription_provider: "remote_openai_compatible",
      remote_stt: {
        provider_preset: "google",
        model_id: "gemini-3.5-transcribe",
      },
    } as Parameters<typeof legacyLiveSttSelection>[0];

    expect(legacyLiveSttSelection(liveSettings)).toEqual({
      provider: "remote_openai_compatible",
      model_id: "gemini-3.5-transcribe-live",
      provider_preset: "google",
    });
    expect(legacyLiveSttSelection(fileSettings)).toBeNull();
  });

  it("grants Gemini capabilities only to exact backend-supported preset and model pairs", () => {
    const googleGemini: SttModelSelection = {
      provider: "remote_openai_compatible",
      provider_preset: "google",
      model_id: "gemini-3.5-transcribe",
    };
    const customGemini: SttModelSelection = {
      provider: "remote_openai_compatible",
      provider_preset: "custom",
      model_id: "gemini-3.5-transcribe",
    };
    const suffixedLiveGemini: SttModelSelection = {
      provider: "remote_openai_compatible",
      provider_preset: "google",
      model_id: "gemini-3.5-transcribe-live-custom",
    };

    expect(sttSupports(googleGemini, "diarization", "file")).toBe(true);
    expect(sttSupports(googleGemini, "vocabulary", "dictation")).toBe(true);
    expect(sttSupports(customGemini, "diarization", "file")).toBe(false);
    expect(sttSupports(customGemini, "vocabulary", "dictation")).toBe(false);
    expect(sttSupports(customGemini, "languageHints", "file")).toBe(true);
    expect(sttModelCapabilities(suffixedLiveGemini).workflows).not.toContain("live");
  });
});
