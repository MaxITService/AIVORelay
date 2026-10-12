import { describe, expect, test } from "bun:test";
import type { ExtendedTranscriptionProfile } from "./ProfileSettingsForm";

import { cloneProfileSettings, profileSettingsPayload } from "./profileSettingsPayload";

const profile = (): ExtendedTranscriptionProfile => ({
  id: "saved", name: "Custom", language: "auto", translate_to_english: false,
  push_to_talk: false, include_in_cycle: false, auto_shortcut_activation: false,
  preview_output_only_enabled: false, stt_prompt_override_enabled: false,
  system_prompt: "", stt_model_selection_override: null,
  llm_post_process_enabled: false, llm_prompt_override: "", llm_model_override: "",
  automatic_app_rules: ["editor.exe"], soniox_context_general_json: "",
  soniox_context_text: "", soniox_context_terms: ["AivoRelay"], soniox_language_hints_strict: null,
  gemini_language_code_override: "", gemini_custom_vocabulary_override: [], chinese_script: null,
  output_settings: { paste_method: null, clipboard_history_allowed: false,
    auto_submit: false, before_insert_key: "", before_insert_delay_ms: 0, check_active_window: null },
});

describe("profile clone and command payload", () => {
  test("JSON serialization retains explicit off, empty overrides, zero delays and per-field inheritance", () => {
    const payload = JSON.parse(JSON.stringify(profileSettingsPayload(profile())));
    expect(payload).toEqual({
      language: "auto", translateToEnglish: false, pushToTalk: false, includeInCycle: false,
      autoShortcutActivation: false, previewOutputOnlyEnabled: false, sttPromptOverrideEnabled: false,
      automaticAppRules: ["editor.exe"], systemPrompt: "", sttModelSelectionOverride: null,
      llmSettings: { enabled: false, promptOverride: "", modelOverride: "" },
      sonioxContextGeneralJson: "", sonioxContextText: "", sonioxContextTerms: ["AivoRelay"],
      sonioxLanguageHintsStrict: null, geminiLanguageCodeOverride: "",
      geminiCustomVocabularyOverride: [], chineseScript: null,
      outputSettings: { paste_method: null, clipboard_history_allowed: false,
        auto_submit: false, before_insert_key: "", before_insert_delay_ms: 0, check_active_window: null },
    });
  });

  test("duplicate edits do not mutate the saved profile or its nested settings", () => {
    const source = { ...profile(), stt_model_selection_override: {
      provider: "local" as const, provider_preset: "", model_id: "model-a",
    }, gemini_custom_vocabulary_override: ["Original"] };
    const copy = cloneProfileSettings(source);
    expect(copy).not.toHaveProperty("id");
    expect(copy).not.toHaveProperty("name");
    expect(profileSettingsPayload({ ...source, ...copy })).toEqual(profileSettingsPayload(source));
    copy.automatic_app_rules!.push("browser.exe");
    copy.soniox_context_terms!.push("Copy");
    copy.gemini_custom_vocabulary_override!.push("Copy");
    copy.stt_model_selection_override!.model_id = "model-b";
    copy.output_settings!.auto_submit = true;
    expect(source).toEqual({ ...profile(), stt_model_selection_override: {
      provider: "local", provider_preset: "", model_id: "model-a",
    }, gemini_custom_vocabulary_override: ["Original"] });
  });

  test("Default clone and payload retain nullable STT/LLM inheritance", () => {
    const source = { ...profile(), id: "default", llm_prompt_override: null, llm_model_override: null };
    const copy = { ...source, ...cloneProfileSettings(source), id: "new", name: "Default copy", include_in_cycle: true };
    const payload = JSON.parse(JSON.stringify(profileSettingsPayload(copy)));
    expect(payload.sttModelSelectionOverride).toBeNull();
    expect(payload.llmSettings).toEqual({ enabled: false, promptOverride: null, modelOverride: null });
    expect(payload.sonioxLanguageHintsStrict).toBeNull();
    expect(payload.outputSettings).toEqual(source.output_settings);
    expect(payload.includeInCycle).toBe(true);
  });
});
