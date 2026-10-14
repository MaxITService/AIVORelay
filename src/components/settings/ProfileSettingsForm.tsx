import React, { useState, useMemo, useEffect, useRef } from "react";
import { Trans, useTranslation } from "react-i18next";
import { ChevronDown, RefreshCcw, RotateCcw } from "lucide-react";
import { commands, type TranscriptionProfile, type ModelInfo } from "@/bindings";
import { sessionToast as toast } from "@/lib/sessionToast";
import { Button } from "../ui/Button";
import { Textarea } from "../ui/Textarea";
import { TextFileActions } from "../ui/TextFileActions";
import { NameSortedDropdown } from "../ui/NameSortedDropdown";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { TriStateToggle, type TriStateValue } from "../ui/TriStateToggle";
import { Tooltip } from "../ui/Tooltip";
import { Collapse } from "../ui/Collapse";
import { WindowDetails } from "../ui/WindowDetails";
import { useWindowState } from "../../hooks/useWindowState";
import { ModelSelect } from "./PostProcessingSettingsApi/ModelSelect";
import { ResetButton } from "../ui/ResetButton";
import { InfoTooltip } from "../ui/InfoTooltip";
import { SonioxContextEditor, isSonioxContextValid } from "./SonioxContextEditor";
import type { ModelOption } from "./PostProcessingSettingsApi/types";
import type { useSettings } from "../../hooks/useSettings";
import { LANGUAGES, supportsLanguageCode } from "../../lib/constants/languages";
import { isLanguageSupportedBySoniox } from "../../lib/constants/sonioxLanguages";
import { getModelPromptInfo } from "./TranscriptionSystemPrompt";
import { useNavigationStore } from "../../stores/navigationStore";
import { getPostProcessingAvailability } from "../../lib/postProcessingAvailability";
import { GEMINI_LOCALES } from "../../lib/gemini/geminiConfig";
import { GEMINI_VOCABULARY_FILE_EXTENSIONS, parseGeminiVocabulary, validateGeminiVocabularyFile } from "../../lib/gemini/vocabulary";
import { ChineseScriptFields } from "./ChineseScript";
import { SttModelSelector } from "./SttModelSelector";
import { KeyActionsBadge, ProfileOutputSettingsFields, getGlobalProfileOutputDefaults, profileKeyActionsEnabled, profileOutputDefaults } from "./ProfileOutputSettings";
import type { ProfileOutputSettings as OutputSettings } from "@/lib/profileOutputSettings";
import type { SttModelSelection } from "../../lib/sttModelSelection";
import { navigateToSettingsAnchor } from "../../lib/anchorNavigation";
import { openLivePreviewSettings } from "../../lib/transcriptionProfileNavigation";

/** Open the shared provider, model, key, and prompt configuration. */
const openPostProcessingSettings = () => {
  useNavigationStore.getState().setSection("postprocessing");
};

const ProfileMoreSettings: React.FC<{
  profileId: string;
  previewEnabled: boolean;
  keyActionsEnabled: boolean;
  children: React.ReactNode;
}> = ({ profileId, previewEnabled, keyActionsEnabled, children }) => {
  const { t } = useTranslation();

  return (
    <WindowDetails
      stateKey={`profile-${profileId}-more`}
      id={`profile-more-${profileId}`}
      className="group/more rounded-lg border border-mid-gray/20 bg-mid-gray/5 transition-colors open:border-purple-500/30 open:bg-purple-500/5"
    >
      <summary className="flex items-center gap-2 px-3 py-2.5 cursor-pointer select-none list-none [&::-webkit-details-marker]:hidden hover:bg-mid-gray/10 transition-colors">
        <ChevronDown className="w-3.5 h-3.5 text-mid-gray shrink-0 transition-transform group-open/more:rotate-180" />
        <span className="text-xs font-semibold text-text/70">
          {t("profileOutput.profileMore")}
        </span>
        {(previewEnabled || keyActionsEnabled) && <span className="ml-auto flex min-w-0 flex-wrap items-center justify-end gap-2">
          {keyActionsEnabled && <KeyActionsBadge />}
          {previewEnabled && <span className="shrink-0 text-[10px] px-1.5 py-0.5 rounded-full bg-purple-500/20 text-purple-400 border border-purple-500/30 whitespace-nowrap group-open/more:hidden">
            {t("profileOutput.previewEnabled")}
          </span>}
        </span>}
      </summary>
      <div className="px-3 py-3 space-y-3 border-t border-mid-gray/10">
        {children}
      </div>
    </WindowDetails>
  );
};

export const profileSttPresentation = (
  selection: SttModelSelection,
  localModels: ModelInfo[],
) => {
  const provider = selection.provider;
  const modelId = selection.model_id ?? "";
  const isSonioxProvider = provider === "remote_soniox";
  const isGeminiProvider =
    provider === "remote_openai_compatible" &&
    modelId.toLowerCase().includes("gemini-3.5-transcribe");
  const localModel =
    provider === "local"
      ? localModels.find((model) => model.id === modelId)
      : undefined;
  const promptInfo =
    provider === "remote_soniox" || provider === "remote_deepgram"
      ? { supportsPrompt: false, charLimit: 0 }
      : provider === "remote_openai_compatible"
        ? getModelPromptInfo(modelId, undefined, false)
        : getModelPromptInfo(modelId, localModel?.engine_type);
  const languageOptions = isSonioxProvider
    ? LANGUAGES.filter((language) =>
        isLanguageSupportedBySoniox(language.value),
      )
    : provider === "local" && localModel?.supported_languages?.length
      ? LANGUAGES.filter(
          (language) =>
            language.value === "auto" ||
            language.value === "os_input" ||
            supportsLanguageCode(
              localModel.supported_languages,
              language.value,
            ),
        )
      : LANGUAGES;

  return {
    isGeminiProvider,
    isSonioxProvider,
    languageOptions,
    promptLimit: promptInfo.supportsPrompt ? promptInfo.charLimit : 0,
    supportsSttPrompt: promptInfo.supportsPrompt,
  };
};

export const useProfileSttCapabilities = (selection: SttModelSelection, localModels: ModelInfo[]) => {
  const [remoteSupportsTranslation, setRemoteSupportsTranslation] =
    useState(false);
  useEffect(() => {
    let cancelled = false;
    setRemoteSupportsTranslation(false);
    if (selection.provider !== "remote_openai_compatible") {
      return;
    }

    void commands
      .remoteSttModelSupportsTranslation(selection.model_id ?? "")
      .then((supported) => {
        if (!cancelled) setRemoteSupportsTranslation(supported);
      })
      .catch(() => {
        if (!cancelled) setRemoteSupportsTranslation(false);
      });

    return () => {
      cancelled = true;
    };
  }, [selection.model_id, selection.provider]);
  const supportsTranslation =
    selection.provider === "local"
      ? (localModels.find(
          (model) => model.id === selection.model_id,
        )?.supports_translation ?? true)
      : selection.provider === "remote_openai_compatible"
        ? remoteSupportsTranslation
        : false;
  return { ...profileSttPresentation(selection, localModels), supportsTranslation };
};

export const strictModeFromApi = (
  value: boolean | null | undefined,
): TriStateValue => {
  if (value === true) return "on";
  if (value === false) return "off";
  return "global";
};

export const strictModeToApi = (value: TriStateValue): boolean | null => {
  if (value === "on") return true;
  if (value === "off") return false;
  return null;
};

export const DEFAULT_CLEAN_PROMPT = `Clean this Speech to text transcript:
1. Fix spelling, capitalization, and punctuation errors
2. Convert number words to digits (twenty-five → 25, ten percent → 10%, five dollars → $5)
3. Replace spoken punctuation with symbols (period → ., comma → , question mark → ?)
4. REPLACE "ENTER" WITH NEWLINE SYMBOL!!!!
5. Keep the language in the original version (if it was french, keep it in french for example) unless last sentence is "Translate this to <language>". Then - do this: CUT OUT THE LAST SENTENCE (the one wiht instruction), and translate the text into target language.\x20

Preserve exact meaning and word order. Do not paraphrase or reorder content.

Return only the cleaned transcript.

Transcript:
\${output}`;

export type ExtendedTranscriptionProfile = Omit<TranscriptionProfile, "output_settings"> & {
  output_settings?: OutputSettings;
  automatic_app_rules: string[];
  include_in_cycle: boolean;
  push_to_talk: boolean;
  auto_shortcut_activation?: boolean;
  preview_output_only_enabled: boolean;
  soniox_language_hints_strict?: boolean | null;
  stt_prompt_override_enabled: boolean;
  stt_model_selection_override?: SttModelSelection | null;
  soniox_context_general_json: string;
  soniox_context_text: string;
  soniox_context_terms: string[];
  gemini_language_code_override?: string | null;
  gemini_custom_vocabulary_override?: string[] | null;
};

const geminiVocabularyText = (terms: string[]) => {
  const lines = terms.join("\n");
  const parsed = parseGeminiVocabulary(lines);
  return parsed.safeToPersist && parsed.normalizedTerms.length === terms.length &&
    parsed.normalizedTerms.every((term, index) => term === terms[index])
      ? lines : JSON.stringify(terms, null, 2);
};

export interface LlmPromptSourceOption {
  value: string;
  label: string;
  prompt: string;
}

export type ProfileValidationReason = "system_prompt" | "gemini_vocabulary" | "soniox_context" | "output_delay";

export interface ProfileSettingsFormProps {
  profile: ExtendedTranscriptionProfile;
  scope: "saved" | "draft" | "global";
  settings: ReturnType<typeof useSettings>["settings"];
  disabled?: boolean;
  onCommit: (patch: Partial<ExtendedTranscriptionProfile>) => Promise<void>;
  onDraftChange?: (patch: Partial<ExtendedTranscriptionProfile>) => void;
  onValidationChange?: (invalid: boolean) => void;
  onValidationReasonsChange?: (reasons: ProfileValidationReason[]) => void;
  shortcut?: React.ReactNode;
  llmModelOptions: ModelOption[];
  llmModelSortKey: string;
  globalLlmModel: string;
  onRefreshModels: () => void;
  isFetchingModels: boolean;
  defaultLlmPrompt: string;
  llmPromptSourceOptions: LlmPromptSourceOption[];
  globalSonioxLanguageHintsStrict: boolean;
  globalGeminiLanguageCode: string;
  globalGeminiVocabulary: string[];
  localSttModels: ModelInfo[];
  globalSttModelSelection: SttModelSelection;
  automaticAppProfilesEnabled: boolean;
  // Note: resolvedOsLanguage removed - language is detected at transcription time, not in UI
}

const ProfileSettingsForm: React.FC<ProfileSettingsFormProps> = ({
  profile, scope, settings, disabled = false, onCommit, onDraftChange, onValidationChange, onValidationReasonsChange, shortcut,
  llmModelOptions, llmModelSortKey, globalLlmModel, onRefreshModels, isFetchingModels,
  defaultLlmPrompt, llmPromptSourceOptions, globalSonioxLanguageHintsStrict,
  globalGeminiLanguageCode, globalGeminiVocabulary, localSttModels, globalSttModelSelection,
  automaticAppProfilesEnabled,
}) => {
  const { t } = useTranslation();
  const [isSaving, setIsUpdating] = useState(false);
  const isUpdating = disabled || isSaving;
  const isDefault = scope === "global";
  const [outputDraftInvalid, setOutputDraftInvalid] = useState(false);
  const [showShortcutMoreOptions, setShowShortcutMoreOptions] =
    useWindowState(`profile-${profile.id}-shortcut-more`, false);
  const autoShortcutActivationEnabled = profile.auto_shortcut_activation === true;
  const shortcutOptionsExpanded = autoShortcutActivationEnabled || showShortcutMoreOptions;
  const pttHelp = t(autoShortcutActivationEnabled
    ? "settings.general.pushToTalk.autoPttDisabledHelp"
    : "settings.general.pushToTalk.description");
  const effectiveSttSelection = profile.stt_model_selection_override ?? globalSttModelSelection;
  const { languageOptions, supportsSttPrompt, promptLimit, isSonioxProvider, isGeminiProvider } =
    profileSttPresentation(effectiveSttSelection, localSttModels);
  const showSonioxLanguageFallbackWarning = !languageOptions.some(option => option.value === profile.language);
  const postProcessingAvailable = getPostProcessingAvailability(settings, {
    sttSelection: effectiveSttSelection,
    profilePreviewOutputOnlyEnabled: profile.preview_output_only_enabled,
    profileOutputSettings: isDefault ? null : profile.output_settings ?? {},
  }).available;
  const { supportsTranslation } = useProfileSttCapabilities(effectiveSttSelection, localSttModels);
  const effectiveLlmEnabled =
    postProcessingAvailable && (profile.llm_post_process_enabled ?? false);
  const persistedSystemPrompt = profile.system_prompt || "";
  const [systemPromptDraft, setSystemPromptDraft] = useWindowState(`profile-${profile.id}-system-prompt-draft${isDefault ? `-${effectiveSttSelection.model_id}` : ""}`, persistedSystemPrompt);
  const [systemPromptBaseline, setSystemPromptBaseline] = useWindowState(`profile-${profile.id}-system-prompt-baseline${isDefault ? `-${effectiveSttSelection.model_id}` : ""}`, persistedSystemPrompt);
  const systemPromptDirtyRef = useRef(systemPromptDraft !== systemPromptBaseline);
  const systemPromptSyncRef = useRef({
    profileId: profile.id,
    persisted: systemPromptBaseline,
  });
  const persistedAutomaticAppRules = (profile.automatic_app_rules ?? []).join(
    "\n",
  );
  const [automaticAppRulesDraft, setAutomaticAppRulesDraft] = useWindowState(`profile-${profile.id}-app-rules-draft`, persistedAutomaticAppRules);
  const [automaticAppRulesBaseline, setAutomaticAppRulesBaseline] = useWindowState(`profile-${profile.id}-app-rules-baseline`, persistedAutomaticAppRules);
  const automaticAppRulesDirtyRef = useRef(automaticAppRulesDraft !== automaticAppRulesBaseline);
  const automaticAppRulesSyncRef = useRef({
    profileId: profile.id,
    persisted: automaticAppRulesBaseline,
  });
  const [llmPromptBaseline, setLlmPromptBaseline] = useWindowState(`profile-${profile.id}-llm-prompt-baseline`, profile.llm_prompt_override ?? defaultLlmPrompt);
  // Track whether user has a custom override (non-null) or uses global (null)
  // Local state for LLM prompt
  const [localLlmPrompt, setLocalLlmPrompt] = useWindowState(`profile-${profile.id}-llm-prompt-draft`, profile.llm_prompt_override ?? defaultLlmPrompt);
  const [isCustomOverride, setIsCustomOverride] = useState(
    profile.llm_prompt_override != null || localLlmPrompt !== llmPromptBaseline,
  );
  const lastSavedLlmPromptRef = useRef({
    prompt: profile.llm_prompt_override ?? defaultLlmPrompt,
    isCustom: profile.llm_prompt_override != null,
  });
  // Track whether user is actively editing to prevent useEffect from clobbering
  const isEditingRef = useRef(localLlmPrompt !== llmPromptBaseline);
  const llmPromptSaveQueueRef = useRef<Promise<void>>(Promise.resolve());
  const llmPromptPendingCountRef = useRef(0);
  const [geminiVocabularyDraft, setGeminiVocabularyDraft] = useWindowState(`profile-${profile.id}-gemini-vocabulary-draft`, geminiVocabularyText(profile.gemini_custom_vocabulary_override ?? []));
  const [geminiVocabularyBaseline, setGeminiVocabularyBaseline] = useWindowState(`profile-${profile.id}-gemini-vocabulary-baseline`, geminiVocabularyText(profile.gemini_custom_vocabulary_override ?? []));
  const geminiVocabularyDirtyRef = useRef(geminiVocabularyDraft !== geminiVocabularyBaseline);
  const persistedGeminiVocabulary = JSON.stringify(
    profile.gemini_custom_vocabulary_override ?? null,
  );
  const persistedGeminiVocabularyDraft = geminiVocabularyText(profile.gemini_custom_vocabulary_override ?? []);
  const geminiVocabularySyncRef = useRef({
    profileId: profile.id,
    persisted: JSON.stringify(parseGeminiVocabulary(geminiVocabularyBaseline).normalizedTerms),
  });
  const geminiVocabularyResult = useMemo(
    () => parseGeminiVocabulary(geminiVocabularyDraft),
    [geminiVocabularyDraft],
  );
  const commitGeminiVocabularyDraft = async (draft: string) => {
    const result = parseGeminiVocabulary(draft);
    if (!result.safeToPersist) return;
    const normalizedTerms = result.normalizedTerms;
    setIsUpdating(true);
    try {
      await onCommit({
        gemini_custom_vocabulary_override: normalizedTerms,
      });
      geminiVocabularyDirtyRef.current = false;
      setGeminiVocabularyDraft(geminiVocabularyText(normalizedTerms));
      setGeminiVocabularyBaseline(geminiVocabularyText(normalizedTerms));
    } finally {
      setIsUpdating(false);
    }
  };

  useEffect(() => {
    const previous = systemPromptSyncRef.current;
    const profileChanged = previous.profileId !== profile.id;
    const persistedChanged = previous.persisted !== persistedSystemPrompt;
    if (
      profileChanged ||
      (!systemPromptDirtyRef.current && persistedChanged)
    ) {
      setSystemPromptDraft(persistedSystemPrompt);
      systemPromptDirtyRef.current = false;
    }
    setSystemPromptBaseline(persistedSystemPrompt);
    systemPromptSyncRef.current = {
      profileId: profile.id,
      persisted: persistedSystemPrompt,
    };
  }, [profile.id, persistedSystemPrompt]);

  useEffect(() => {
    const previous = automaticAppRulesSyncRef.current;
    const profileChanged = previous.profileId !== profile.id;
    const persistedChanged = previous.persisted !== persistedAutomaticAppRules;
    if (
      profileChanged ||
      (!automaticAppRulesDirtyRef.current && persistedChanged)
    ) {
      setAutomaticAppRulesDraft(persistedAutomaticAppRules);
      automaticAppRulesDirtyRef.current = false;
    }
    setAutomaticAppRulesBaseline(persistedAutomaticAppRules);
    automaticAppRulesSyncRef.current = {
      profileId: profile.id,
      persisted: persistedAutomaticAppRules,
    };
  }, [profile.id, persistedAutomaticAppRules]);

  useEffect(() => {
    const previous = geminiVocabularySyncRef.current;
    const profileChanged = previous.profileId !== profile.id;
    const persistedChanged = previous.persisted !== persistedGeminiVocabulary;

    if (profileChanged || (!geminiVocabularyDirtyRef.current && persistedChanged)) {
      setGeminiVocabularyDraft(persistedGeminiVocabularyDraft);
      geminiVocabularyDirtyRef.current = false;
    }

    setGeminiVocabularyBaseline(persistedGeminiVocabularyDraft);
    geminiVocabularySyncRef.current = {
      profileId: profile.id,
      persisted: persistedGeminiVocabulary,
    };
  }, [profile.id, persistedGeminiVocabulary, persistedGeminiVocabularyDraft]);

  // Sync local prompt when profile or global prompt changes (but not during active editing)
  useEffect(() => {
    if (isEditingRef.current) return;
    setLlmPromptBaseline(profile.llm_prompt_override ?? defaultLlmPrompt);
    if (profile.llm_prompt_override == null) {
      // Using global prompt — auto-track global changes
      setLocalLlmPrompt(defaultLlmPrompt);
      setIsCustomOverride(false);
      lastSavedLlmPromptRef.current = {
        prompt: defaultLlmPrompt,
        isCustom: false,
      };
    } else {
      // Custom override — show saved value
      setLocalLlmPrompt(profile.llm_prompt_override);
      setIsCustomOverride(true);
      lastSavedLlmPromptRef.current = {
        prompt: profile.llm_prompt_override,
        isCustom: true,
      };
    }
  }, [profile.llm_prompt_override, defaultLlmPrompt]);


  const languageLabel = useMemo(() => {
    if (isGeminiProvider) {
      const selection = profile.gemini_language_code_override ?? globalGeminiLanguageCode;
      if (selection === "auto") return t("settings.gemini.language.auto", "Auto Detect");
      if (selection === "os_input") {
        return t("settings.gemini.language.osInput", "Follow OS Input Language");
      }
      const locale = GEMINI_LOCALES.find(([, value]) => value === selection);
      return locale ? `${locale[0]} (${selection})` : selection;
    }
    const lang = LANGUAGES.find((l) => l.value === profile.language);
    return (
      lang?.label || profile.language || t("settings.general.language.auto")
    );
  }, [
    globalGeminiLanguageCode,
    isGeminiProvider,
    profile.gemini_language_code_override,
    profile.language,
    t,
  ]);

  const effectiveLanguageOptions = useMemo(() => {
    if (languageOptions.some((option) => option.value === profile.language)) {
      return languageOptions;
    }

    return [
      {
        value: profile.language,
        label: isSonioxProvider
          ? `${languageLabel} (fallback to auto in Soniox)`
          : `${languageLabel} (fallback to auto for selected model)`,
        className: "text-amber-400",
      },
      ...languageOptions,
    ];
  }, [languageLabel, languageOptions, profile.language, isSonioxProvider]);

  const promptLength = systemPromptDraft.length;
  const isOverLimit = promptLimit > 0 && promptLength > promptLimit;

  const sonioxContextValid = isSonioxContextValid(profile.soniox_context_general_json ?? "", profile.soniox_context_text ?? "", profile.soniox_context_terms ?? []);
  useEffect(() => {
    if (!isOverLimit) return;
    const details = document.getElementById(`profile-system-prompt-${profile.id}`)?.closest("details");
    if (details) details.open = true;
  }, [isOverLimit, profile.id]);

  const promptInvalid = supportsSttPrompt && (isDefault || profile.stt_prompt_override_enabled) && isOverLimit;
  const geminiVocabularyInvalid = profile.gemini_custom_vocabulary_override != null && !geminiVocabularyResult.safeToPersist;
  const validationReasons = useMemo<ProfileValidationReason[]>(() => [
    ...(promptInvalid ? ["system_prompt" as const] : []),
    ...(geminiVocabularyInvalid ? ["gemini_vocabulary" as const] : []),
    ...(!sonioxContextValid ? ["soniox_context" as const] : []),
    ...(outputDraftInvalid ? ["output_delay" as const] : []),
  ], [promptInvalid, geminiVocabularyInvalid, sonioxContextValid, outputDraftInvalid]);
  useEffect(() => {
    onValidationChange?.(validationReasons.length > 0);
    onValidationReasonsChange?.(validationReasons);
  }, [onValidationChange, onValidationReasonsChange, validationReasons]);

  const commitFieldsPatch = async (patch: Partial<ExtendedTranscriptionProfile>) => {
    setIsUpdating(true);
    try { await onCommit(patch); }
    finally { setIsUpdating(false); }
  };

  // Instant update handlers
  const handleLanguageChange = async (newLanguage: string) => {
    if (newLanguage === profile.language) return;
    setIsUpdating(true);
    try {
      await onCommit({ language: newLanguage });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleTranslateChange = async (newTranslate: boolean) => {
    setIsUpdating(true);
    try {
      await onCommit({ translate_to_english: newTranslate });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleSystemPromptChange = async (value = systemPromptDraft) => {
    if (promptLimit > 0 && value.length > promptLimit) return;
    if (value === persistedSystemPrompt) {
      systemPromptDirtyRef.current = false;
      return;
    }
    setIsUpdating(true);
    try {
      await onCommit({ system_prompt: value });
      systemPromptDirtyRef.current = false;
    } catch (error) {
      toast.error(
        t(
          "settings.transcriptionProfiles.systemPromptSaveFailed",
          "Could not save voice model prompt.",
        ),
        {
          description: error instanceof Error ? error.message : String(error),
        },
      );
    } finally {
      setIsUpdating(false);
    }
  };

  const handleIncludeInCycleChange = async (newValue: boolean) => {
    setIsUpdating(true);
    try {
      await onCommit({ include_in_cycle: newValue });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleAutomaticAppRulesCommit = async () => {
    const rules = automaticAppRulesDraft
      .split(/\r?\n/)
      .map((rule) => rule.trim())
      .filter(Boolean);
    const normalizedDraft = rules.join("\n");
    if (normalizedDraft === persistedAutomaticAppRules) {
      setAutomaticAppRulesDraft(normalizedDraft);
      automaticAppRulesDirtyRef.current = false;
      return;
    }

    setIsUpdating(true);
    try {
      await onCommit({ automatic_app_rules: rules });
      setAutomaticAppRulesDraft(normalizedDraft);
      automaticAppRulesDirtyRef.current = false;
    } catch (error) {
      console.error("Failed to save automatic application rules:", error);
      toast.error(
        t(
          "settings.transcriptionProfiles.automaticApps.saveFailed",
          "Failed to save automatic application rules.",
        ),
      );
    } finally {
      setIsUpdating(false);
    }
  };

  const handlePushToTalkChange = async (newValue: boolean) => {
    setIsUpdating(true);
    try {
      await onCommit({ push_to_talk: newValue });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleAutoShortcutActivationChange = async (newValue: boolean) => {
    setIsUpdating(true);
    try {
      await onCommit({ auto_shortcut_activation: newValue });
    } finally {
      setIsUpdating(false);
    }
  };

  const handlePreviewOutputOnlyChange = async (newValue: boolean) => {
    setIsUpdating(true);
    try {
      await onCommit({ preview_output_only_enabled: newValue });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleSttPromptOverrideChange = async (newValue: boolean) => {
    setIsUpdating(true);
    try {
      await onCommit({ stt_prompt_override_enabled: newValue });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleSttModelOverrideEnabledChange = async (enabled: boolean) => {
    setIsUpdating(true);
    try {
      await onCommit({
        stt_model_selection_override: enabled ? globalSttModelSelection : null,
      });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleSttModelOverrideChange = async (
    selection: SttModelSelection,
  ) => {
    setIsUpdating(true);
    try {
      await onCommit({ stt_model_selection_override: selection });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleSonioxGeneralJsonChange = async (newValue: string) => {
    if (newValue.trim() === (profile.soniox_context_general_json || "").trim())
      return;
    setIsUpdating(true);
    try {
      await onCommit({
        soniox_context_general_json: newValue,
      });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleSonioxContextTextChange = async (newValue: string) => {
    if (newValue.trim() === (profile.soniox_context_text || "").trim()) return;
    setIsUpdating(true);
    try {
      await onCommit({
        soniox_context_text: newValue,
      });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleSonioxTermsChange = async (newValue: string[]) => {
    const previous = profile.soniox_context_terms || [];
    if (
      previous.length === newValue.length &&
      previous.every((value, idx) => value === newValue[idx])
    ) {
      return;
    }
    setIsUpdating(true);
    try {
      await onCommit({
        soniox_context_terms: newValue,
      });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleLlmEnabledChange = async (newValue: boolean) => {
    setIsUpdating(true);
    try {
      await onCommit({ llm_post_process_enabled: newValue });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleLlmPromptChange = (newPrompt: string): Promise<void> => {
    onDraftChange?.({ llm_prompt_override: newPrompt });
    if (
      llmPromptPendingCountRef.current === 0 &&
      newPrompt === profile.llm_prompt_override
    ) {
      return Promise.resolve();
    }

    isEditingRef.current = true;
    llmPromptPendingCountRef.current += 1;
    const queuedSave = llmPromptSaveQueueRef.current.then(async () => {
      let saveFailed = false;
      setIsUpdating(true);
      try {
        await onCommit({
            llm_prompt_override: newPrompt,
        });
        setLlmPromptBaseline(newPrompt);
        lastSavedLlmPromptRef.current = {
          prompt: newPrompt,
          isCustom: true,
        };
      } catch (error) {
        saveFailed = true;
        console.error("Failed to save profile LLM prompt:", error);
        toast.error(
          t(
            "settings.transcriptionProfiles.llmPostProcessing.promptSaveFailed",
            "Could not save the profile LLM prompt.",
          ),
          {
            description: error instanceof Error ? error.message : String(error),
          },
        );
      } finally {
        llmPromptPendingCountRef.current -= 1;
        if (llmPromptPendingCountRef.current === 0) {
          if (saveFailed) {
            setLocalLlmPrompt(lastSavedLlmPromptRef.current.prompt);
            setLlmPromptBaseline(lastSavedLlmPromptRef.current.prompt);
            setIsCustomOverride(lastSavedLlmPromptRef.current.isCustom);
          }
          setIsUpdating(false);
          isEditingRef.current = false;
        }
      }
    });
    llmPromptSaveQueueRef.current = queuedSave;
    return queuedSave;
  };

  const handleLlmPromptSourceSelect = (promptId: string) => {
    const source = llmPromptSourceOptions.find(
      (option) => option.value === promptId,
    );
    if (!source) return;

    setLocalLlmPrompt(source.prompt);
    setIsCustomOverride(true);
    void handleLlmPromptChange(source.prompt);
  };

  const handleResetToGlobal = async () => {
    if (profile.llm_prompt_override == null) return;
    setLocalLlmPrompt(defaultLlmPrompt);
    setIsCustomOverride(false);
    isEditingRef.current = false;
    setIsUpdating(true);
    try {
      await onCommit({
        llm_prompt_override: null,
      });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleLlmModelChange = async (newModel: string | null) => {
    if (newModel === profile.llm_model_override) return;
    setIsUpdating(true);
    try {
      await onCommit({
        llm_model_override: newModel || null,
      });
    } finally {
      setIsUpdating(false);
    }
  };

  return (
    <div className="space-y-3">
            {!isDefault && <div className="min-w-0 bg-mid-gray/5 p-3 rounded-lg border border-mid-gray/10">
              <label htmlFor={`profile-cycle-${profile.id}`} className="text-xs font-semibold text-text/70 block mb-2">
                {t("settings.transcriptionProfiles.includeInCycle")}
              </label>
              <div className="flex items-center gap-2">
                <input
                  id={`profile-cycle-${profile.id}`}
                  type="checkbox"
                  checked={profile.include_in_cycle}
                  onChange={(e) => handleIncludeInCycleChange(e.target.checked)}
                  disabled={isUpdating}
                  className="w-4 h-4 rounded border-mid-gray bg-background text-purple-500 focus:ring-purple-500/50"
                />
                <span className="text-xs text-mid-gray leading-snug">
                  {t(
                    "settings.transcriptionProfiles.includeInCycleDescription",
                  )}
                </span>
              </div>
            </div>}

          {!isDefault && automaticAppProfilesEnabled && (
            <div className="space-y-2 rounded-lg border border-sky-500/20 bg-sky-500/5 p-3">
              <div className="space-y-1">
                <p className="text-xs font-semibold text-text/80">
                  {t(
                    "settings.transcriptionProfiles.automaticApps.title",
                    "Automatically use this profile in applications",
                  )}
                </p>
                <p className="text-xs text-mid-gray leading-relaxed">
                  {t(
                    "settings.transcriptionProfiles.automaticApps.description",
                    "When the main Transcribe shortcut starts in a matching foreground application, this profile overrides the manually active profile for that recording only.",
                  )}
                </p>
              </div>
              <Textarea
                value={automaticAppRulesDraft}
                rows={4}
                className="w-full font-mono text-xs"
                aria-label={t(
                  "settings.transcriptionProfiles.automaticApps.title",
                  "Automatically use this profile in applications",
                )}
                placeholder={t(
                  "settings.transcriptionProfiles.automaticApps.placeholder",
                  "code.exe\nchrome.exe\ntitle:Microsoft Teams\npath:*\\Microsoft Office\\*",
                )}
                onChange={(event) => {
                  const nextDraft = event.target.value;
                  automaticAppRulesDirtyRef.current =
                    nextDraft !== persistedAutomaticAppRules;
                  setAutomaticAppRulesDraft(nextDraft);
                  onDraftChange?.({ automatic_app_rules: nextDraft.split(/\r?\n/).map(rule => rule.trim()).filter(Boolean) });
                }}
                onBlur={() => void handleAutomaticAppRulesCommit()}
                disabled={isUpdating}
              />
              <p className="text-[11px] text-mid-gray leading-relaxed">
                {t(
                  "settings.transcriptionProfiles.automaticApps.help",
                  "One rule per line. Use an executable name such as code.exe, or the prefixes exe:, title:, and path:. Matching ignores case; * and ? are wildcards. The most specific rule wins. Profile-specific shortcuts always keep their explicit profile.",
                )}
              </p>
            </div>
          )}

          {!isDefault && (
          <div className="space-y-3 rounded-lg border border-mid-gray/10 bg-mid-gray/5 p-3">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="text-xs font-semibold text-text/70">
                  {t(
                    "settings.sttModelSelector.profileOverrideTitle",
                    "Override dictation model",
                  )}
                </p>
                <p className="mt-1 text-xs text-mid-gray">
                  {t(
                    "settings.sttModelSelector.profileOverrideDescription",
                    "Use a different provider and model for ordinary dictation with this profile.",
                  )}
                </p>
              </div>
              <ToggleSwitch
                checked={profile.stt_model_selection_override != null}
                onChange={handleSttModelOverrideEnabledChange}
                disabled={isUpdating}
              />
            </div>
            {profile.stt_model_selection_override && (
              <SttModelSelector
                workflow="dictation"
                selection={profile.stt_model_selection_override}
                localModels={localSttModels}
                onChange={handleSttModelOverrideChange}
                disabled={isUpdating}
              />
            )}
          </div>

          )}

          {/* Shortcut assignment and recording press mode */}
          <div className="space-y-3 bg-mid-gray/5 p-3 rounded-lg border border-mid-gray/10">
            {/* Shortcut identity is supplied by the persistence adapter. */}
            {shortcut && <div className="space-y-2 min-w-0">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.shortcut")}
              </label>
              {shortcut}
            </div>}

            <div id={isDefault ? "settings-dictation-press-mode" : undefined} tabIndex={isDefault ? -1 : undefined} className="min-w-0">
              <div className="flex items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-2">
                <label htmlFor={`profile-push-to-talk-${profile.id}`} className="text-xs font-semibold text-text/70">
                  {t("settings.general.pushToTalk.label")}
                </label>
                <InfoTooltip content={pttHelp} />
                {!autoShortcutActivationEnabled && <button
                  type="button"
                  onClick={() => setShowShortcutMoreOptions((shown) => !shown)}
                  aria-expanded={shortcutOptionsExpanded}
                  aria-controls={`profile-shortcut-options-${profile.id}`}
                  className="text-[11px] text-[#8b8490] underline underline-offset-2 hover:text-[#aaa3ae] transition-colors"
                >
                  {t(shortcutOptionsExpanded ? "settings.general.pushToTalk.hideOptions" : "settings.general.pushToTalk.showOptions")}
                </button>}
              </div>
              <div className="flex shrink-0 items-center gap-2">
                <Tooltip content={pttHelp} disabled={!autoShortcutActivationEnabled} className="inline-flex"
                  tabIndex={autoShortcutActivationEnabled ? 0 : undefined}
                  ariaLabel={t("settings.general.pushToTalk.label")}
                  ariaDisabled={autoShortcutActivationEnabled}>
                  <ToggleSwitch
                    inputId={`profile-push-to-talk-${profile.id}`}
                    checked={profile.push_to_talk ?? true}
                    onChange={handlePushToTalkChange}
                    disabled={isUpdating || autoShortcutActivationEnabled}
                  />
                </Tooltip>
              </div>
              </div>
              <Collapse open={shortcutOptionsExpanded}>
                <div id={`profile-shortcut-options-${profile.id}`} className="mt-3 pt-3 border-t border-mid-gray/15">
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <label htmlFor={`profile-auto-shortcut-${profile.id}`} className="text-xs font-medium text-text/80">
                        {t(
                          "settings.general.pushToTalk.autoLabel",
                          "Automatic tap / hold",
                        )}
                      </label>
                      <div className="mt-1 text-[11px] text-mid-gray leading-snug">
                        {t(
                          "settings.general.pushToTalk.autoDescription",
                          "Recording starts immediately. Release within 300 ms to keep recording until the next press, or keep holding and release to stop.",
                        )}
                      </div>
                    </div>
                    <ToggleSwitch
                      inputId={`profile-auto-shortcut-${profile.id}`}
                      checked={profile.auto_shortcut_activation ?? false}
                      onChange={handleAutoShortcutActivationChange}
                      disabled={isUpdating}
                    />
                  </div>
                </div>
              </Collapse>
            </div>
          </div>

          {/* Language + Translate to English — same row */}
          <div className="grid gap-3 lg:grid-cols-2">
            {/* Language Selection */}
            {!isGeminiProvider && (
              <div className="space-y-2 relative z-20 min-w-0">
              <label htmlFor={`profile-language-${profile.id}`} className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.language")}
              </label>
              <NameSortedDropdown
                id={`profile-language-${profile.id}`}
                sortKey="profile-languages"
                fixedDirection="asc"
                pinnedValues={["auto", "os_input"]}
                selectedValue={profile.language}
                options={effectiveLanguageOptions.map((l) => ({
                  value: l.value,
                  label: l.label,
                  className: l.className,
                }))}
                onSelect={(value) => value && handleLanguageChange(value)}
                placeholder={t("settings.general.language.auto")}
                disabled={isUpdating}
              />
              {isSonioxProvider && (
                <div className="flex items-center gap-1.5 mt-1.5">
                  <Tooltip content={t("settings.advanced.soniox.languageHintsStrict.titleTooltip", "This option only appears if a Soniox model is used")}>
                    <span className="text-[10px] text-text/60">{t("settings.advanced.soniox.languageHintsStrict.title", "Strict")}:</span>
                  </Tooltip>
                  <div onClick={(e) => e.stopPropagation()}>
                    {isDefault ? <ToggleSwitch checked={Boolean(profile.soniox_language_hints_strict)} disabled={isUpdating}
                      onChange={value => onCommit({ soniox_language_hints_strict: value })} /> : (
                    <TriStateToggle
                      value={strictModeFromApi(
                        profile.soniox_language_hints_strict,
                      )}
                      globalValue={globalSonioxLanguageHintsStrict}
                      onChange={async (mode) => {
                        setIsUpdating(true);
                        try {
                          await onCommit({
                            soniox_language_hints_strict: strictModeToApi(mode),
                          });
                        } finally {
                          setIsUpdating(false);
                        }
                      }}
                      disabled={isUpdating}
                    />                    )}
                  </div>
                  <InfoTooltip content={t("settings.advanced.soniox.languageHintsStrict.description")} />
                </div>
              )}
              {showSonioxLanguageFallbackWarning && (
                <p className="text-xs text-amber-400/90">
                  {isSonioxProvider
                    ? t(
                        "settings.transcriptionProfiles.sonioxLanguageFallbackWarning",
                        "This language is not supported by Soniox. Soniox requests will fallback to auto-detect.",
                      )
                    : t(
                        "settings.transcriptionProfiles.localModelLanguageFallbackWarning",
                        "This language is not supported by the selected local model. Requests will fallback to auto-detect.",
                      )}
                </p>
              )}
              </div>
            )}

            {/* Translate to English Toggle */}
            {supportsTranslation && (
              <div id={isDefault ? "settings-translate-to-english" : undefined} tabIndex={isDefault ? -1 : undefined} className="space-y-2 min-w-0">
                <label htmlFor={`profile-translate-${profile.id}`} className="text-xs font-semibold text-text/70">
                  {t("settings.transcriptionProfiles.translateToEnglish")}
                </label>
                <div className="flex items-center gap-2">
                  <button
                    id={`profile-translate-${profile.id}`}
                    type="button"
                    role="switch"
                    aria-checked={profile.translate_to_english}
                    onClick={() =>
                      handleTranslateChange(!profile.translate_to_english)
                    }
                    disabled={isUpdating}
                    className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors shrink-0 ${
                      profile.translate_to_english
                        ? "bg-purple-500"
                        : "bg-mid-gray/30"
                    } ${isUpdating ? "opacity-50 cursor-not-allowed" : "cursor-pointer"}`}
                  >
                    <span
                      className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                        profile.translate_to_english
                          ? "translate-x-6"
                          : "translate-x-1"
                      }`}
                    />
                  </button>
                  <span className="text-xs text-mid-gray leading-snug">
                    {t(
                      "settings.transcriptionProfiles.translateToEnglishDescription",
                    )}
                  </span>
                </div>
              </div>
            )}
          </div>

          {(isGeminiProvider || (profile.gemini_custom_vocabulary_override != null && !geminiVocabularyResult.safeToPersist)) && (
            <div className="space-y-3 border-t border-mid-gray/10 pt-3">
              <div className="space-y-2">
                <label className="text-xs font-semibold text-text/70">
                  {t("settings.gemini.language.title", "Gemini language")}
                </label>
                <NameSortedDropdown
                  sortKey="profile-gemini-languages"
                  fixedDirection="asc"
                  pinnedValues={isDefault ? ["auto", "os_input"] : ["__inherit__", "auto", "os_input"]}
                  selectedValue={profile.gemini_language_code_override ?? "__inherit__"}
                  options={[
                    ...(!isDefault ? [{
                      value: "__inherit__",
                      label: t("settings.gemini.inheritLanguage", {
                        language: globalGeminiLanguageCode,
                        defaultValue: `Inherit global (${globalGeminiLanguageCode})`,
                      }),
                    }] : []),
                    { value: "auto", label: t("settings.gemini.language.auto", "Auto Detect") },
                    { value: "os_input", label: t("settings.gemini.language.osInput", "Follow OS Input Language") },
                    ...GEMINI_LOCALES.map(([label, value]) => ({ value, label: `${label} (${value})` })),
                  ]}
                  onSelect={async value => {
                    if (!value) return;
                    setIsUpdating(true);
                    try {
                      await onCommit({
                        gemini_language_code_override: value === "__inherit__" ? null : value,
                      });
                    } finally {
                      setIsUpdating(false);
                    }
                  }}
                  disabled={isUpdating}
                />
                <p className="text-xs text-mid-gray">
                  {t(
                    "settings.gemini.language.mappingHelp",
                    "If the OS input language does not map unambiguously to an exact supported locale, AivoRelay uses automatic detection and does not send an unsupported hint.",
                  )}
                </p>
              </div>

              <div className="space-y-2">
                <div className="flex items-center justify-between gap-3">
                  <label className="text-xs font-semibold text-text/70">
                    {t("settings.gemini.vocabulary.title", "Gemini Custom Vocabulary")}
                  </label>
                  {!isDefault && <label className="flex items-center gap-2 text-xs text-mid-gray">
                    <input
                      type="checkbox"
                      checked={profile.gemini_custom_vocabulary_override != null}
                      onChange={async event => {
                        const enabled = event.target.checked;
                        const next = enabled ? [] : null;
                        geminiVocabularyDirtyRef.current = false;
                        setGeminiVocabularyDraft("");
                        setIsUpdating(true);
                        try {
                          await onCommit({ gemini_custom_vocabulary_override: next });
                        } catch (error) {
                          setGeminiVocabularyDraft(persistedGeminiVocabularyDraft);
                          throw error;
                        } finally {
                          setIsUpdating(false);
                        }
                      }}
                      disabled={isUpdating}
                    />
                    {t("settings.gemini.vocabulary.override", "Replace global vocabulary")}
                  </label>}
                </div>
                {profile.gemini_custom_vocabulary_override == null ? (
                  <p className="text-xs text-mid-gray">
                    {t("settings.gemini.vocabulary.inherited", "Inheriting {{count}} global terms.", { count: globalGeminiVocabulary.length })}
                  </p>
                ) : (
                  <>
                    <Textarea
                      value={geminiVocabularyDraft}
                      className="w-full"
                      onChange={event => {
                        const nextDraft = event.target.value;
                        geminiVocabularyDirtyRef.current =
                          nextDraft !== persistedGeminiVocabularyDraft;
                        setGeminiVocabularyDraft(nextDraft);
                        const parsed = parseGeminiVocabulary(nextDraft);
                        if (parsed.safeToPersist) onDraftChange?.({ gemini_custom_vocabulary_override: parsed.normalizedTerms });
                      }}
                      onBlur={() => void commitGeminiVocabularyDraft(geminiVocabularyDraft)}
                      rows={7}
                      disabled={isUpdating}
                      placeholder={'Gemini\nKubernetes\nBigQuery'}
                    />
                    <div className="flex items-start justify-between gap-3">
                      <p className={`text-xs ${geminiVocabularyResult.errors.length ? "text-red-400" : "text-mid-gray"}`}>
                        {t("settings.gemini.vocabulary.status", "Detected {{format}} · {{count}}/1000 terms", {
                          format: geminiVocabularyResult.format,
                          count: geminiVocabularyResult.normalizedTerms.length,
                        })}
                      </p>
                      <TextFileActions
                        value={geminiVocabularyDraft}
                        defaultFileName="gemini-vocabulary.txt"
                        extensions={GEMINI_VOCABULARY_FILE_EXTENSIONS}
                        validate={validateGeminiVocabularyFile}
                        disabled={isUpdating}
                        onOpen={async text => {
                          geminiVocabularyDirtyRef.current = true;
                          setGeminiVocabularyDraft(text);
                          await commitGeminiVocabularyDraft(text);
                        }}
                      />
                    </div>
                    <p className="text-xs text-mid-gray">
                      {t(
                        "settings.gemini.vocabulary.formatHelp",
                        "Paste one term per line, CSV, quoted CSV, or a JSON string array. Example: Gemini, Kubernetes, BigQuery",
                      )}
                    </p>
                    {geminiVocabularyResult.errors.map(issue => (
                      <p key={`${issue.code}-${issue.position ?? 0}`} className="text-xs text-red-400">{issue.message}</p>
                    ))}
                    {geminiVocabularyResult.warnings.map(issue => (
                      <p key={`${issue.code}-${issue.position ?? 0}`} className="text-xs text-amber-400">{issue.message}</p>
                    ))}
                  </>
                )}
              </div>
            </div>
          )}

          {(isSonioxProvider || !sonioxContextValid) && (
            <SonioxContextEditor
              stateKey={`profile-${profile.id}-soniox`}
              generalJson={profile.soniox_context_general_json || ""}
              text={profile.soniox_context_text || ""}
              terms={profile.soniox_context_terms || []}
              disabled={isUpdating}
              onDraftGeneralJsonChange={value => onDraftChange?.({ soniox_context_general_json: value })}
              onDraftTextChange={value => onDraftChange?.({ soniox_context_text: value })}
              onDraftTermsChange={value => onDraftChange?.({ soniox_context_terms: value })}
              onCommitGeneralJson={handleSonioxGeneralJsonChange}
              onCommitText={handleSonioxContextTextChange}
              onCommitTerms={handleSonioxTermsChange}
            />
          )}

          {/* Voice Model Prompt Override — collapsible */}
          {!isSonioxProvider && supportsSttPrompt && (
            <WindowDetails id={isDefault ? "settings-speech-model-prompt" : scope === "draft" ? "new-profile-stt-prompt-details" : undefined} tabIndex={isDefault ? -1 : undefined} stateKey={`profile-${profile.id}-stt-prompt`} className="group rounded-lg border border-mid-gray/20 bg-mid-gray/5 overflow-hidden transition-colors open:border-purple-500/30 open:bg-purple-500/5">
              <summary className="flex items-center justify-between gap-2 px-3 py-2.5 cursor-pointer select-none list-none [&::-webkit-details-marker]:hidden hover:bg-mid-gray/10 transition-colors">
                <div className="flex items-center gap-2 min-w-0">
                  <ChevronDown className="w-3.5 h-3.5 text-mid-gray shrink-0 transition-transform group-open:rotate-180" />
                  <span className="text-xs font-semibold text-text/70">
                    {t(isDefault ? "settings.transcriptionProfiles.systemPromptGlobal" : "settings.transcriptionProfiles.overrideSystemPrompt")}
                  </span>
                  <InfoTooltip
                    content={t(
                      isDefault ? "settings.transcriptionProfiles.voiceModelPromptGlobalTooltip" : "settings.transcriptionProfiles.voiceModelPromptTooltip",
                    )}
                  />
                </div>
                <div className="flex items-center gap-2 shrink-0">
                  {/* Status badges */}
                  {!isDefault && profile.stt_prompt_override_enabled && (
                    <span
                      className={`text-[10px] px-1.5 py-0.5 rounded-full border whitespace-nowrap ${
                        (profile.system_prompt || "").trim()
                          ? "bg-purple-500/20 text-purple-400 border-purple-500/30"
                          : "bg-amber-500/15 text-amber-400 border-amber-500/25"
                      }`}
                    >
                      {(profile.system_prompt || "").trim()
                        ? t(
                            "settings.transcriptionProfiles.badges.sttPromptOverridden",
                            "System prompt overridden",
                          )
                        : t(
                            "settings.transcriptionProfiles.badges.sttPromptEmpty",
                            "System prompt is empty",
                          )}
                    </span>
                  )}
                  {!isDefault && (
                  <div onClick={(e) => e.stopPropagation()}>
                    <ToggleSwitch
                      checked={profile.stt_prompt_override_enabled ?? false}
                      onChange={handleSttPromptOverrideChange}
                      disabled={isUpdating}
                    />
                  </div>
                  )}
                </div>
              </summary>
              <div className="px-3 pb-3 space-y-2 border-t border-mid-gray/10">
                <p className="text-xs text-mid-gray pt-2">
                  {isDefault ? t("settings.transcriptionProfiles.voiceModelPromptGlobalTooltip") : profile.stt_prompt_override_enabled
                    ? t(
                        "settings.transcriptionProfiles.overrideSystemPromptOnDescription",
                      )
                    : t(
                        "settings.transcriptionProfiles.overrideSystemPromptOffDescription",
                      )}
                </p>
                {isDefault && !effectiveSttSelection.model_id && <p className="text-xs text-amber-400/90">
                  {t("settings.transcriptionProfiles.selectModelToEditGlobalPrompt", "Select an active transcription model to edit this prompt.")}
                </p>}
                {(isDefault || profile.stt_prompt_override_enabled) && (
                  <>
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2">
                        <label htmlFor={`profile-system-prompt-${profile.id}`} className="text-xs font-semibold text-text/70">
                          {t("settings.transcriptionProfiles.systemPrompt")}
                        </label>
                        <InfoTooltip
                          content={t(
                            isDefault ? "settings.transcriptionProfiles.voiceModelPromptGlobalTooltip" : "settings.transcriptionProfiles.voiceModelPromptTooltip",
                          )}
                        />
                      </div>
                      <span
                        className={`text-xs ${isOverLimit ? "text-red-400" : "text-mid-gray"}`}
                      >
                        {promptLength}
                        {promptLimit > 0 && ` / ${promptLimit}`}
                      </span>
                    </div>
                    <textarea
                      id={`profile-system-prompt-${profile.id}`}
                      value={systemPromptDraft}
                      onChange={(event) => {
                        const nextDraft = event.target.value;
                        systemPromptDirtyRef.current =
                          nextDraft !== persistedSystemPrompt;
                        setSystemPromptDraft(nextDraft);
                        onDraftChange?.({ system_prompt: nextDraft });
                      }}
                      onBlur={() => void handleSystemPromptChange()}
                      placeholder={t(
                        "settings.transcriptionProfiles.systemPromptPlaceholder",
                      )}
                      disabled={isUpdating || (isDefault && !effectiveSttSelection.model_id)}
                      aria-invalid={isOverLimit}
                      rows={3}
                      className={`w-full px-3 py-2 text-sm bg-[#1e1e1e]/80 border rounded-md resize-none transition-colors ${
                        isOverLimit
                          ? "border-red-400 focus:border-red-400"
                          : "border-[#3c3c3c] focus:border-[#4a4a4a]"
                      } ${isUpdating ? "opacity-40 cursor-not-allowed" : ""} text-[#e8e8e8] placeholder-[#6b6b6b]`}
                    />
                    <div className="flex items-start justify-between gap-3">
                      <p className="text-xs text-mid-gray">
                        {t(
                          "settings.transcriptionProfiles.systemPromptDescription",
                        )}
                      </p>
                      <TextFileActions
                        value={systemPromptDraft}
                        defaultFileName="voice-model-prompt.txt"
                        maxLength={promptLimit}
                        disabled={isUpdating || (isDefault && !effectiveSttSelection.model_id)}
                        onOpen={async (text) => {
                          systemPromptDirtyRef.current =
                            text !== persistedSystemPrompt;
                          setSystemPromptDraft(text);
                          await handleSystemPromptChange(text);
                        }}
                      />
                    </div>
                    {isOverLimit && (
                      <p className="text-xs text-red-400">
                        {t(
                          "settings.transcriptionProfiles.systemPromptTooLong",
                          {
                            limit: promptLimit,
                          },
                        )}
                      </p>
                    )}
                  </>
                )}
              </div>
            </WindowDetails>
          )}

          <ProfileMoreSettings profileId={profile.id} previewEnabled={profile.preview_output_only_enabled ?? false}
            keyActionsEnabled={profileKeyActionsEnabled(profile.output_settings ?? {}, profileOutputDefaults(getGlobalProfileOutputDefaults(settings), isDefault))}>
            <div id={`profile-preview-row-${profile.id}`} tabIndex={-1} className="min-w-0 flex items-center justify-between gap-3 rounded-lg border border-mid-gray/10 bg-mid-gray/5 p-3">
              <div className="min-w-0 space-y-1">
                <div className="flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
                  <label htmlFor={`profile-preview-${profile.id}`} className="text-xs font-semibold text-text/70">
                    {t(
                      "settings.transcriptionProfiles.previewOutputOnly.title",
                      "Output to Preview",
                    )}
                  </label>
                  <button type="button" onClick={openLivePreviewSettings} className="text-[11px] text-mid-gray underline underline-offset-2 hover:text-text transition-colors">
                    {t("settings.transcriptionProfiles.previewOutputOnly.openSettings", "Preview settings")}
                  </button>
                </div>
                <p className="text-xs text-mid-gray leading-snug">
                  {t(
                    "settings.transcriptionProfiles.previewOutputOnly.description",
                    "Keep this profile's output in the preview buffer before insertion.",
                  )}
                </p>
              </div>
              <ToggleSwitch
                inputId={`profile-preview-${profile.id}`}
                checked={profile.preview_output_only_enabled ?? false}
                onChange={handlePreviewOutputOnlyChange}
                disabled={isUpdating}
              />
            </div>

            <ProfileOutputSettingsFields scopeKey={profile.id} values={profile.output_settings ?? {}}
              defaults={getGlobalProfileOutputDefaults(settings)} isDefault={isDefault} disabled={isUpdating}
              onChange={patch => commitFieldsPatch({ output_settings: patch })}
              onDraftChange={scope === "draft" ? patch => onDraftChange?.({ output_settings: patch }) : undefined}
              onValidationChange={setOutputDraftInvalid}
              sttSelection={effectiveSttSelection} localModels={localSttModels}
              previewOutputOnly={profile.preview_output_only_enabled ?? false} />

          {/* LLM Post-Processing Section — collapsible */}
          <WindowDetails stateKey={`profile-${profile.id}-llm`} className="group rounded-lg border border-mid-gray/20 bg-mid-gray/5 overflow-hidden transition-colors open:border-purple-500/30 open:bg-purple-500/5">
              <summary className="flex items-center justify-between gap-2 px-3 py-2.5 cursor-pointer select-none list-none [&::-webkit-details-marker]:hidden hover:bg-mid-gray/10 transition-colors">
                <div className="flex items-center gap-2 min-w-0">
                  <ChevronDown className="w-3.5 h-3.5 text-mid-gray shrink-0 transition-transform group-open:rotate-180" />
                  <span className="text-xs font-semibold text-text/70">
                    {t(
                      "settings.transcriptionProfiles.llmPostProcessing.title",
                    )}
                  </span>
                  <InfoTooltip
                    content={t(
                      isDefault ? "settings.postProcessing.defaultProfileDescription" : "settings.transcriptionProfiles.llmPostProcessing.description",
                    )}
                  />
                </div>
                <div className="flex items-center gap-2 shrink-0">
                  {/* Status badges */}
                  {effectiveLlmEnabled && (
                    <div className="flex flex-wrap items-center gap-1.5">
                      <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-purple-500/20 text-purple-400 border border-purple-500/30 whitespace-nowrap">
                        {t(
                          "settings.transcriptionProfiles.badges.llmActive",
                          "LLM active",
                        )}
                      </span>
                      {profile.llm_prompt_override &&
                        profile.llm_prompt_override !== defaultLlmPrompt && (
                          <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-amber-500/15 text-amber-400 border border-amber-500/25 whitespace-nowrap">
                            {t(
                              "settings.transcriptionProfiles.badges.promptOverridden",
                              "Prompt overridden",
                            )}
                          </span>
                        )}
                      {profile.llm_model_override && (
                        <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-blue-500/15 text-blue-400 border border-blue-500/25 whitespace-nowrap">
                          {t(
                            "settings.transcriptionProfiles.badges.modelOverride",
                            "Model: {{model}}",
                            { model: profile.llm_model_override },
                          )}
                        </span>
                      )}
                    </div>
                  )}
                  <div onClick={(e) => e.stopPropagation()}>
                    <ToggleSwitch
                      checked={effectiveLlmEnabled}
                      onChange={handleLlmEnabledChange}
                      disabled={isUpdating || !postProcessingAvailable}
                      ariaLabel={t(
                        isDefault ? "settings.postProcessing.defaultProfileToggleLabel" : "settings.transcriptionProfiles.llmPostProcessing.title",
                      )}
                    />
                  </div>
                </div>
              </summary>
              <div className="px-3 pb-3 space-y-2 border-t border-mid-gray/10">
                <p className="text-xs text-mid-gray pt-2">
                  {t(
                    isDefault ? "settings.postProcessing.defaultProfileDescription" : "settings.transcriptionProfiles.llmPostProcessing.description",
                  )}
                </p>

                {!postProcessingAvailable && (
                  <div className="p-2 bg-red-500/10 border border-red-500/30 rounded text-xs text-red-400">
                    <Trans
                      i18nKey="settings.postProcessing.unavailableDirectRealtime"
                      defaultValue="LLM post-processing is unavailable while realtime text is inserted directly into the target application. Use <preview>Preview</preview> or a non-live output route. Your saved choice is preserved for compatible routes."
                      components={{
                        preview: (
                          <button
                            type="button"
                            className="underline underline-offset-2 hover:text-red-200"
                            onClick={() => {
                              // The row is in this form, inside "More", which may be collapsed.
                              navigateToSettingsAnchor({
                                activateSection: () => {},
                                targetId: `profile-preview-row-${profile.id}`,
                                block: "center",
                                updateHash: false,
                              });
                            }}
                          />
                        ),
                      }}
                    />
                  </div>
                )}

                {isDefault && (
                  <p className="text-xs text-mid-gray">
                    {t(
                      "settings.postProcessing.defaultProfileNote",
                      "Custom profiles keep their own post-processing choice. Provider, model, API key, and prompt configuration are shared.",
                    )}
                  </p>
                )}
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  onClick={openPostProcessingSettings}
                >
                  {t(
                    "settings.postProcessing.openConfiguration",
                    "Open LLM Post Processing settings",
                  )}
                </Button>
                {!isDefault && effectiveLlmEnabled && (
                  <div className="space-y-3">
                    {/* Prompt Override */}
                    <div className="space-y-1">
                      {/* ${output} Variable Documentation */}
                      <div className="p-2 bg-blue-500/10 border border-blue-500/30 rounded text-xs space-y-1">
                        <div className="font-semibold text-blue-400">
                          {t(
                            "settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.title",
                          )}
                        </div>
                        {/* Trusted HTML from repo-controlled translations only. Do not pass user, model, or network content here. */}
                        <p
                          className="text-text/70"
                          dangerouslySetInnerHTML={{
                            __html: t(
                              "settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.description",
                            ),
                          }}
                        />
                        <WindowDetails stateKey={`profile-${profile.id}-llm-variables`} className="cursor-pointer">
                          <summary className="text-blue-400 hover:text-blue-300">
                            {t(
                              "settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.howItWorks",
                            )}
                          </summary>
                          <ol className="list-decimal list-inside text-text/60 mt-1 space-y-0.5 pl-2">
                            <li>
                              {t(
                                "settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.step1",
                              )}
                            </li>
                            {/* Trusted HTML from repo-controlled translations only. Do not pass user, model, or network content here. */}
                            <li
                              dangerouslySetInnerHTML={{
                                __html: t(
                                  "settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.step2",
                                ),
                              }}
                            />
                            <li>
                              {t(
                                "settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.step3",
                              )}
                            </li>
                            <li>
                              {t(
                                "settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.step4",
                              )}
                            </li>
                          </ol>
                          <div className="mt-2 space-y-1">
                            <div className="text-green-400">
                              {t(
                                "settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.validExample",
                              )}
                            </div>
                            <code className="block bg-green-500/10 px-2 py-1 rounded text-green-300 whitespace-pre-wrap">
                              {t(
                                "settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.validPrompt",
                              )}
                            </code>
                            <div className="text-red-400 mt-2">
                              {t(
                                "settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.invalidExample",
                              )}
                            </div>
                            <code className="block bg-red-500/10 px-2 py-1 rounded text-red-300">
                              {t(
                                "settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.invalidPrompt",
                              )}
                            </code>
                            {/* Trusted HTML from repo-controlled translations only. Do not pass user, model, or network content here. */}
                            <p
                              className="text-red-400/80"
                              dangerouslySetInnerHTML={{
                                __html: t(
                                  "settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.invalidWarning",
                                ),
                              }}
                            />
                          </div>
                          {/* Trusted HTML from repo-controlled translations only. Do not pass user, model, or network content here. */}
                          <p
                            className="mt-2 text-text/60"
                            dangerouslySetInnerHTML={{
                              __html: t(
                                "settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.tip",
                              ),
                            }}
                          />
                        </WindowDetails>
                      </div>

                      <div className="mt-2 space-y-1">
                        <label className="text-xs text-text/60">
                          {t(
                            "settings.transcriptionProfiles.llmPostProcessing.useExistingPrompt",
                            "Use an existing prompt",
                          )}
                        </label>
                        <p className="text-xs text-mid-gray">
                          {t(
                            "settings.transcriptionProfiles.llmPostProcessing.useExistingPromptHint",
                            "Copy a saved LLM Post-Processing prompt into this profile. Later edits affect only this profile.",
                          )}
                        </p>
                        <NameSortedDropdown
                          sortKey="profile-prompts"
                          options={llmPromptSourceOptions}
                          selectedValue={null}
                          onSelect={handleLlmPromptSourceSelect}
                          placeholder={t(
                            llmPromptSourceOptions.length > 0
                              ? "settings.transcriptionProfiles.llmPostProcessing.selectExistingPrompt"
                              : "settings.transcriptionProfiles.llmPostProcessing.noExistingPrompts",
                            llmPromptSourceOptions.length > 0
                              ? "Select a saved prompt..."
                              : "No saved prompts available",
                          )}
                          disabled={
                            llmPromptSourceOptions.length === 0 ||
                            (isUpdating &&
                              llmPromptPendingCountRef.current === 0)
                          }
                          ariaLabel={t(
                            "settings.transcriptionProfiles.llmPostProcessing.useExistingPrompt",
                            "Use an existing prompt",
                          )}
                          dropUp={false}
                        />
                      </div>

                      <div className="flex items-center justify-between mt-2">
                        <div className="flex flex-col">
                          {!isCustomOverride ? (
                            <span className="text-[10px] text-purple-400/70 font-medium">
                              {t(
                                "settings.transcriptionProfiles.llmPostProcessing.usingGlobalPrompt",
                              )}
                            </span>
                          ) : (
                            <span className="text-[10px] text-orange-400 font-medium">
                              {t(
                                "settings.transcriptionProfiles.llmPostProcessing.customPromptActive",
                                "Custom prompt active",
                              )}
                            </span>
                          )}
                        </div>
                        {isCustomOverride && (
                          <button
                            type="button"
                            onClick={handleResetToGlobal}
                            disabled={isUpdating}
                            className="flex items-center gap-1.5 px-2 py-1 text-xs bg-purple-500/10 hover:bg-purple-500/20 border border-purple-500/30 rounded-md text-purple-300 transition-colors disabled:opacity-50"
                          >
                            <RotateCcw className="w-3.5 h-3.5" />
                            {t(
                              "settings.transcriptionProfiles.llmPostProcessing.resetToGlobal",
                            )}
                          </button>
                        )}
                      </div>
                      <textarea
                        value={localLlmPrompt}
                        onChange={(e) => {
                          setLocalLlmPrompt(e.target.value);
                          onDraftChange?.({ llm_prompt_override: e.target.value });
                          setIsCustomOverride(true);
                          isEditingRef.current = true;
                        }}
                        onBlur={(e) => {
                          isEditingRef.current = false;
                          if (isCustomOverride) {
                            void handleLlmPromptChange(e.target.value);
                          }
                        }}
                        placeholder={t(
                          "settings.transcriptionProfiles.llmPostProcessing.promptPlaceholder",
                        )}
                        disabled={isUpdating}
                        rows={6}
                        className={`w-full px-3 py-2 text-sm bg-[#1e1e1e]/80 border rounded-md resize-none transition-colors border-[#3c3c3c] focus:border-[#4a4a4a] ${isUpdating ? "opacity-40 cursor-not-allowed" : ""} text-[#e8e8e8] placeholder-[#6b6b6b]`}
                      />
                      <TextFileActions
                        value={localLlmPrompt}
                        defaultFileName="llm-prompt.txt"
                        disabled={isUpdating}
                        onOpen={async (text) => {
                          setLocalLlmPrompt(text);
                          setIsCustomOverride(true);
                          await handleLlmPromptChange(text);
                        }}
                      />
                    </div>

                    {/* Model Override */}
                    <div className="space-y-1">
                      <label htmlFor={`profile-llm-model-${profile.id}`} className="text-xs text-text/60">
                        {t(
                          "settings.transcriptionProfiles.llmPostProcessing.overrideModel",
                        )}
                      </label>
                      <p className="text-xs text-mid-gray">
                        {t(
                          "settings.transcriptionProfiles.llmPostProcessing.overrideModelHint",
                        )}
                      </p>
                      <div className="flex items-center gap-2">
                        <ModelSelect
                          inputId={`profile-llm-model-${profile.id}`}
                          sortKey={llmModelSortKey}
                          value={profile.llm_model_override || ""}
                          options={llmModelOptions}
                          disabled={isUpdating}
                          placeholder={
                            globalLlmModel
                              ? t(
                                  "settings.transcriptionProfiles.llmPostProcessing.usingGlobalModel",
                                  { model: globalLlmModel },
                                )
                              : t(
                                  "settings.transcriptionProfiles.llmPostProcessing.modelPlaceholder",
                                )
                          }
                          onSelect={(value) =>
                            handleLlmModelChange(value || null)
                          }
                          onCreate={(value) => handleLlmModelChange(value)}
                          onBlur={() => {}}
                          className="flex-1"
                        />
                        <ResetButton
                          onClick={onRefreshModels}
                          disabled={isFetchingModels || isUpdating}
                          title="Fetch Models from Server, then you can type part of model name for searching it in drop-down."
                        >
                          <RefreshCcw
                            className={`w-4 h-4 ${isFetchingModels ? "animate-spin" : ""}`}
                          />
                        </ResetButton>
                      </div>
                    </div>
                  </div>
                )}
              </div>
          </WindowDetails>
            <ChineseScriptFields scopeKey={profile.id} value={profile.chinese_script ?? null} allowInherit={!isDefault}
              descriptionMode="tooltip" disabled={isUpdating} onChange={value => commitFieldsPatch({ chinese_script: value })} />
          </ProfileMoreSettings>
    </div>
  );
};

export default ProfileSettingsForm;
