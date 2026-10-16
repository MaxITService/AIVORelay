import type { ExtendedTranscriptionProfile } from "./ProfileSettingsForm";

export const cloneProfileSettings = (profile: ExtendedTranscriptionProfile): Partial<ExtendedTranscriptionProfile> => {
  const { id: _id, name: _name, ...values } = profile;
  return { ...values, automatic_app_rules: [...(profile.automatic_app_rules ?? [])],
    soniox_context_terms: [...(profile.soniox_context_terms ?? [])],
    gemini_custom_vocabulary_override: profile.gemini_custom_vocabulary_override == null ? null : [...profile.gemini_custom_vocabulary_override],
    stt_model_selection_override: profile.stt_model_selection_override ? { ...profile.stt_model_selection_override } : null,
    output_settings: { ...profile.output_settings },
  };
};

/** A blank profile LLM prompt is not an override: post-processing then uses the global prompt. */
export const llmPromptOverrideFromInput = (prompt: string | null | undefined): string | null =>
  prompt?.trim() ? prompt : null;

export const profileSettingsPayload = (profile: ExtendedTranscriptionProfile) => ({
  language: profile.language, translateToEnglish: profile.translate_to_english,
  automaticAppRules: profile.automatic_app_rules ?? [], systemPrompt: profile.system_prompt ?? "",
  sttPromptOverrideEnabled: profile.stt_prompt_override_enabled ?? false,
  sttModelSelectionOverride: profile.stt_model_selection_override ?? null,
  pushToTalk: profile.push_to_talk, autoShortcutActivation: profile.auto_shortcut_activation ?? false,
  previewOutputOnlyEnabled: profile.preview_output_only_enabled ?? false, includeInCycle: profile.include_in_cycle,
  llmSettings: { enabled: profile.llm_post_process_enabled ?? false,
    promptOverride: profile.llm_prompt_override ?? null, modelOverride: profile.llm_model_override ?? null,
    liveStreamInsert: profile.llm_live_stream_insert ?? null },
  sonioxContextGeneralJson: profile.soniox_context_general_json ?? "", sonioxContextText: profile.soniox_context_text ?? "",
  sonioxContextTerms: profile.soniox_context_terms ?? [], sonioxLanguageHintsStrict: profile.soniox_language_hints_strict ?? null,
  geminiLanguageCodeOverride: profile.gemini_language_code_override ?? null,
  geminiCustomVocabularyOverride: profile.gemini_custom_vocabulary_override ?? null,
  chineseScript: profile.chinese_script ?? null, outputSettings: profile.output_settings ?? {},
});

/** update_transcription_profile payload; Chinese script and output settings are saved by their own commands. */
export const profileUpdatePayload = (
  profile: ExtendedTranscriptionProfile, patch: Partial<ExtendedTranscriptionProfile> = {},
) => {
  const next = { ...profile, ...patch };
  // Every field is sent: the command stores omitted overrides as cleared.
  const { chineseScript: _chineseScript, outputSettings: _outputSettings, ...settings } = profileSettingsPayload(next);
  return { id: next.id, name: next.name, ...settings };
};
