import { expect, test } from "bun:test";
import { getProfileSpokenLanguageState } from "./profileSpokenLanguage";

const model = {
  engine_type: "TranscribeCpp" as const,
  supported_languages: ["ru", "en", "de"],
  supports_language_detection: false,
};
const profile = {
  language: "auto", spoken_languages: ["ru", "en"],
  language_detection: "when_needed" as const,
};

test("when-needed preferences follow the model, without rewriting saved choices", () => {
  expect(getProfileSpokenLanguageState(profile, model)).toMatchObject({
    selected: true, summaryKey: "autoSummary", compatible: ["ru", "en"],
  });
  expect(getProfileSpokenLanguageState(profile, { ...model, supports_language_detection: true })).toMatchObject({
    selected: false, summaryKey: "modelAuto",
  });
  expect(getProfileSpokenLanguageState({ ...profile, language_detection: "always" }, {
    ...model, supports_language_detection: true,
  }).selected).toBe(true);
  expect(getProfileSpokenLanguageState(profile, model).selected).toBe(true);
  expect(profile.spoken_languages).toEqual(["ru", "en"]);
});

test("incompatible saved languages are distinguished from an empty selection", () => {
  expect(getProfileSpokenLanguageState({ ...profile, spoken_languages: ["ja", "fr"] }, model).summaryKey).toBe("noCompatible");
  expect(getProfileSpokenLanguageState({ ...profile, spoken_languages: [], language_detection: "always" }, model).summaryKey).toBe("chooseLanguages");
  expect(getProfileSpokenLanguageState({ ...profile, spoken_languages: ["ru", "ja"] }, model)).toMatchObject({
    summaryKey: "singleCompatible", summaryLanguage: "ru",
  });
});

test("monolingual aliases and unknown capabilities do not promise detection", () => {
  for (const supported_languages of [["en"], ["he", "iw"], ["zh", "zh-Hans", "zh-Hant"]]) {
    expect(getProfileSpokenLanguageState(profile, { ...model, supported_languages })).toMatchObject({
      canUsePreferences: false, selected: false, summaryKey: "singleModelLanguage",
    });
  }
  expect(getProfileSpokenLanguageState(profile, { ...model, supported_languages: [] })).toMatchObject({
    canUsePreferences: false, selected: false, summaryKey: "unknownModelLanguages", compatible: [],
  });
  expect(getProfileSpokenLanguageState(profile)).toMatchObject({ canUsePreferences: false, selected: false });
});

test("language aliases match across capability and preference codes without duplicate detection choices", () => {
  expect(getProfileSpokenLanguageState({ ...profile, spoken_languages: ["iw", "he", "jw"] }, {
    ...model, supported_languages: ["he", "jv"],
  })).toMatchObject({ selected: true, compatible: ["iw", "jw"], summaryKey: "autoSummary" });
  expect(getProfileSpokenLanguageState({ ...profile, language: "en" }, model).selected).toBe(false);
});

test("native streaming with built-in language detection uses model Auto while keeping saved preferences", () => {
  const streamingModel = {
    ...model,
    id: "nemotron-3.5-asr-streaming-0.6b",
    supports_language_detection: true,
    supports_streaming: true,
  };
  const alwaysSingle = {
    language: "auto",
    spoken_languages: ["en"],
    language_detection: "always" as const,
  };
  // Streaming capability alone does not enable native streaming without live output or Preview.
  expect(getProfileSpokenLanguageState(alwaysSingle, streamingModel, {
    preview_output_only_enabled: false,
    native_streaming_live_output_models: [],
  })).toMatchObject({
    canUsePreferences: true, selected: true, summaryKey: "singleCompatible", summaryLanguage: "en",
  });
  // Direct live output enables native streaming -> model native Auto ignores preferences.
  expect(getProfileSpokenLanguageState(alwaysSingle, streamingModel, {
    preview_output_only_enabled: false,
    native_streaming_live_output_models: [streamingModel.id],
  })).toMatchObject({
    canUsePreferences: false, selected: false, summaryKey: "nativeStreamingAuto", compatible: ["en"],
  });
  // Profile or global Preview enables native streaming -> model native Auto ignores preferences.
  expect(getProfileSpokenLanguageState({ ...alwaysSingle, preview_output_only_enabled: true }, streamingModel, {
    preview_output_only_enabled: false,
    native_streaming_live_output_models: [],
  })).toMatchObject({
    canUsePreferences: false, selected: false, summaryKey: "nativeStreamingAuto", compatible: ["en"],
  });
  // Moonshine streaming ignores native_streaming_live_output_models unless Preview is active.
  const moonshineMultilingual = {
    ...streamingModel,
    id: "handy-computer/moonshine-streaming-tiny-gguf",
  };
  expect(getProfileSpokenLanguageState(alwaysSingle, moonshineMultilingual, {
    preview_output_only_enabled: false,
    native_streaming_live_output_models: [moonshineMultilingual.id],
  })).toMatchObject({
    canUsePreferences: true, selected: true, summaryKey: "singleCompatible",
  });
  expect(alwaysSingle.spoken_languages).toEqual(["en"]);
});
