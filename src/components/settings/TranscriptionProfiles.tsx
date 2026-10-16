import React, { useState, useMemo, useEffect, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Trash2, ChevronDown, ChevronUp, Globe, Check, RefreshCw, Pencil } from "lucide-react";
import { commands, type TranscriptionProfile } from "@/bindings";
import { sessionToast as toast } from "@/lib/sessionToast";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { SettingsGroup } from "../ui/SettingsGroup";
import { SettingContainer } from "../ui/SettingContainer";
import { Button } from "../ui/Button";
import { Input } from "../ui/Input";
import { NameSortControl } from "../ui/NameSortControl";
import { useListSortDirection } from "../../hooks/useListSortPreference";
import { Badge } from "../ui/Badge";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { Tooltip } from "../ui/Tooltip";
import type { TriStateValue } from "../ui/TriStateToggle";
import { Collapse } from "../ui/Collapse";
import { useWindowState } from "../../hooks/useWindowState";
import { HandyShortcut } from "./HandyShortcut";
import type { ModelOption } from "./PostProcessingSettingsApi/types";
import { useSettings } from "../../hooks/useSettings";
import { useSortedDisplayNames } from "../../hooks/useSortedDisplayNames";
import { useModels } from "../../hooks/useModels";
import { LANGUAGES } from "../../lib/constants/languages";
import { useNavigationStore } from "../../stores/navigationStore";
import { GEMINI_LOCALES } from "../../lib/gemini/geminiConfig";
import ProfileSettingsForm, { useProfileSttCapabilities, type ProfileSettingsFormProps, type ProfileValidationReason, type ExtendedTranscriptionProfile, type LlmPromptSourceOption, strictModeToApi, DEFAULT_CLEAN_PROMPT } from "./ProfileSettingsForm";
import { copyDefaultProfileOutput, getGlobalProfileOutputDefaults } from "./ProfileOutputSettings";
import type { ProfileOutputSettings as OutputSettings } from "@/lib/profileOutputSettings";
import { globalSttSelection, sttCatalog, sttProviderId, sttSelectionKey, type SttModelSelection } from "../../lib/sttModelSelection";
import { getNativeDirectOutputModelId, getProfileOutputSummary } from "../../lib/postProcessingAvailability";
import { getLocalizedLanguageLabel } from "./models/ModelMetadataPanel";
import { useSettingsStore } from "../../stores/settingsStore";
import { getTranscriptionProfileAnchorId } from "../../lib/transcriptionProfileNavigation";
import { availableProfileName, getProfileNameError, profileNameErrorFromBackend, type ProfileNameError } from "../../lib/profileNames";
import { cloneProfileSettings, profileSettingsPayload, profileUpdatePayload } from "./profileSettingsPayload";

const profileNameErrorMessage = (reason: ProfileNameError | null, t: ReturnType<typeof useTranslation>["t"]) => {
  if (reason === "required") return t("settings.transcriptionProfiles.nameEmpty", "Enter a profile name.");
  if (reason === "duplicate") return t("settings.transcriptionProfiles.nameDuplicate", "A profile with this name already exists. Choose another name.");
  return null;
};

/** Open Advanced settings section. */
const openAdvancedSettings = () => {
  useNavigationStore.getState().setSection("advanced");
};

const APPLE_PROVIDER_ID = "apple_intelligence";

// Read old window-state drafts once; all new edits use the complete shared profile model.
type LegacyProfileDraftMetadata = {
  sourceName?: string;
  sttModelSelectionOverride: SttModelSelection | null;
  autoShortcutActivation: boolean;
  geminiLanguageCodeOverride: string | null;
  geminiCustomVocabularyOverride: string[] | null;
  chineseScript: TranscriptionProfile["chinese_script"];
  outputSettings: OutputSettings;
};

interface ProfileCardProps extends Omit<ProfileSettingsFormProps, "scope" | "disabled" | "onDraftChange" | "onValidationChange" | "shortcut"> {
  scope: "saved" | "global";
  formKey?: string;
  isExpanded: boolean;
  onToggleExpand: () => void;
  onDelete?: (id: string) => Promise<void>;
  onDuplicate: (profile: ExtendedTranscriptionProfile) => void;
  isCreatingProfile: boolean;
  isActive: boolean;
  onSetActive: (id: string) => Promise<void>;
  allProfileNames: readonly { id: string; name: string }[];
}

const ProfileCard: React.FC<ProfileCardProps> = ({
  profile,
  scope,
  formKey,
  settings,
  onCommit,
  isExpanded,
  onToggleExpand,
  onDelete,
  onDuplicate,
  isCreatingProfile,
  isActive,
  onSetActive,
  llmModelOptions,
  llmModelSortKey,
  globalLlmModel,
  onRefreshModels,
  isFetchingModels,
  defaultLlmPrompt,
  llmPromptSourceOptions,
  globalSonioxLanguageHintsStrict,
  globalGeminiLanguageCode,
  globalGeminiVocabulary,
  localSttModels,
  globalSttModelSelection,
  automaticAppProfilesEnabled,
  allProfileNames,
}) => {
  const { t, i18n } = useTranslation();
  const isDefault = scope === "global";
  const ProfileIcon = isDefault ? RefreshCw : Globe;
  const [isIdentityUpdating, setIsUpdating] = useState(false);
  const [isFormSaving, setIsFormSaving] = useState(false);
  const pendingCommitsRef = useRef(0);
  const commitProfilePatch = async (patch: Partial<ExtendedTranscriptionProfile>) => {
    pendingCommitsRef.current += 1;
    setIsFormSaving(true);
    try { await onCommit(patch); }
    finally {
      pendingCommitsRef.current -= 1;
      if (pendingCommitsRef.current === 0) setIsFormSaving(false);
    }
  };
  const isUpdating = isIdentityUpdating || isFormSaving;
  const [isEditingName, setIsEditingName] = useState(false);
  const [tempName, setTempName] = useState(profile.name);
  const [renameError, setRenameError] = useState<string | null>(null);
  const [renameBackendNameError, setRenameBackendNameError] = useState<ProfileNameError | null>(null);
  const renamePendingRef = useRef(false);
  const cancelRenameRef = useRef(false);
  const renameInvalid = getProfileNameError(tempName, allProfileNames, profile.id) ?? renameBackendNameError;
  const renameNameError = profileNameErrorMessage(renameInvalid, t) ?? renameError;
  const renameErrorId = `profile-name-error-${profile.id}`;
  const effectiveSttSelection = profile.stt_model_selection_override ?? globalSttModelSelection;
  const { isGeminiProvider, supportsTranslation } = useProfileSttCapabilities(effectiveSttSelection, localSttModels);
  const languageLabel = useMemo(() => {
    if (!isGeminiProvider) return LANGUAGES.find(option => option.value === profile.language)?.label ?? profile.language;
    const code = profile.gemini_language_code_override ?? globalGeminiLanguageCode;
    if (code === "auto") return t("settings.gemini.language.auto", "Auto Detect");
    if (code === "os_input") return t("settings.gemini.language.osInput", "Follow OS Input Language");
    const locale = GEMINI_LOCALES.find(([, value]) => value === code);
    return locale ? `${locale[0]} (${code})` : code;
  }, [isGeminiProvider, profile.language, profile.gemini_language_code_override, globalGeminiLanguageCode, t]);
  const modelLabel = useMemo(() => {
    if (!isDefault && profile.stt_model_selection_override == null) {
      return t("settings.transcriptionProfiles.summary.modelFromDefault");
    }
    const model = sttCatalog("dictation", localSttModels).find(option =>
      sttSelectionKey(option.selection) === sttSelectionKey(effectiveSttSelection));
    if (model) return model.localModel ? model.modelLabel : `${model.providerLabel} ${model.modelLabel}`;
    const providerId = sttProviderId(effectiveSttSelection);
    const providerLabel = providerId.charAt(0).toUpperCase() + providerId.slice(1);
    return effectiveSttSelection.model_id
      ? `${effectiveSttSelection.provider === "local" ? "" : `${providerLabel} `}${effectiveSttSelection.model_id}`
      : t("settings.sttModelSelector.selectModel");
  }, [isDefault, profile.stt_model_selection_override, effectiveSttSelection, localSttModels, t]);
  const summaryLanguageCode = isGeminiProvider
    ? profile.gemini_language_code_override ?? globalGeminiLanguageCode
    : profile.language;
  const summaryLanguageLabel = summaryLanguageCode === "auto"
    ? t("settings.general.language.auto")
    : summaryLanguageCode === "os_input"
      ? t("settings.general.language.followOsInput")
      : getLocalizedLanguageLabel(summaryLanguageCode, i18n.resolvedLanguage ?? i18n.language);
  const outputRouteOptions = {
    sttSelection: effectiveSttSelection,
    profilePreviewOutputOnlyEnabled: profile.preview_output_only_enabled ?? false,
    profileOutputSettings: isDefault ? null : profile.output_settings ?? {},
  };
  // Null while the inputs are still loading: show neither the output nor the LLM label.
  const outputSummary = getProfileOutputSummary(settings, outputRouteOptions, localSttModels);
  const outputLabel = outputSummary && t(`settings.transcriptionProfiles.summary.${outputSummary.route === "preview"
    ? "preview" : outputSummary.route === "none" ? "noInsertion" : "insertion"}`);
  const effectiveLlmEnabled = Boolean(profile.llm_post_process_enabled) && Boolean(outputSummary?.llmAvailable);
  const summary = [
    modelLabel,
    `${summaryLanguageLabel}${supportsTranslation && profile.translate_to_english
      ? ` ${t("settings.transcriptionProfiles.toEnglish", "→ EN")}` : ""}`,
    ...(outputLabel ? [outputLabel] : []),
    ...(effectiveLlmEnabled ? ["LLM"] : []),
  ].join(" · ");
  const handleNameChange = async () => {
    if (isDefault || renamePendingRef.current || cancelRenameRef.current || isUpdating) return;
    if (renameInvalid) return;
    if (tempName.trim() === profile.name) {
      setIsEditingName(false);
      return;
    }
    renamePendingRef.current = true;
    setRenameError(null);
    setIsUpdating(true);
    try {
      await commitProfilePatch({ name: tempName.trim() });
      setIsEditingName(false);
    } catch (error) {
      const nameError = profileNameErrorFromBackend(error);
      setRenameBackendNameError(nameError);
      setRenameError(nameError ? null : t("settings.transcriptionProfiles.renameFailed", "Failed to rename profile. Try again."));
    } finally {
      renamePendingRef.current = false;
      setIsUpdating(false);
    }
  };
  const handleDelete = async () => {
    if (isDefault || !onDelete) return;
    setIsUpdating(true);
    try { await onDelete(profile.id); }
    finally { setIsUpdating(false); }
  };
  return (
    <div
      id={getTranscriptionProfileAnchorId(profile.id)}
      tabIndex={-1}
      className={`min-w-0 border rounded-lg transition-colors ${isActive ? "border-purple-500/50 bg-purple-500/5" : "border-mid-gray/30 bg-background/50"}`}
    >
      {/* Header - always visible */}
      <div
        className="flex items-center justify-between gap-3 px-4 py-3 cursor-pointer hover:bg-mid-gray/5 transition-colors"
        data-settings-search-reveal-for={getTranscriptionProfileAnchorId(profile.id)}
        aria-expanded={isExpanded}
        onClick={onToggleExpand}
      >
        <div className="flex flex-1 items-center gap-3 min-w-0">
          <ProfileIcon
            className={`w-4 h-4 shrink-0 ${isActive ? "text-purple-400" : isDefault ? "text-mid-gray" : "text-logo-primary"}`}
          />
          <div className="flex flex-1 flex-col min-w-0">
            <div className="flex flex-wrap items-center gap-2 min-w-0">
              {isDefault ? (
                <span className="font-medium text-sm break-words">
                  {profile.name}
                </span>
              ) : isEditingName ? (
                <div className="flex flex-col gap-1 min-w-0" onClick={(e) => e.stopPropagation()}
                  onBlur={(e) => {
                    if (e.currentTarget.contains(e.relatedTarget as Node | null)) return;
                    void handleNameChange();
                  }}>
                  <div className="flex items-center gap-1 min-w-0">
                  <Input
                    autoFocus
                    value={tempName}
                    onChange={(e) => { setTempName(e.target.value); setRenameError(null); setRenameBackendNameError(null); }}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        e.preventDefault();
                        void handleNameChange();
                      } else if (e.key === "Escape") {
                        e.preventDefault();
                        cancelRenameRef.current = true;
                        setTempName(profile.name);
                        setRenameError(null);
                        setRenameBackendNameError(null);
                        setIsEditingName(false);
                      }
                    }}
                    variant="compact"
                    className="h-6 py-0 px-2 min-w-[120px] text-sm font-medium"
                    onClick={(e) => e.stopPropagation()}
                    disabled={isUpdating}
                    aria-invalid={Boolean(renameNameError)}
                    aria-describedby={renameNameError ? renameErrorId : undefined}
                  />
                  <div className="flex items-center gap-0.5">
                    <Tooltip content={profileNameErrorMessage(renameInvalid, t) ?? ""} disabled={!renameInvalid}
                      className="inline-flex rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-purple-500/50"
                      tabIndex={renameInvalid ? 0 : undefined} triggerRole={renameInvalid ? "button" : undefined}
                      ariaDisabled={renameInvalid ? true : undefined}
                      ariaLabel={renameInvalid ? t("settings.transcriptionProfiles.saveName", "Save profile name") : undefined}>
                    <Button
                      type="button"
                      size="sm"
                      variant="ghost"
                      className={`p-1 h-6 w-6 text-green-400 hover:text-green-300${renameInvalid ? " pointer-events-none" : ""}`}
                      onClick={(e) => {
                        e.stopPropagation();
                        void handleNameChange();
                      }}
                      disabled={isUpdating || Boolean(renameInvalid)}
                      aria-label={t("settings.transcriptionProfiles.saveName", "Save profile name")}
                      aria-hidden={renameInvalid ? true : undefined}
                    >
                      <Check className="w-3 h-3" />
                    </Button>
                    </Tooltip>
                  </div>
                  </div>
                  {renameNameError && <p id={renameErrorId} className="text-xs text-red-400" role="status">{renameNameError}</p>}
                </div>
              ) : (
                <div
                  className="flex items-center gap-2 group/name cursor-pointer min-w-0"
                  onClick={(e) => {
                    e.stopPropagation();
                    setIsEditingName(true);
                    setTempName(profile.name);
                    setRenameError(null);
                    setRenameBackendNameError(null);
                    cancelRenameRef.current = false;
                  }}
                  title={t(
                    "settings.transcriptionProfiles.clickToRename",
                    "Click to rename",
                  )}
                >
                  <span className="font-medium text-sm break-words">
                    {profile.name}
                  </span>
                  <Pencil className="w-3 h-3 text-purple-400/70 opacity-60 group-hover/name:opacity-100 transition-opacity shrink-0" />
                </div>
              )}
              {isActive && (
                <Badge
                  variant="secondary"
                  className="bg-purple-500/20 text-purple-400 border-purple-500/30 text-[10px] px-1.5 py-0 max-w-full truncate"
                >
                  {t("settings.transcriptionProfiles.active")}
                </Badge>
              )}
              {automaticAppProfilesEnabled &&
                (profile.automatic_app_rules?.length ?? 0) > 0 && (
                  <Badge
                    variant="secondary"
                    className="bg-sky-500/15 text-sky-300 border-sky-500/30 text-[10px] px-1.5 py-0 max-w-full truncate"
                  >
                    {t("settings.transcriptionProfiles.badges.automaticApps", {
                      count: profile.automatic_app_rules?.length ?? 0,
                      defaultValue: "Auto: {{count}} app rules",
                    })}
                  </Badge>
                )}
            </div>
            {!isExpanded ? (
              <span className="min-w-0 truncate text-xs leading-5 text-mid-gray" title={summary}>
                {summary}
              </span>
            ) : <span className="text-xs text-mid-gray break-words">
              {languageLabel}
              {supportsTranslation && profile.translate_to_english && (
                <span className="text-purple-400 ml-1">
                  {t("settings.transcriptionProfiles.toEnglish", "→ EN")}
                </span>
              )}
              {(isGeminiProvider ? (profile.gemini_language_code_override ?? globalGeminiLanguageCode) === "os_input" : profile.language === "os_input") && (
                <span
                  className="text-amber-400/60 ml-2 text-[10px]"
                  title={t("settings.general.language.followOsInputTooltip")}
                >
                  ({t("settings.general.language.followOsInputHint")})
                </span>
              )}
            </span>}
          </div>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <Button
            onClick={(e) => {
              e.stopPropagation();
              onDuplicate(profile);
            }}
            disabled={isUpdating || isCreatingProfile}
            variant="secondary"
            size="sm"
          >
            {t("settings.transcriptionProfiles.duplicate", "Duplicate")}
          </Button>
          <Button
            onClick={(e) => {
              e.stopPropagation();
              onSetActive(profile.id);
            }}
            disabled={isActive || isUpdating}
            variant={isActive ? "secondary" : "primary"}
            size="sm"
            className={isActive ? "opacity-100 cursor-default" : ""}
            title={
              isActive ? t("settings.transcriptionProfiles.active") : undefined
            }
          >
            {isActive ? (
              <Check className="w-4 h-4" />
            ) : (
              <span className="text-xs">
                {t("settings.transcriptionProfiles.setActive")}
              </span>
            )}
          </Button>
          {!isDefault && onDelete && (
            <Button
              onClick={(e) => {
                e.stopPropagation();
                handleDelete();
              }}
              variant="secondary"
              size="sm"
              disabled={isUpdating}
              className="text-red-400 hover:text-red-300 hover:border-red-400/50 p-1.5"
            >
              <Trash2 className="w-3.5 h-3.5" />
            </Button>
          )}
          {isExpanded ? (
            <ChevronUp className="w-4 h-4 text-mid-gray" />
          ) : (
            <ChevronDown className="w-4 h-4 text-mid-gray" />
          )}
        </div>
      </div>

      <Collapse open={isExpanded}>
        <div className="px-4 pb-4 pt-3 border-t border-mid-gray/20">
          <ProfileSettingsForm key={formKey ?? profile.id} profile={profile} scope={scope} settings={settings}
            onCommit={commitProfilePatch} disabled={isIdentityUpdating}
            llmModelOptions={llmModelOptions} llmModelSortKey={llmModelSortKey}
            globalLlmModel={globalLlmModel} onRefreshModels={onRefreshModels} isFetchingModels={isFetchingModels}
            defaultLlmPrompt={defaultLlmPrompt} llmPromptSourceOptions={llmPromptSourceOptions}
            globalSonioxLanguageHintsStrict={globalSonioxLanguageHintsStrict}
            globalGeminiLanguageCode={globalGeminiLanguageCode} globalGeminiVocabulary={globalGeminiVocabulary}
            localSttModels={localSttModels} globalSttModelSelection={globalSttModelSelection}
            automaticAppProfilesEnabled={automaticAppProfilesEnabled}
            shortcut={<HandyShortcut shortcutId={`transcribe_${profile.id}`} grouped={true} />} />
        </div>
      </Collapse>
    </div>
  );
};

export const TranscriptionProfiles: React.FC = () => {
  const { t } = useTranslation();
  const {
    settings,
    refreshSettings,
    updateSetting,
    updateBinding,
    isUpdating: isSettingUpdating,
    postProcessModelOptions,
    fetchPostProcessModels,
  } = useSettings();
  const { models: localSttModels, getModelInfo } = useModels();
  const [profileSortDirection, setProfileSortDirection] =
    useListSortDirection("transcription-profiles");
  const [expandedIds, setExpandedIds] = useWindowState<Set<string>>(
    "transcription-profiles-expanded",
    () => new Set(["default"]),
  );
  const [automaticAppsExpanded, setAutomaticAppsExpanded] =
    useWindowState("transcription-automatic-apps-expanded", false);
  const [automaticAppsManuallyCollapsed, setAutomaticAppsManuallyCollapsed] =
    useWindowState("transcription-automatic-apps-manually-collapsed", false);
  const [isCreating, setIsCreating] = useWindowState("profile-new-creating", false);
  const [isFetchingModels, setIsFetchingModels] = useState(false);
  const [newName, setNewName] = useWindowState("profile-new-name", "");
  const [newLanguage, setNewLanguage] = useWindowState("profile-new-language", "auto");
  const [newTranslate, setNewTranslate] = useWindowState("profile-new-translate", false);
  const [newSystemPrompt, setNewSystemPrompt] = useWindowState("profile-new-system-prompt", "");
  const [newPushToTalk, setNewPushToTalk] = useWindowState("profile-new-push-to-talk", true);
  const [newPreviewOutputOnly, setNewPreviewOutputOnly] = useWindowState("profile-new-preview", false);
  const [newIncludeInCycle, setNewIncludeInCycle] = useWindowState("profile-new-cycle", true);
  const [newShortcut, setNewShortcut] = useWindowState("profile-new-shortcut", "");
  const [newSttPromptOverrideEnabled, setNewSttPromptOverrideEnabled] =
    useWindowState("profile-new-stt-prompt-override", false);
  const [newLlmEnabledOverride, setNewLlmEnabledOverride] = useWindowState<
    boolean | null
  >("profile-new-llm-enabled", null);
  const [newLlmPromptOverride, setNewLlmPromptOverride] = useWindowState("profile-new-llm-prompt", "");
  const [newLlmPromptExplicitEmpty, setNewLlmPromptExplicitEmpty] = useWindowState("profile-new-llm-explicit-empty", false);
  const [newLlmModelOverride, setNewLlmModelOverride] = useWindowState<string | null>(
    "profile-new-llm-model", null,
  );
  const [newSonioxContextGeneralJson, setNewSonioxContextGeneralJson] =
    useWindowState("profile-new-soniox-general", "");
  const [newSonioxContextText, setNewSonioxContextText] = useWindowState("profile-new-soniox-text", "");
  const [newSonioxContextTerms, setNewSonioxContextTerms] = useWindowState<string[]>(
    "profile-new-soniox-terms", [],
  );
  const [
    newSonioxLanguageHintsStrictMode,
    setNewSonioxLanguageHintsStrictMode,
  ] = useWindowState<TriStateValue>("profile-new-soniox-strict", "global");
  const [draftAdditionalSettings, setDraftAdditionalSettings] = useWindowState<LegacyProfileDraftMetadata | null>(
    "profile-new-duplicate-settings", null,
  );
  const [duplicateSourceName, setDuplicateSourceName] = useWindowState("profile-new-duplicate-source", draftAdditionalSettings?.sourceName ?? "");
  const [draftRevision, setDraftRevision] = useWindowState("profile-new-form-revision", 0);
  const [newFormValidationReasons, setNewFormValidationReasons] = useState<ProfileValidationReason[]>([]);
  const newFormInvalid = newFormValidationReasons.length > 0;
  const [newFormValues, setNewFormValues] = useWindowState<Partial<ExtendedTranscriptionProfile>>("profile-new-settings-form", {});
  const profileCommitQueues = useRef(new Map<string, Promise<void>>());
  const [newAutomaticAppRules, setNewAutomaticAppRules] = useWindowState("profile-new-app-rules", "");
  const [newProfileError, setNewProfileError] = useState<string | null>(null);
  const [newBackendNameError, setNewBackendNameError] = useState<ProfileNameError | null>(null);
  const globalGeminiVocabulary = ((settings as any)?.gemini_custom_vocabulary ?? []) as string[];
  const profiles = (settings?.transcription_profiles ||
    []) as ExtendedTranscriptionProfile[];
  const { items: visibleProfiles } = useSortedDisplayNames(
    profiles,
    (profile) => profile.name,
    profileSortDirection,
  );
  const globalSttModelSelection = globalSttSelection(settings);
  const newLlmEnabled =
    newLlmEnabledOverride ?? Boolean(settings?.post_process_enabled);

  const isExpanded = (id: string) => expandedIds.has(id);

  const toggleExpanded = (id: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const activeProfileId = (settings as any)?.active_profile_id || "default";
  const overlayEnabled =
    (settings as any)?.profile_switch_overlay_enabled ?? true;

  // Listen for active profile changes from shortcuts
  useEffect(() => {
    const unlistenPromise = listen("active-profile-changed", () => {
      refreshSettings();
    });

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [refreshSettings]);

  // Compute the active model ID based on provider - used for prompt limits and prompt storage
  const activeModelId = useMemo(() => {
    const provider = String(settings?.transcription_provider || "local");
    if (provider === "remote_openai_compatible") {
      return settings?.remote_stt?.model_id || "";
    }
    if (provider === "remote_soniox" || provider === "remote_deepgram") {
      return "";
    }
    return settings?.selected_model || "";
  }, [
    settings?.transcription_provider,
    settings?.remote_stt?.model_id,
    settings?.selected_model,
  ]);

  const hasActivePromptModel = activeModelId.length > 0;
  const activePromptValue = hasActivePromptModel
    ? settings?.transcription_prompts?.[activeModelId] || ""
    : "";
  // Native direct output wins over Preview, so turning a profile's Preview on turns off
  // direct output for the model that profile records with.
  const disableNativeDirectOutputForPreview = async (sttOverride: SttModelSelection | null | undefined) => {
    const modelId = getNativeDirectOutputModelId(settings, sttOverride, getModelInfo);
    if (!modelId) return;
    const result = await commands.changeNativeStreamingLiveOutputModelSetting(modelId, false);
    if (result.status === "error") throw new Error(result.error);
    toast.info(t("modelSelector.nativeLiveOutput.directDisabledTitle"), {
      description: t("modelSelector.nativeLiveOutput.directDisabledDescription", {
        model: getModelInfo(modelId)?.name || modelId,
      }),
    });
  };
  const globalGeminiLanguageCode = String((settings as any)?.gemini_language_code ?? "auto");
  // LLM Post-Processing configuration
  const currentLlmProviderId = settings?.post_process_provider_id || "";

  const llmModelOptions = useMemo<ModelOption[]>(() => {
    const models = postProcessModelOptions[currentLlmProviderId] || [];
    return models.map((m) => ({ value: m, label: m }));
  }, [postProcessModelOptions, currentLlmProviderId]);
  const llmModelSortKey = `profile-llm-models:${currentLlmProviderId}`;

  const globalLlmModel =
    settings?.post_process_models?.[currentLlmProviderId] || "";

  const globalPromptText = useMemo(() => {
    const prompts = settings?.post_process_prompts || [];
    const selectedId = settings?.post_process_selected_prompt_id;
    return (
      prompts.find((p) => p.id === selectedId)?.prompt || DEFAULT_CLEAN_PROMPT
    );
  }, [
    settings?.post_process_prompts,
    settings?.post_process_selected_prompt_id,
  ]);

  const llmPromptSourceOptions = useMemo<LlmPromptSourceOption[]>(
    () =>
      (settings?.post_process_prompts ?? [])
        .filter((prompt) => prompt.prompt.trim().length > 0)
        .map((prompt) => ({
          value: prompt.id,
          label:
            prompt.name.trim() ||
            t(
              "settings.transcriptionProfiles.llmPostProcessing.untitledPrompt",
              "Untitled prompt",
            ),
          prompt: prompt.prompt,
        })),
    [settings?.post_process_prompts, t],
  );

  const handleRefreshModels = useCallback(async () => {
    if (!currentLlmProviderId || currentLlmProviderId === APPLE_PROVIDER_ID)
      return;
    setIsFetchingModels(true);
    try {
      await fetchPostProcessModels(currentLlmProviderId);
    } catch (error) {
      console.error("Failed to fetch models:", error);
    } finally {
      setIsFetchingModels(false);
    }
  }, [currentLlmProviderId, fetchPostProcessModels]);

  const newProfileDraft: ExtendedTranscriptionProfile = {
    id: `new-${draftRevision}`, name: newName, language: newLanguage,
    translate_to_english: newTranslate, automatic_app_rules: newAutomaticAppRules.split(/\r?\n/).map(rule => rule.trim()).filter(Boolean),
    system_prompt: newSystemPrompt, stt_prompt_override_enabled: newSttPromptOverrideEnabled,
    stt_model_selection_override: draftAdditionalSettings?.sttModelSelectionOverride ?? null,
    push_to_talk: newPushToTalk, auto_shortcut_activation: draftAdditionalSettings?.autoShortcutActivation ?? false,
    preview_output_only_enabled: newPreviewOutputOnly, include_in_cycle: newIncludeInCycle,
    llm_post_process_enabled: newLlmEnabled,
    llm_prompt_override: newLlmPromptExplicitEmpty ? "" : newLlmPromptOverride || null,
    llm_model_override: newLlmModelOverride,
    llm_live_stream_insert: settings?.post_process_live_stream_insert ?? false,
    soniox_context_general_json: newSonioxContextGeneralJson, soniox_context_text: newSonioxContextText,
    soniox_context_terms: newSonioxContextTerms, soniox_language_hints_strict: strictModeToApi(newSonioxLanguageHintsStrictMode),
    gemini_language_code_override: draftAdditionalSettings?.geminiLanguageCodeOverride ?? null,
    gemini_custom_vocabulary_override: draftAdditionalSettings?.geminiCustomVocabularyOverride ?? null,
    chinese_script: draftAdditionalSettings?.chineseScript ?? null,
    // A new profile starts as a copy of the Default profile's output (without key presses).
    output_settings: draftAdditionalSettings?.outputSettings ?? copyDefaultProfileOutput(getGlobalProfileOutputDefaults(settings)),
    ...newFormValues,
  };
  // Blur commits and Create can occur in one event batch; keep the exact current draft synchronously.
  const newProfileDraftRef = useRef(newProfileDraft);
  newProfileDraftRef.current = newProfileDraft;
  const applyNewDraftPatch = (patch: Partial<ExtendedTranscriptionProfile>) => {
    const next = { ...newProfileDraftRef.current, ...patch,
      output_settings: { ...newProfileDraftRef.current.output_settings, ...patch.output_settings } };
    newProfileDraftRef.current = next;
    setNewFormValues(previous => ({ ...previous, ...patch,
      ...(patch.output_settings ? { output_settings: next.output_settings } : {}),
    }));
  };
  const commitNewDraftPatch = async (patch: Partial<ExtendedTranscriptionProfile>) => applyNewDraftPatch(patch);
  const globalProfile: ExtendedTranscriptionProfile = {
    id: "default", name: t("settings.transcriptionProfiles.defaultProfile"),
    language: settings?.selected_language ?? "auto", translate_to_english: settings?.translate_to_english ?? false,
    automatic_app_rules: [], include_in_cycle: false,
    push_to_talk: settings?.push_to_talk ?? true, auto_shortcut_activation: settings?.auto_shortcut_activation ?? false,
    preview_output_only_enabled: settings?.preview_output_only_enabled ?? false,
    system_prompt: activePromptValue, stt_prompt_override_enabled: true, stt_model_selection_override: null,
    llm_post_process_enabled: settings?.post_process_enabled ?? false, llm_prompt_override: null, llm_model_override: null,
    llm_live_stream_insert: settings?.post_process_live_stream_insert ?? false,
    soniox_context_general_json: settings?.soniox_context_general_json ?? "", soniox_context_text: settings?.soniox_context_text ?? "",
    soniox_context_terms: settings?.soniox_context_terms ?? [], soniox_language_hints_strict: settings?.soniox_language_hints_strict ?? false,
    gemini_language_code_override: globalGeminiLanguageCode, gemini_custom_vocabulary_override: globalGeminiVocabulary,
    chinese_script: settings?.chinese_script ?? "as_transcribed", output_settings: getGlobalProfileOutputDefaults(settings),
  };
  const sharedFormProps = {
    settings, llmModelOptions, llmModelSortKey, globalLlmModel, onRefreshModels: handleRefreshModels, isFetchingModels,
    defaultLlmPrompt: globalPromptText, llmPromptSourceOptions,
    globalSonioxLanguageHintsStrict: Boolean(settings?.soniox_language_hints_strict),
    globalGeminiLanguageCode, globalGeminiVocabulary, localSttModels, globalSttModelSelection,
    automaticAppProfilesEnabled: Boolean(settings?.automatic_app_profiles_enabled),
  };

  const newNameError = profileNameErrorMessage(getProfileNameError(newName, profiles) ?? newBackendNameError, t);
  const createUnavailableReasons = isCreating
    ? [t("settings.transcriptionProfiles.creatingProfile", "Creating profile…")]
    : [
        ...(newNameError ? [newNameError] : []),
        ...newFormValidationReasons.map(reason => {
          switch (reason) {
            case "system_prompt": return t("settings.transcriptionProfiles.shortenSystemPrompt", "Shorten the voice model prompt.");
            case "gemini_vocabulary": return t("settings.transcriptionProfiles.fixGeminiVocabulary", "Fix the Gemini vocabulary.");
            case "soniox_context": return t("settings.transcriptionProfiles.fixSonioxContext", "Fix the Soniox context.");
            case "output_delay": return t("profileOutput.delayInvalid", "Enter a number from 0 to 5000 ms.");
          }
        }),
      ];
  const createDisabled = createUnavailableReasons.length > 0;

  const clearNewProfile = () => {
    setNewName("");
    setNewLanguage("auto");
    setNewTranslate(false);
    setNewSystemPrompt("");
    setNewSttPromptOverrideEnabled(false);
    setNewPushToTalk(true);
    setNewPreviewOutputOnly(false);
    setNewIncludeInCycle(true);
    setNewShortcut("");
    setNewLlmEnabledOverride(null);
    setNewLlmPromptOverride("");
    setNewLlmPromptExplicitEmpty(false);
    setNewLlmModelOverride(null);
    setNewSonioxContextGeneralJson("");
    setNewSonioxContextText("");
    setNewSonioxContextTerms([]);
    setNewSonioxLanguageHintsStrictMode("global");
    setNewAutomaticAppRules("");
    setDraftAdditionalSettings(null);
    setNewFormValues({});
    setDuplicateSourceName("");
    setDraftRevision(revision => revision + 1);
    setNewFormValidationReasons([]);
    setNewProfileError(null);
    setNewBackendNameError(null);
  };

  const hasNewProfileDraft = Boolean(
    newName || newLanguage !== "auto" || newTranslate || newSystemPrompt ||
    !newPushToTalk || newPreviewOutputOnly || !newIncludeInCycle || newShortcut ||
    newSttPromptOverrideEnabled || newLlmEnabledOverride !== null ||
    newLlmPromptOverride || newLlmPromptExplicitEmpty || newLlmModelOverride !== null ||
    newSonioxContextGeneralJson || newSonioxContextText || newSonioxContextTerms.length ||
    newSonioxLanguageHintsStrictMode !== "global" || newAutomaticAppRules || draftAdditionalSettings || Object.keys(newFormValues).length,
  );

  const handleDuplicate = (profile: ExtendedTranscriptionProfile) => {
    if (isCreating) return;
    setNewName(availableProfileName(t("settings.transcriptionProfiles.copyName", { name: profile.name, defaultValue: "{{name}} — copy" }), profiles));
    setNewFormValues({ ...cloneProfileSettings(profile), ...(profile.id === "default" ? { include_in_cycle: true } : {}) });
    setDuplicateSourceName(profile.name);
    setDraftRevision(revision => revision + 1);
    setNewFormValidationReasons([]);
    setNewProfileError(null);
    setNewBackendNameError(null);
    requestAnimationFrame(() => {
      document.getElementById("settings-create-profile")?.scrollIntoView({ block: "start", behavior: "smooth" });
      requestAnimationFrame(() => {
        const input = document.getElementById("new-profile-name") as HTMLInputElement | null;
        input?.focus({ preventScroll: true });
        input?.select();
      });
    });
  };

  const handleCreate = async () => {
    if (isCreating || newNameError) return;
    if (newFormInvalid) return;
    const draft = newProfileDraftRef.current;
    setNewProfileError(null);
    setIsCreating(true);
    try {
      const newProfile = await invoke<TranscriptionProfile>(
        "add_transcription_profile",
        {
          payload: {
            name: newName.trim(), ...profileSettingsPayload(draft),
            llmSettings: !["llm_post_process_enabled", "llm_prompt_override", "llm_model_override", "llm_live_stream_insert"].some(field => field in newFormValues) && newLlmEnabledOverride === null &&
              draft.llm_prompt_override === null && draft.llm_model_override === null
                ? null : profileSettingsPayload(draft).llmSettings,
          },
        },
      );
      await refreshSettings();
      if (newShortcut) {
        // The binding exists only after creation; a rejected key keeps the profile.
        try {
          await updateBinding(`transcribe_${newProfile.id}`, newShortcut);
        } catch (error) {
          toast.error(t("settings.transcriptionProfiles.shortcutAssignFailed", {
            error: error instanceof Error ? error.message : String(error),
          }));
        }
      }
      clearNewProfile();
      setExpandedIds((prev) => {
        const next = new Set(prev);
        next.add(newProfile.id);
        return next;
      });
    } catch (error) {
      console.error("Failed to create profile:", error);
      const nameError = profileNameErrorFromBackend(error);
      setNewBackendNameError(nameError);
      setNewProfileError(
        nameError ? null : error instanceof Error
          ? error.message
          : t(
              "settings.transcriptionProfiles.createFailed",
              "Failed to create profile.",
            ),
      );
    } finally {
      setIsCreating(false);
    }
  };

  const handleUpdate = async (profile: ExtendedTranscriptionProfile) => {
    try {
      if (profile.id === activeProfileId && (profile.preview_output_only_enabled ?? false)) {
        await disableNativeDirectOutputForPreview(profile.stt_model_selection_override);
      }
      await invoke("update_transcription_profile", { payload: profileUpdatePayload(profile) });
      await refreshSettings();
    } catch (error) {
      console.error("Failed to update profile:", error);
      throw error;
    }
  };

  const queueProfileCommit = (id: string, commit: () => Promise<void>) => {
    const previous = profileCommitQueues.current.get(id) ?? Promise.resolve();
    const pending = previous.catch(() => {}).then(commit);
    profileCommitQueues.current.set(id, pending);
    return pending;
  };
  const handleSavedProfileCommit = (id: string, patch: Partial<ExtendedTranscriptionProfile>) => queueProfileCommit(id, async () => {
    const latest = useSettingsStore.getState().settings?.transcription_profiles?.find(profile => profile.id === id) as ExtendedTranscriptionProfile | undefined;
    if (!latest) throw new Error("Profile no longer exists.");
    try {
      if (patch.output_settings) await invoke("change_transcription_output_settings", {
        profileId: id, output: { ...latest.output_settings, ...patch.output_settings },
      });
      if ("chinese_script" in patch) await invoke("change_chinese_script_setting", { profileId: id, script: patch.chinese_script ?? null });
      const { output_settings: _output, chinese_script: _script, ...settingsPatch } = patch;
      if (Object.keys(settingsPatch).length) {
        const current = useSettingsStore.getState().settings?.transcription_profiles?.find(profile => profile.id === id) as ExtendedTranscriptionProfile;
        await handleUpdate({ ...current, ...settingsPatch });
      } else await refreshSettings();
    } catch (error) {
      const nameError = profileNameErrorFromBackend(error);
      if (!nameError || !("name" in patch)) toast.error(profileNameErrorMessage(nameError, t) ?? String(error));
      throw error;
    }
  });
  const handleGlobalProfileCommit = (patch: Partial<ExtendedTranscriptionProfile>) => queueProfileCommit("default", async () => {
    try {
      if (patch.output_settings) await invoke("change_transcription_output_settings", {
        profileId: "default", output: { ...getGlobalProfileOutputDefaults(useSettingsStore.getState().settings), ...patch.output_settings },
      });
      if ("chinese_script" in patch) await updateSetting("chinese_script", patch.chinese_script ?? "as_transcribed", { throwOnError: true });
      if ("system_prompt" in patch) {
        if (!activeModelId) return;
        const result = await commands.changeTranscriptionPromptSetting(activeModelId, patch.system_prompt ?? "");
        if (result.status === "error") throw new Error(String(result.error));
      }
      if ("llm_post_process_enabled" in patch) await invoke("change_default_profile_post_process_enabled_setting", { enabled: patch.llm_post_process_enabled });
      const globalFields = {
        language: "selected_language", translate_to_english: "translate_to_english", push_to_talk: "push_to_talk",
        auto_shortcut_activation: "auto_shortcut_activation", preview_output_only_enabled: "preview_output_only_enabled",
        soniox_language_hints_strict: "soniox_language_hints_strict", soniox_context_general_json: "soniox_context_general_json",
        soniox_context_text: "soniox_context_text", soniox_context_terms: "soniox_context_terms",
        gemini_language_code_override: "gemini_language_code", gemini_custom_vocabulary_override: "gemini_custom_vocabulary",
        llm_live_stream_insert: "post_process_live_stream_insert",
      } as const;
      for (const [field, setting] of Object.entries(globalFields)) {
        if (field in patch) await updateSetting(setting as keyof NonNullable<typeof settings>, patch[field as keyof ExtendedTranscriptionProfile] as never, { throwOnError: true });
      }
      if ("preview_output_only_enabled" in patch && activeProfileId === "default") {
        await invoke("change_soniox_live_preview_enabled_setting", { enabled: patch.preview_output_only_enabled });
      }
      await refreshSettings();
    } catch (error) {
      await refreshSettings();
      toast.error(String(error));
      throw error;
    }
  });

  const handleDelete = async (id: string) => {
    try {
      const result = await commands.deleteTranscriptionProfile(id);
      if (result.status === "error") throw new Error(String(result.error));
      await refreshSettings();
      setExpandedIds((prev) => {
        if (!prev.has(id)) return prev;
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    } catch (error) {
      console.error("Failed to delete profile:", error);
      toast.error(
        t(
          "settings.transcriptionProfiles.deleteFailed",
          "Could not delete profile.",
        ),
        {
          description: error instanceof Error ? error.message : String(error),
        },
      );
    }
  };

  const handleSetActive = async (id: string) => {
    try {
      let previewEnabled: boolean;
      let sttOverride: SttModelSelection | null | undefined;
      if (id === "default") {
        previewEnabled = (settings as any)?.preview_output_only_enabled ?? false;
      } else {
        const targetProfile = profiles.find((p) => p.id === id);
        previewEnabled = targetProfile?.preview_output_only_enabled ?? false;
        sttOverride = targetProfile?.stt_model_selection_override;
      }
      if (previewEnabled) {
        await disableNativeDirectOutputForPreview(sttOverride);
      }
      await invoke("set_active_profile", { id });
      await refreshSettings();
    } catch (e) {
      console.error("Failed to set active profile", e);
    }
  };

  const handleOverlayChange = async (enabled: boolean) => {
    if (updateSetting) {
      await updateSetting("profile_switch_overlay_enabled" as any, enabled);
    }
  };

  const automaticAppProfilesEnabled = Boolean(
    settings?.automatic_app_profiles_enabled,
  );
  const recordingOverlayShowApp = Boolean(
    settings?.recording_overlay_show_app,
  );
  const previousAutomaticAppProfilesEnabled = useRef(automaticAppProfilesEnabled);

  useEffect(() => {
    if (automaticAppProfilesEnabled) {
      if (!automaticAppsManuallyCollapsed) {
        setAutomaticAppsExpanded(true);
      }
    } else if (previousAutomaticAppProfilesEnabled.current) {
      setAutomaticAppsManuallyCollapsed(false);
      setAutomaticAppsExpanded(false);
    }
    previousAutomaticAppProfilesEnabled.current = automaticAppProfilesEnabled;
  }, [automaticAppProfilesEnabled, automaticAppsManuallyCollapsed, setAutomaticAppsExpanded, setAutomaticAppsManuallyCollapsed]);

  const handleAutomaticAppsDisclosureToggle = (
    event: React.SyntheticEvent<HTMLDetailsElement>,
  ) => {
    const expanded = event.currentTarget.open;
    setAutomaticAppsManuallyCollapsed(automaticAppProfilesEnabled && !expanded);
    setAutomaticAppsExpanded(expanded);
  };

  const handleAutomaticAppProfilesToggle = async (enabled: boolean) => {
    if (updateSetting) {
      try {
        await updateSetting("automatic_app_profiles_enabled", enabled, {
          throwOnError: true,
        });
      } catch (error) {
        toast.error(String(error));
      } finally {
        // The backend changes the dependent overlay setting in the same write.
        // Refresh the complete snapshot so the optimistic store reflects it too.
        await refreshSettings();
      }
    }
  };

  const handleRecordingOverlayShowAppToggle = async (enabled: boolean) => {
    if (updateSetting) {
      try {
        await updateSetting("recording_overlay_show_app", enabled, {
          throwOnError: true,
        });
      } catch (error) {
        toast.error(String(error));
      }
    }
  };

  return (
    <div className="space-y-4">
      <SettingsGroup title={t("settings.transcriptionProfiles.title")}>
      {/* Help text */}
      <SettingContainer
        title=""
        description=""
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <div className="p-3 bg-purple-500/10 border border-purple-500/30 rounded-lg">
          <p className="text-sm text-text/80">
            {t("settings.transcriptionProfiles.help")}
          </p>
        </div>
      </SettingContainer>

      {/* Transcribe with Active Profile - MAIN SHORTCUT */}
      <SettingContainer
        title=""
        description=""
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <div className="p-3 bg-gradient-to-r from-purple-500/15 to-pink-500/10 border border-purple-500/40 rounded-lg">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex flex-col gap-1 min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <span className="text-sm font-semibold text-text">
                  {t("settings.transcriptionProfiles.transcribeActiveProfile")}
                </span>
                <Badge
                  variant="secondary"
                  className="bg-purple-500/20 text-purple-400 border-purple-500/30 text-[10px] px-1.5 py-0"
                >
                  {activeProfileId === "default"
                    ? t("settings.transcriptionProfiles.defaultProfile")
                    : profiles.find((p) => p.id === activeProfileId)?.name ||
                      activeProfileId}
                </Badge>
              </div>
              <span className="text-xs text-mid-gray">
                {t(
                  "settings.transcriptionProfiles.transcribeActiveProfileDescription",
                )}
              </span>
            </div>
            <div className="shrink-0">
              <HandyShortcut shortcutId="transcribe" />
            </div>
          </div>
        </div>
      </SettingContainer>

      {/* General Settings: Cycle Shortcut & Overlay */}
      <SettingContainer
        title="General Settings"
        description=""
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <div className="space-y-3">
          {/* Cycle Shortcut */}
          <div className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center">
            <div className="flex flex-col min-w-0">
              <span className="text-sm font-medium">
                {t("settings.transcriptionProfiles.includeInCycleDescription")}
              </span>
              <span className="text-xs text-mid-gray leading-snug">
                {t(
                  "settings.transcriptionProfiles.cycleActiveProfilesShort",
                  "Global shortcut to cycle through active profiles",
                )}
              </span>
            </div>
            <div className="shrink-0">
              <HandyShortcut shortcutId="cycle_profile" />
            </div>
          </div>

          {/* Overlay Toggle */}
          <div className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center">
            <div className="flex flex-col min-w-0">
              <span id="settings-profile-switch-overlay" tabIndex={-1} className="text-sm font-medium">
                {t("settings.transcriptionProfiles.showOverlayOnSwitch")}
              </span>
              <span className="text-xs text-mid-gray leading-snug">
                {t(
                  "settings.transcriptionProfiles.showOverlayOnSwitchDescription",
                )}
              </span>
            </div>
            <div className="shrink-0">
              <ToggleSwitch
                checked={overlayEnabled}
                onChange={handleOverlayChange}
              />
            </div>
          </div>
        </div>
      </SettingContainer>

      <SettingContainer
        id="settings-manage-profiles"
        title={
          <span className="flex min-w-0 flex-wrap items-baseline justify-between gap-2">
            <span>{t("settings.transcriptionProfiles.existingProfiles")}</span>
            <NameSortControl
              direction={profileSortDirection}
              onChange={setProfileSortDirection}
              ariaLabel={t(
                "listSorting.profilesLabel",
                "Sort profiles by name",
              )}
            />
          </span>
        }
        description=""
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <div className="space-y-3">
          {/* Default Profile Card - shared card with global settings */}
          <ProfileCard {...sharedFormProps} profile={globalProfile} scope="global"
            allProfileNames={profiles}
            formKey={`default-${activeModelId}`}
            isExpanded={isExpanded("default")} onToggleExpand={() => toggleExpanded("default")}
            onCommit={handleGlobalProfileCommit}
            onDuplicate={handleDuplicate} isCreatingProfile={isCreating}
            isActive={activeProfileId === "default"} onSetActive={handleSetActive} />

          {/* Custom Profiles */}
          {visibleProfiles.map(profile => (
            <ProfileCard key={profile.id} {...sharedFormProps} profile={profile} scope="saved"
              allProfileNames={profiles}
              isExpanded={isExpanded(profile.id)} onToggleExpand={() => toggleExpanded(profile.id)}
              onCommit={patch => handleSavedProfileCommit(profile.id, patch)}
              onDelete={handleDelete} onDuplicate={handleDuplicate} isCreatingProfile={isCreating}
              isActive={activeProfileId === profile.id} onSetActive={handleSetActive} />
          ))}
        </div>
      </SettingContainer>
      </SettingsGroup>

      {/* Application-Aware Profiles Settings */}
      <details
        id="settings-automatic-app-profiles"
        tabIndex={-1}
        open={automaticAppsExpanded}
        onToggle={handleAutomaticAppsDisclosureToggle}
        className="group glass-panel-subtle rounded-xl overflow-visible border border-white/[0.08] transition-colors open:border-purple-500/30"
      >
        <summary className="flex cursor-pointer list-none items-center justify-between gap-3 rounded-xl px-5 py-4 text-left transition-colors hover:bg-white/[0.03] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#ff4d8d]/35 [&::-webkit-details-marker]:hidden">
          <span className="flex min-w-0 flex-wrap items-center gap-x-1.5 gap-y-1 text-sm font-semibold text-text">
            {automaticAppProfilesEnabled && !automaticAppsExpanded && (
              <span
                className="h-2 w-2 shrink-0 rounded-full bg-emerald-400 shadow-[0_0_7px_rgba(52,211,153,0.8)]"
                aria-hidden="true"
              />
            )}
            <span>
              {t(
                "settings.transcriptionProfiles.automaticApps.sectionTitle",
                "I want to have a different profile for different applications.",
              )}
            </span>
            {!automaticAppProfilesEnabled && (
              <span className="text-mid-gray">
                ({t(
                  "settings.transcriptionProfiles.automaticApps.clickIfYes",
                  "click if yes",
                )})
              </span>
            )}
            {automaticAppProfilesEnabled && !automaticAppsExpanded && (
              <span className="text-emerald-400">
                ({t(
                  "settings.transcriptionProfiles.automaticApps.enabledStatus",
                  "enabled",
                )})
              </span>
            )}
          </span>
          <ChevronDown
            className="h-4 w-4 shrink-0 text-mid-gray transition-transform group-open:rotate-180"
            aria-hidden="true"
          />
        </summary>
        <div className="space-y-4 border-t border-white/[0.05] px-5 py-4">
          <div className="p-3 bg-sky-500/10 border border-sky-500/30 rounded-lg">
            <p className="text-xs text-text/80 leading-relaxed">
              {t(
                "settings.transcriptionProfiles.automaticApps.sectionHelp",
                "Turn this on, then open any profile above: it gets a new blue box, “Automatically use this profile in applications”, where you list the applications for that profile, for example chrome.exe. When you start dictating in one of them, that profile is used automatically. Other applications use the profile you selected manually. While this is off, AivoRelay does not check which application is active.",
              )}
            </p>
          </div>

          {/* Master Toggle */}
          <div className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center">
            <div className="flex flex-col min-w-0">
              <span className="text-sm font-medium">
                {t(
                  "settings.transcriptionProfiles.automaticApps.masterToggle",
                  "Different applications to use different profiles",
                )}
              </span>
              <span className="text-xs text-mid-gray leading-snug">
                {t(
                  "settings.transcriptionProfiles.automaticApps.masterToggleDescription",
                  "Enable application detection for the main Transcribe hotkey. The floating Voice Activation Button continues to use the profile selected manually.",
                )}
              </span>
            </div>
            <div className="shrink-0">
              <ToggleSwitch
                checked={automaticAppProfilesEnabled}
                onChange={handleAutomaticAppProfilesToggle}
                isUpdating={isSettingUpdating(
                  "automatic_app_profiles_enabled",
                )}
                ariaLabel={t(
                  "settings.transcriptionProfiles.automaticApps.masterToggle",
                  "Different applications to use different profiles",
                )}
              />
            </div>
          </div>

          {/* Show App in Overlay Toggle (dependent) */}
          {automaticAppProfilesEnabled && (
            <>
              <div className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center pt-2 border-t border-mid-gray/10">
                <div className="flex flex-col min-w-0">
                  <span className="text-sm font-medium">
                    {t(
                      "settings.transcriptionProfiles.automaticApps.showAppInOverlay",
                      "Show active application in recording overlay",
                    )}
                  </span>
                  <span className="text-xs text-mid-gray leading-snug">
                    {t(
                      "settings.transcriptionProfiles.automaticApps.showAppInOverlayDescription",
                      "Show the active program name in a compact pill above the recording visualization.",
                    )}
                  </span>
                </div>
                <div className="shrink-0">
                  <ToggleSwitch
                    checked={recordingOverlayShowApp}
                    onChange={handleRecordingOverlayShowAppToggle}
                    disabled={isSettingUpdating(
                      "automatic_app_profiles_enabled",
                    )}
                    isUpdating={isSettingUpdating(
                      "recording_overlay_show_app",
                    )}
                    ariaLabel={t(
                      "settings.transcriptionProfiles.automaticApps.showAppInOverlay",
                      "Show active application in recording overlay",
                    )}
                  />
                </div>
              </div>

              {/* Admin UIPI Notice */}
              <div className="p-3 bg-amber-500/10 border border-amber-500/30 rounded-lg text-xs text-amber-200/90 leading-relaxed">
                <span className="font-semibold text-amber-300">
                  {t(
                    "settings.transcriptionProfiles.automaticApps.adminNoticeTitle",
                    "Administrator windows:",
                  )}{" "}
                </span>
                {t(
                  "settings.transcriptionProfiles.automaticApps.adminNoticePrefix",
                  "Windows User Interface Privilege Isolation (UIPI) prevents dictation from typing into administrator/elevated windows unless AivoRelay is also running as administrator. You can configure ",
                )}
                <button
                  type="button"
                  onClick={openAdvancedSettings}
                  className="underline hover:text-amber-100 font-medium cursor-pointer transition-colors"
                >
                  {t(
                    "settings.transcriptionProfiles.automaticApps.adminNoticeLink",
                    "Autostart with administrator privileges in Advanced settings",
                  )}
                </button>{" "}
                <span className="font-semibold text-amber-300">
                  {t(
                    "settings.transcriptionProfiles.automaticApps.adminNoticeNotRecommended",
                    "(NOT recommended)",
                  )}
                </span>
                .
              </div>
            </>
          )}
        </div>
      </details>

      <SettingsGroup>
        {/* Create new profile */}
        <SettingContainer
          id="settings-create-profile"
        title={t("settings.transcriptionProfiles.createNew")}
        description={t("settings.transcriptionProfiles.createNewDescription")}
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <div className="space-y-3 p-3 border border-dashed border-mid-gray/30 rounded-lg overflow-visible">
          {hasNewProfileDraft && (
            <div className="flex items-center justify-between gap-2">
              <span className="text-xs text-purple-300 rounded border border-purple-500/30 bg-purple-500/10 px-2 py-1">
                {t("settings.transcriptionProfiles.draft", "Draft")}
              </span>
              <button type="button" onClick={clearNewProfile} disabled={isCreating} className="text-xs text-text/70 underline hover:text-text disabled:opacity-50">
                {t("settings.transcriptionProfiles.clearDraft", "Clear")}
              </button>
            </div>
          )}
          {/* Profile Name */}
          <div className="flex flex-wrap items-center gap-3 min-w-0">
            <label htmlFor="new-profile-name" className="text-sm font-semibold text-text/70">
              {t("settings.transcriptionProfiles.profileName")} *
            </label>
            <Input
              id="new-profile-name"
              type="text"
              value={newName}
              className="min-w-0 flex-1 basis-64"
              onChange={(e) => { setNewName(e.target.value); setNewBackendNameError(null); setNewProfileError(null); }}
              placeholder={t(
                "settings.transcriptionProfiles.profileNamePlaceholder",
              )}
              variant="compact"
              disabled={isCreating}
              required
              aria-invalid={Boolean(newNameError) && hasNewProfileDraft}
              aria-describedby={newNameError ? "new-profile-name-hint" : undefined}
            />
          </div>
          {newNameError && <p id="new-profile-name-hint" className={`text-xs ${newName.trim() ? "text-red-400" : "text-mid-gray"}`} role="status">
            {newNameError}
          </p>}

          {duplicateSourceName && <p className="text-xs text-text/70">
            {t("settings.transcriptionProfiles.copyDraftFrom", { name: duplicateSourceName, defaultValue: "Copied from {{name}}. Review settings before creating." })}
          </p>}
          <ProfileSettingsForm key={`new-${draftRevision}`} {...sharedFormProps} profile={newProfileDraft} scope="draft"
            onCommit={commitNewDraftPatch} onDraftChange={applyNewDraftPatch} onValidationReasonsChange={setNewFormValidationReasons} disabled={isCreating}
            shortcut={<HandyShortcut shortcutId="transcribe_new_profile" grouped={true} disabled={isCreating}
              title={newName.trim() || t("settings.transcriptionProfiles.draft", "Draft")} description={newName.trim() || t("settings.transcriptionProfiles.draft", "Draft")}
              draftBinding={newShortcut} onDraftBindingChange={setNewShortcut} />} />

          {newProfileError && (
            <p className="text-xs text-red-400">{newProfileError}</p>
          )}

          {/* Create Button */}
          <div className="flex justify-end pt-1">
            <Tooltip
              className="inline-flex rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-purple-500/50"
              disabled={!createDisabled}
              tabIndex={createDisabled ? 0 : undefined}
              triggerRole={createDisabled ? "button" : undefined}
              ariaDisabled={createDisabled || undefined}
              ariaLabel={createDisabled ? t("settings.transcriptionProfiles.addProfile") : undefined}
              content={createUnavailableReasons.map((reason, index) => (
                <React.Fragment key={reason}>
                  {index > 0 && <br />}
                  <span>{createUnavailableReasons.length > 1 ? "• " : ""}{reason}</span>
                </React.Fragment>
              ))}
            >
              <Button
                type="button"
                onClick={handleCreate}
                variant="primary"
                size="sm"
                disabled={createDisabled}
                aria-hidden={createDisabled || undefined}
                className={`inline-flex items-center${createDisabled ? " pointer-events-none" : ""}`}
              >
                <Plus className="w-3.5 h-3.5 mr-1.5" />
                {t("settings.transcriptionProfiles.addProfile")}
              </Button>
            </Tooltip>
          </div>
        </div>
        </SettingContainer>
      </SettingsGroup>
    </div>
  );
};
