import React, { useState, useMemo, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  Plus,
  Trash2,
  ChevronDown,
  ChevronUp,
  Globe,
  Check,
  RefreshCw,
  RefreshCcw,
  Info,
} from "lucide-react";
import { commands, TranscriptionProfile } from "@/bindings";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { SettingsGroup } from "../ui/SettingsGroup";
import { SettingContainer } from "../ui/SettingContainer";
import { Button } from "../ui/Button";
import { Input } from "../ui/Input";
import { Dropdown } from "../ui/Dropdown";
import { Badge } from "../ui/Badge";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { HandyShortcut } from "./HandyShortcut";
import { TranslateToEnglish } from "./TranslateToEnglish";
import { ModelSelect } from "./PostProcessingSettingsApi/ModelSelect";
import { ResetButton } from "../ui/ResetButton";
import type { ModelOption } from "./PostProcessingSettingsApi/types";
import { useSettings } from "../../hooks/useSettings";
import { useModels } from "../../hooks/useModels";
import { LANGUAGES } from "../../lib/constants/languages";
import { getModelPromptInfo } from "./TranscriptionSystemPrompt";

const APPLE_PROVIDER_ID = "apple_intelligence";

const DEFAULT_CLEAN_PROMPT = `Clean this Speech to text transcript:
1. Fix spelling, capitalization, and punctuation errors
2. Convert number words to digits (twenty-five → 25, ten percent → 10%, five dollars → $5)
3. Replace spoken punctuation with symbols (period → ., comma → , question mark → ?)
4. REPLACE "ENTER" WITH NEWLINE SYMBOL!!!!
5. Keep the language in the original version (if it was french, keep it in french for example) unless last sentence is "Translate this to <language>". Then - do this: CUT OUT THE LAST SENTENCE (the one wiht instruction), and translate the text into target language. 

Preserve exact meaning and word order. Do not paraphrase or reorder content.

Return only the cleaned transcript.

Transcript:
\${output}`;

interface ExtendedTranscriptionProfile extends TranscriptionProfile {
  include_in_cycle: boolean;
  push_to_talk: boolean;
  stt_prompt_override_enabled: boolean;
}

interface ProfileCardProps {
  profile: ExtendedTranscriptionProfile;
  isExpanded: boolean;
  onToggleExpand: () => void;
  onUpdate: (profile: ExtendedTranscriptionProfile) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
  canDelete: boolean;
  promptLimit: number;
  isActive: boolean;
  onSetActive: (id: string) => Promise<void>;
  llmConfigured: boolean;
  llmModelOptions: ModelOption[];
  globalLlmModel: string;
  onRefreshModels: () => void;
  isFetchingModels: boolean;
  defaultLlmPrompt: string;
  // Note: resolvedOsLanguage removed - language is detected at transcription time, not in UI
}

const ProfileCard: React.FC<ProfileCardProps> = ({
  profile,
  isExpanded,
  onToggleExpand,
  onUpdate,
  onDelete,
  canDelete,
  promptLimit,
  isActive,
  onSetActive,
  llmConfigured,
  llmModelOptions,
  globalLlmModel,
  onRefreshModels,
  isFetchingModels,
  defaultLlmPrompt,
}) => {
  const { t } = useTranslation();
  const [isUpdating, setIsUpdating] = useState(false);
  // Local state for LLM prompt - initializes with override or default
  const [localLlmPrompt, setLocalLlmPrompt] = useState(
    profile.llm_prompt_override || defaultLlmPrompt
  );

  // Update local prompt when default changes (settings loaded) or profile changes
  useEffect(() => {
    if (!profile.llm_prompt_override) {
      setLocalLlmPrompt(defaultLlmPrompt);
    } else {
      setLocalLlmPrompt(profile.llm_prompt_override);
    }
  }, [profile.llm_prompt_override, defaultLlmPrompt]);

  const bindingId = `transcribe_${profile.id}`;

  const languageLabel = useMemo(() => {
    const lang = LANGUAGES.find((l) => l.value === profile.language);
    return lang?.label || t("settings.general.language.auto");
  }, [profile.language, t]);

  const promptLength = (profile.system_prompt || "").length;
  const isOverLimit = promptLimit > 0 && promptLength > promptLimit;

  // Instant update handlers
  const handleNameChange = async (newName: string) => {
    const trimmed = newName.trim();
    if (!trimmed || trimmed === profile.name) return;
    setIsUpdating(true);
    try {
      await onUpdate({ ...profile, name: trimmed });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleLanguageChange = async (newLanguage: string) => {
    if (newLanguage === profile.language) return;
    setIsUpdating(true);
    try {
      await onUpdate({ ...profile, language: newLanguage });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleTranslateChange = async (newTranslate: boolean) => {
    setIsUpdating(true);
    try {
      await onUpdate({ ...profile, translate_to_english: newTranslate });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleSystemPromptChange = async (newPrompt: string) => {
    if (newPrompt === (profile.system_prompt || "")) return;
    if (promptLimit > 0 && newPrompt.length > promptLimit) return;
    setIsUpdating(true);
    try {
      await onUpdate({ ...profile, system_prompt: newPrompt });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleIncludeInCycleChange = async (newValue: boolean) => {
    setIsUpdating(true);
    try {
      await onUpdate({ ...profile, include_in_cycle: newValue });
    } finally {
      setIsUpdating(false);
    }
  };

  const handlePushToTalkChange = async (newValue: boolean) => {
    setIsUpdating(true);
    try {
      await onUpdate({ ...profile, push_to_talk: newValue });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleSttPromptOverrideChange = async (newValue: boolean) => {
    setIsUpdating(true);
    try {
      await onUpdate({ ...profile, stt_prompt_override_enabled: newValue });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleLlmEnabledChange = async (newValue: boolean) => {
    setIsUpdating(true);
    try {
      await onUpdate({ ...profile, llm_post_process_enabled: newValue });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleLlmPromptChange = async (newPrompt: string) => {
    const trimmed = newPrompt.trim();
    if (trimmed === (profile.llm_prompt_override || "")) return;
    setIsUpdating(true);
    try {
      await onUpdate({
        ...profile,
        llm_prompt_override: trimmed || null,
      });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleLlmModelChange = async (newModel: string | null) => {
    if (newModel === profile.llm_model_override) return;
    setIsUpdating(true);
    try {
      await onUpdate({
        ...profile,
        llm_model_override: newModel || null,
      });
    } finally {
      setIsUpdating(false);
    }
  };

  const handleDelete = async () => {
    setIsUpdating(true);
    try {
      await onDelete(profile.id);
    } finally {
      setIsUpdating(false);
    }
  };

  return (
    <div
      className={`min-w-0 border rounded-lg transition-colors ${isActive ? "border-purple-500/50 bg-purple-500/5" : "border-mid-gray/30 bg-background/50"}`}
    >
      {/* Header - always visible */}
      <div
        className="flex items-center justify-between gap-3 px-4 py-3 cursor-pointer hover:bg-mid-gray/5 transition-colors"
        onClick={onToggleExpand}
      >
        <div className="flex items-center gap-3 min-w-0">
          <Globe
            className={`w-4 h-4 ${isActive ? "text-purple-400" : "text-logo-primary"}`}
          />
          <div className="flex flex-col min-w-0">
            <div className="flex flex-wrap items-center gap-2 min-w-0">
              <span className="font-medium text-sm break-words">
                {profile.name}
              </span>
              {isActive && (
                <Badge
                  variant="secondary"
                  className="bg-purple-500/20 text-purple-400 border-purple-500/30 text-[10px] px-1.5 py-0 max-w-full truncate"
                >
                  {t("settings.transcriptionProfiles.active")}
                </Badge>
              )}
            </div>
            <span className="text-xs text-mid-gray break-words">
              {languageLabel}
              {profile.translate_to_english && (
                <span className="text-purple-400 ml-1">→ EN</span>
              )}
              {profile.language === "os_input" && (
                <span
                  className="text-amber-400/60 ml-2 text-[10px]"
                  title={t("settings.general.language.followOsInputTooltip")}
                >
                  ({t("settings.general.language.followOsInputHint")})
                </span>
              )}
            </span>
          </div>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <Button
            onClick={(e) => {
              e.stopPropagation();
              onSetActive(profile.id);
            }}
            disabled={isActive || isUpdating}
            variant={isActive ? "secondary" : "primary"}
            size="sm"
            className={isActive ? "opacity-100 cursor-default" : ""}
            title={isActive ? t("settings.transcriptionProfiles.active") : undefined}
          >
            {isActive ? (
              <Check className="w-4 h-4" />
            ) : (
              <span className="text-xs">
                {t("settings.transcriptionProfiles.setActive")}
              </span>
            )}
          </Button>
          {canDelete && (
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

      {/* Expanded content */}
      {isExpanded && (
        <div className="px-4 pb-4 pt-3 border-t border-mid-gray/20 space-y-3">
          {/* Cycle & Push-to-Talk Controls */}
          <div className="grid gap-3 md:grid-cols-2 bg-mid-gray/5 p-3 rounded-lg border border-mid-gray/10">
            <div className="min-w-0">
              <label className="text-xs font-semibold text-text/70 block mb-2">
                {t("settings.transcriptionProfiles.includeInCycle")}
              </label>
              <div className="flex items-center gap-2">
                <input
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
            </div>

            <div className="min-w-0">
              <label className="text-xs font-semibold text-text/70 block mb-2">
                {t("settings.general.pushToTalk.label")}
              </label>
              <div className="flex items-center gap-2">
                <ToggleSwitch
                  checked={profile.push_to_talk ?? true}
                  onChange={handlePushToTalkChange}
                  disabled={isUpdating}
                />
                <span className="text-xs text-mid-gray leading-snug">
                  {t("settings.general.pushToTalk.description")}
                </span>
              </div>
            </div>
          </div>

          <div className="grid gap-3 lg:grid-cols-2">
            {/* Shortcut */}
            <div className="space-y-2 min-w-0">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.shortcut")}
              </label>
              <HandyShortcut shortcutId={bindingId} grouped={true} />
            </div>

            {/* Profile Name */}
            <div className="space-y-2 min-w-0">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.profileName")}
              </label>
              <Input
                type="text"
                defaultValue={profile.name}
                onBlur={(e) => handleNameChange(e.target.value)}
                placeholder={t(
                  "settings.transcriptionProfiles.profileNamePlaceholder",
                )}
                variant="compact"
                disabled={isUpdating}
              />
            </div>

            {/* Language Selection */}
            <div className="space-y-2 relative z-20 min-w-0">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.language")}
              </label>
              <Dropdown
                selectedValue={profile.language}
                options={LANGUAGES.map((l) => ({
                  value: l.value,
                  label: l.label,
                  className: l.className,
                }))}
                onSelect={(value) => value && handleLanguageChange(value)}
                placeholder={t("settings.general.language.auto")}
                disabled={isUpdating}
              />
            </div>

            {/* Translate to English Toggle */}
            <div className="space-y-2 min-w-0">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.translateToEnglish")}
              </label>
              <div className="flex flex-col gap-2 rounded-md border border-mid-gray/10 bg-mid-gray/5 px-3 py-2 sm:flex-row sm:items-center sm:justify-between">
                <p className="text-xs text-mid-gray leading-snug">
                  {t(
                    "settings.transcriptionProfiles.translateToEnglishDescription",
                  )}
                </p>
                <button
                  type="button"
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
              </div>
            </div>
          </div>

          {/* Voice Model Prompt Override */}
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <label className="text-xs font-semibold text-text/70">
                  {t("settings.transcriptionProfiles.overrideSystemPrompt")}
                </label>
                <div
                  className="relative flex items-center justify-center p-1 group"
                  title={t("settings.transcriptionProfiles.voiceModelPromptTooltip")}
                >
                  <svg
                    className="w-4 h-4 text-[#707070] cursor-help hover:text-[#ff4d8d] transition-colors duration-200"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                    />
                  </svg>
                </div>
              </div>
              <ToggleSwitch
                checked={profile.stt_prompt_override_enabled ?? false}
                onChange={handleSttPromptOverrideChange}
                disabled={isUpdating}
              />
            </div>
            <p className="text-xs text-mid-gray">
              {profile.stt_prompt_override_enabled
                ? t("settings.transcriptionProfiles.overrideSystemPromptOnDescription")
                : t("settings.transcriptionProfiles.overrideSystemPromptOffDescription")}
            </p>
            {profile.stt_prompt_override_enabled && (
              <>
                <div className="flex items-center justify-between">
                  <label className="text-xs font-semibold text-text/70">
                    {t("settings.transcriptionProfiles.systemPrompt")}
                  </label>
                  <span
                    className={`text-xs ${isOverLimit ? "text-red-400" : "text-mid-gray"}`}
                  >
                    {promptLength}
                    {promptLimit > 0 && ` / ${promptLimit}`}
                  </span>
                </div>
                <textarea
                  defaultValue={profile.system_prompt || ""}
                  onBlur={(e) => handleSystemPromptChange(e.target.value)}
                  placeholder={t(
                    "settings.transcriptionProfiles.systemPromptPlaceholder",
                  )}
                  disabled={isUpdating}
                  rows={3}
                  className={`w-full px-3 py-2 text-sm bg-[#1e1e1e]/80 border rounded-md resize-none transition-colors ${
                    isOverLimit
                      ? "border-red-400 focus:border-red-400"
                      : "border-[#3c3c3c] focus:border-[#4a4a4a]"
                  } ${isUpdating ? "opacity-40 cursor-not-allowed" : ""} text-[#e8e8e8] placeholder-[#6b6b6b]`}
                />
                <p className="text-xs text-mid-gray">
                  {t("settings.transcriptionProfiles.systemPromptDescription")}
                </p>
                {isOverLimit && (
                  <p className="text-xs text-red-400">
                    {t("settings.transcriptionProfiles.systemPromptTooLong", {
                      limit: promptLimit,
                    })}
                  </p>
                )}
              </>
            )}
          </div>

          {/* LLM Post-Processing Section */}
          {llmConfigured && (
            <div className="space-y-2 pt-3 border-t border-mid-gray/10">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <label className="text-xs font-semibold text-text/70">
                    {t("settings.transcriptionProfiles.llmPostProcessing.title")}
                  </label>
                  <div
                    className="relative flex items-center justify-center p-1 group"
                    title={t("settings.transcriptionProfiles.llmPostProcessing.description")}
                  >
                    <svg
                      className="w-4 h-4 text-[#707070] cursor-help hover:text-[#ff4d8d] transition-colors duration-200"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                      />
                    </svg>
                  </div>
                </div>
                <ToggleSwitch
                  checked={profile.llm_post_process_enabled ?? false}
                  onChange={handleLlmEnabledChange}
                  disabled={isUpdating}
                />
              </div>
              <p className="text-xs text-mid-gray">
                {t("settings.transcriptionProfiles.llmPostProcessing.description")}
              </p>

              {profile.llm_post_process_enabled && (
                <div className="space-y-3 pl-3 border-l-2 border-purple-500/30">
                  {/* Prompt Override */}
                  <div className="space-y-1">
                    <label className="text-xs text-text/60">
                      {t("settings.transcriptionProfiles.llmPostProcessing.overridePrompt")}
                    </label>
                    <p className="text-xs text-mid-gray">
                      {t("settings.transcriptionProfiles.llmPostProcessing.overridePromptHint")}
                    </p>
                    {/* ${output} Variable Documentation */}
                    <div className="p-2 bg-blue-500/10 border border-blue-500/30 rounded text-xs space-y-1">
                      <div className="font-semibold text-blue-400">
                        {t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.title")}
                      </div>
                      <p className="text-text/70" dangerouslySetInnerHTML={{ __html: t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.description") }} />
                      <details className="cursor-pointer">
                        <summary className="text-blue-400 hover:text-blue-300">
                          {t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.howItWorks")}
                        </summary>
                        <ol className="list-decimal list-inside text-text/60 mt-1 space-y-0.5 pl-2">
                          <li>{t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.step1")}</li>
                          <li dangerouslySetInnerHTML={{ __html: t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.step2") }} />
                          <li>{t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.step3")}</li>
                          <li>{t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.step4")}</li>
                        </ol>
                        <div className="mt-2 space-y-1">
                          <div className="text-green-400">{t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.validExample")}</div>
                          <code className="block bg-green-500/10 px-2 py-1 rounded text-green-300 whitespace-pre-wrap">
                            {t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.validPrompt")}
                          </code>
                          <div className="text-red-400 mt-2">{t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.invalidExample")}</div>
                          <code className="block bg-red-500/10 px-2 py-1 rounded text-red-300">
                            {t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.invalidPrompt")}
                          </code>
                          <p className="text-red-400/80" dangerouslySetInnerHTML={{ __html: t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.invalidWarning") }} />
                        </div>
                        <p className="mt-2 text-text/60" dangerouslySetInnerHTML={{ __html: t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.tip") }} />
                      </details>
                    </div>
                    <textarea
                      value={localLlmPrompt || defaultLlmPrompt}
                      onChange={(e) => setLocalLlmPrompt(e.target.value)}
                      onBlur={(e) => handleLlmPromptChange(e.target.value)}
                      placeholder={t("settings.transcriptionProfiles.llmPostProcessing.promptPlaceholder")}
                      disabled={isUpdating}
                      rows={6}
                      className={`w-full px-3 py-2 text-sm bg-[#1e1e1e]/80 border rounded-md resize-none transition-colors border-[#3c3c3c] focus:border-[#4a4a4a] ${isUpdating ? "opacity-40 cursor-not-allowed" : ""} text-[#e8e8e8] placeholder-[#6b6b6b]`}
                    />
                  </div>

                  {/* Model Override */}
                  <div className="space-y-1">
                    <label className="text-xs text-text/60">
                      {t("settings.transcriptionProfiles.llmPostProcessing.overrideModel")}
                    </label>
                    <p className="text-xs text-mid-gray">
                      {t("settings.transcriptionProfiles.llmPostProcessing.overrideModelHint")}
                    </p>
                    <div className="flex items-center gap-2">
                      <ModelSelect
                        value={profile.llm_model_override || ""}
                        options={llmModelOptions}
                        disabled={isUpdating}
                        placeholder={
                          globalLlmModel
                            ? t("settings.transcriptionProfiles.llmPostProcessing.usingGlobalModel", { model: globalLlmModel })
                            : t("settings.transcriptionProfiles.llmPostProcessing.modelPlaceholder")
                        }
                        onSelect={(value) => handleLlmModelChange(value || null)}
                        onCreate={(value) => handleLlmModelChange(value)}
                        onBlur={() => {}}
                        className="flex-1"
                      />
                      <ResetButton
                        onClick={onRefreshModels}
                        disabled={isFetchingModels || isUpdating}
                        title="Fetch Models from Server, then you can type part of model name for searching it in drop-down."
                      >
                        <RefreshCcw className={`w-4 h-4 ${isFetchingModels ? "animate-spin" : ""}`} />
                      </ResetButton>
                    </div>
                  </div>
                </div>
              )}
            </div>
          )}

        </div>
      )}
    </div>
  );
};

export const TranscriptionProfiles: React.FC = () => {
  const { t } = useTranslation();
  const { settings, refreshSettings, updateSetting, postProcessModelOptions, fetchPostProcessModels } = useSettings();
  const { getModelInfo } = useModels();
  const [expandedIds, setExpandedIds] = useState<Set<string>>(
    () => new Set(["default"]),
  );
  const [isCreating, setIsCreating] = useState(false);
  const [isFetchingModels, setIsFetchingModels] = useState(false);
  const [newName, setNewName] = useState("");
  const [newLanguage, setNewLanguage] = useState("auto");
  const [newTranslate, setNewTranslate] = useState(false);
  const [newSystemPrompt, setNewSystemPrompt] = useState("");
  const [newPushToTalk, setNewPushToTalk] = useState(true);
  const [newIncludeInCycle, setNewIncludeInCycle] = useState(true);
  const [newLlmEnabled, setNewLlmEnabled] = useState(false);
  const [newLlmPromptOverride, setNewLlmPromptOverride] = useState("");
  const [newLlmModelOverride, setNewLlmModelOverride] = useState<string | null>(null);

  const profiles = (settings?.transcription_profiles ||
    []) as ExtendedTranscriptionProfile[];

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
    const isRemote =
      settings?.transcription_provider === "remote_openai_compatible";
    return isRemote
      ? settings?.remote_stt?.model_id || ""
      : settings?.selected_model || "";
  }, [
    settings?.transcription_provider,
    settings?.remote_stt?.model_id,
    settings?.selected_model,
  ]);

  // Get model info for prompt configuration (same logic as TranscriptionSystemPrompt)
  const modelInfo = useMemo(() => {
    if (!activeModelId) {
      return { supportsPrompt: true, charLimit: 896, modelId: "" };
    }
    const isRemote = settings?.transcription_provider === "remote_openai_compatible";
    if (isRemote) {
      return getModelPromptInfo(activeModelId);
    }
    // For local models, get engine_type from model info
    const localModelInfo = getModelInfo(activeModelId);
    return getModelPromptInfo(activeModelId, localModelInfo?.engine_type);
  }, [activeModelId, settings?.transcription_provider, getModelInfo]);

  // Get prompt limit based on active transcription settings
  const promptLimit = modelInfo.supportsPrompt ? modelInfo.charLimit : 0;

  const newPromptLength = newSystemPrompt.length;
  const isNewPromptOverLimit = promptLimit > 0 && newPromptLength > promptLimit;

  // LLM Post-Processing configuration
  const currentLlmProviderId = settings?.post_process_provider_id || "";

  // Show LLM section if post-processing is enabled globally (user has configured it)
  const isLlmConfigured = useMemo(() => {
    // If global post-process is enabled, the user has set it up
    if (settings?.post_process_enabled) return true;
    // Also check if provider is configured with API key
    if (!currentLlmProviderId) return false;
    if (currentLlmProviderId === APPLE_PROVIDER_ID) return true;
    const apiKey = settings?.post_process_api_keys?.[currentLlmProviderId];
    return !!apiKey?.trim();
  }, [settings?.post_process_enabled, currentLlmProviderId, settings?.post_process_api_keys]);

  const llmModelOptions = useMemo<ModelOption[]>(() => {
    const models = postProcessModelOptions[currentLlmProviderId] || [];
    return models.map((m) => ({ value: m, label: m }));
  }, [postProcessModelOptions, currentLlmProviderId]);

  const globalLlmModel = settings?.post_process_models?.[currentLlmProviderId] || "";

  const globalPromptText = useMemo(() => {
    const prompts = settings?.post_process_prompts || [];
    const selectedId = settings?.post_process_selected_prompt_id;
    return prompts.find((p) => p.id === selectedId)?.prompt || DEFAULT_CLEAN_PROMPT;
  }, [settings?.post_process_prompts, settings?.post_process_selected_prompt_id]);

  const handleRefreshModels = useCallback(async () => {
    if (!currentLlmProviderId || currentLlmProviderId === APPLE_PROVIDER_ID) return;
    setIsFetchingModels(true);
    try {
      await fetchPostProcessModels(currentLlmProviderId);
    } catch (error) {
      console.error("Failed to fetch models:", error);
    } finally {
      setIsFetchingModels(false);
    }
  }, [currentLlmProviderId, fetchPostProcessModels]);

  const handleCreate = async () => {
    if (!newName.trim()) return;
    if (isNewPromptOverLimit) return;
    setIsCreating(true);
    try {
      const newProfile = await invoke<TranscriptionProfile>("add_transcription_profile", {
        name: newName.trim(),
        language: newLanguage,
        translateToEnglish: newTranslate,
        systemPrompt: newSystemPrompt,
        pushToTalk: newPushToTalk,
        includeInCycle: newIncludeInCycle,
        llmSettings: {
          enabled: newLlmEnabled,
          prompt_override: newLlmPromptOverride.trim() || null,
          model_override: newLlmModelOverride,
        },
      });
      await refreshSettings();
      setNewName("");
      setNewLanguage("auto");
      setNewTranslate(false);
      setNewSystemPrompt("");
      setNewPushToTalk(true);
      setNewIncludeInCycle(true);
      setNewLlmEnabled(false);
      setNewLlmPromptOverride("");
      setNewLlmModelOverride(null);
      setExpandedIds((prev) => {
        const next = new Set(prev);
        next.add(newProfile.id);
        return next;
      });
    } catch (error) {
      console.error("Failed to create profile:", error);
    } finally {
      setIsCreating(false);
    }
  };

  const handleUpdate = async (profile: ExtendedTranscriptionProfile) => {
    try {
      await invoke("update_transcription_profile", {
        id: profile.id,
        name: profile.name,
        language: profile.language,
        translateToEnglish: profile.translate_to_english,
        systemPrompt: profile.system_prompt || "",
        sttPromptOverrideEnabled: profile.stt_prompt_override_enabled ?? false,
        includeInCycle: profile.include_in_cycle,
        pushToTalk: profile.push_to_talk,
        llmSettings: {
          enabled: profile.llm_post_process_enabled ?? false,
          promptOverride: profile.llm_prompt_override ?? null,
          modelOverride: profile.llm_model_override ?? null,
        },
      });
      await refreshSettings();
    } catch (error) {
      console.error("Failed to update profile:", error);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await commands.deleteTranscriptionProfile(id);
      await refreshSettings();
      setExpandedIds((prev) => {
        if (!prev.has(id)) return prev;
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    } catch (error) {
      console.error("Failed to delete profile:", error);
    }
  };

  const handleSetActive = async (id: string) => {
    try {
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

  return (
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
                {t("settings.transcriptionProfiles.transcribeActiveProfileDescription")}
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
                Global shortcut to cycle through active profiles
              </span>
            </div>
            <div className="shrink-0">
              <HandyShortcut shortcutId="cycle_profile" />
            </div>
          </div>

          {/* Overlay Toggle */}
          <div className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center">
            <div className="flex flex-col min-w-0">
              <span className="text-sm font-medium">
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
        title={t("settings.transcriptionProfiles.existingProfiles")}
        description=""
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <div className="space-y-3">
          {/* Default Profile Card - Expandable with global settings */}
          <div
            className={`min-w-0 border rounded-lg transition-colors ${activeProfileId === "default" ? "border-purple-500/50 bg-purple-500/5" : "border-mid-gray/30 bg-background/50"}`}
          >
            {/* Header - clickable to expand */}
            <div
              className="flex items-center justify-between gap-3 px-4 py-3 cursor-pointer hover:bg-mid-gray/5 transition-colors"
              onClick={() => toggleExpanded("default")}
            >
              <div className="flex items-center gap-3 min-w-0">
                <RefreshCw
                  className={`w-4 h-4 ${activeProfileId === "default" ? "text-purple-400" : "text-mid-gray"}`}
                />
                <div className="flex flex-col min-w-0">
                  <div className="flex flex-wrap items-center gap-2 min-w-0">
                    <span className="font-medium text-sm break-words">
                      {t("settings.transcriptionProfiles.defaultProfile")}
                    </span>
                    {activeProfileId === "default" && (
                      <Badge
                        variant="secondary"
                        className="bg-purple-500/20 text-purple-400 border-purple-500/30 text-[10px] px-1.5 py-0"
                      >
                        {t("settings.transcriptionProfiles.active")}
                      </Badge>
                    )}
                  </div>
                  <span className="text-xs text-mid-gray break-words">
                    {(() => {
                      const selectedLang = settings?.selected_language || "auto";
                      const lang = LANGUAGES.find((l) => l.value === selectedLang);
                      return lang?.label || t("settings.general.language.auto");
                    })()}
                    {settings?.translate_to_english && (
                      <span className="text-purple-400 ml-1">→ EN</span>
                    )}
                    {settings?.selected_language === "os_input" && (
                      <span
                        className="text-amber-400/60 ml-2 text-[10px]"
                        title={t("settings.general.language.followOsInputTooltip")}
                      >
                        ({t("settings.general.language.followOsInputHint")})
                      </span>
                    )}
                  </span>
                </div>
              </div>
              <div className="flex items-center gap-2 shrink-0">
                <Button
                  onClick={(e) => {
                    e.stopPropagation();
                    handleSetActive("default");
                  }}
                  disabled={activeProfileId === "default"}
                  variant={
                    activeProfileId === "default" ? "secondary" : "primary"
                  }
                  size="sm"
                  className={
                    activeProfileId === "default"
                      ? "opacity-100 cursor-default"
                      : ""
                  }
                  title={activeProfileId === "default" ? t("settings.transcriptionProfiles.active") : undefined}
                >
                  {activeProfileId === "default" ? (
                    <Check className="w-4 h-4" />
                  ) : (
                    <span className="text-xs">
                      {t("settings.transcriptionProfiles.setActive")}
                    </span>
                  )}
                </Button>
                {isExpanded("default") ? (
                  <ChevronUp className="w-4 h-4 text-mid-gray" />
                ) : (
                  <ChevronDown className="w-4 h-4 text-mid-gray" />
                )}
              </div>
            </div>

            {/* Expanded content - Global settings */}
            {isExpanded("default") && (
              <div className="px-4 pb-4 pt-3 border-t border-mid-gray/20 space-y-3">
                {/* Optional Shortcut for Default profile */}
                <div className="space-y-2">
                  <label className="text-xs font-semibold text-text/70">
                    {t("settings.transcriptionProfiles.shortcut")}
                  </label>
                  <HandyShortcut shortcutId="transcribe_default" grouped={true} />
                </div>

                {/* Language & Push-to-Talk in a row */}
                <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                  <div className="space-y-2 min-w-0">
                    <label className="text-xs font-semibold text-text/70">
                      {t("settings.general.language.title")}
                    </label>
                    <Dropdown
                      selectedValue={settings?.selected_language || "auto"}
                      onSelect={(value: string) =>
                        updateSetting &&
                        updateSetting("selected_language" as any, value)
                      }
                      options={LANGUAGES.map((lang) => ({
                        value: lang.value,
                        label: lang.label,
                        className: lang.className,
                      }))}
                    />
                  </div>
                  <div className="space-y-2 min-w-0">
                    <label className="text-xs font-semibold text-text/70">
                      {t("settings.general.pushToTalk.label")}
                    </label>
                    <div className="flex items-start gap-2">
                      <ToggleSwitch
                        checked={settings?.push_to_talk ?? true}
                        onChange={(checked) =>
                          updateSetting &&
                          updateSetting("push_to_talk" as any, checked)
                        }
                      />
                      <span className="text-xs text-mid-gray leading-snug">
                        {t("settings.general.pushToTalk.description")}
                      </span>
                    </div>
                  </div>
                </div>

                {/* Translate to English */}
                <TranslateToEnglish grouped={true} descriptionMode="inline" />

                {/* Voice Model Prompt - only show if model supports prompts */}
                {modelInfo.supportsPrompt && (
                  <div className="space-y-2">
                    <div className="flex items-center justify-between">
                      <label className="text-xs font-semibold text-text/70">
                        {t("settings.general.transcriptionSystemPrompt.title")}
                      </label>
                      {promptLimit > 0 && (
                        <span
                          className={`text-xs ${(settings?.transcription_prompts?.[activeModelId] || "").length > promptLimit ? "text-red-400" : "text-mid-gray"}`}
                        >
                          {
                            (
                              settings?.transcription_prompts?.[
                                activeModelId
                              ] || ""
                            ).length
                          }
                          /{promptLimit}
                        </span>
                      )}
                    </div>
                    <textarea
                      value={
                        settings?.transcription_prompts?.[
                          activeModelId
                        ] || ""
                      }
                      onChange={async (e) => {
                        if (activeModelId) {
                          await commands.changeTranscriptionPromptSetting(
                            activeModelId,
                            e.target.value,
                          );
                          refreshSettings();
                        }
                      }}
                      placeholder={t(
                        "settings.general.transcriptionSystemPrompt.placeholder",
                      )}
                      className="w-full h-20 px-3 py-2 bg-background border border-mid-gray/30 rounded-lg text-sm text-text placeholder-mid-gray/50 resize-none focus:outline-none focus:border-purple-500/50"
                    />
                  </div>
                )}
              </div>
            )}
          </div>

          {/* Custom Profiles */}
          {profiles.map((profile) => (
            <ProfileCard
              key={profile.id}
              profile={profile}
              isExpanded={isExpanded(profile.id)}
              onToggleExpand={() => toggleExpanded(profile.id)}
              onUpdate={handleUpdate}
              onDelete={handleDelete}
              canDelete={true}
              promptLimit={promptLimit}
              isActive={activeProfileId === profile.id}
              onSetActive={handleSetActive}
              llmConfigured={isLlmConfigured}
              llmModelOptions={llmModelOptions}
              globalLlmModel={globalLlmModel}
              onRefreshModels={handleRefreshModels}
              isFetchingModels={isFetchingModels}
              defaultLlmPrompt={globalPromptText}
            />
          ))}
        </div>
      </SettingContainer>

      {/* Create new profile */}
      <SettingContainer
        title={t("settings.transcriptionProfiles.createNew")}
        description={t("settings.transcriptionProfiles.createNewDescription")}
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <div className="space-y-3 p-3 border border-dashed border-mid-gray/30 rounded-lg overflow-visible">
          {/* Profile Name & Language */}
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
            <div className="space-y-1 min-w-0">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.profileName")}
              </label>
              <Input
                type="text"
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                placeholder={t(
                  "settings.transcriptionProfiles.profileNamePlaceholder",
                )}
                variant="compact"
                disabled={isCreating}
              />
            </div>
            <div className="space-y-1 relative z-10 min-w-0">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.language")}
              </label>
              <Dropdown
                selectedValue={newLanguage}
                options={LANGUAGES.map((l) => ({
                  value: l.value,
                  label: l.label,
                  className: l.className,
                }))}
                onSelect={(value) => value && setNewLanguage(value)}
                placeholder={t("settings.general.language.auto")}
                disabled={isCreating}
              />
            </div>
          </div>

          {/* Translate to English & Push to Talk */}
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
            <div className="space-y-2 min-w-0">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.transcriptionProfiles.translateToEnglish")}
              </label>
              <div className="flex items-center gap-2">
                <ToggleSwitch
                  checked={newTranslate}
                  onChange={setNewTranslate}
                  disabled={isCreating}
                />
                <span className="text-xs text-mid-gray leading-snug">
                  {t("settings.transcriptionProfiles.translateToEnglishTooltip")}
                </span>
              </div>
            </div>
            <div className="space-y-2 min-w-0">
              <label className="text-xs font-semibold text-text/70">
                {t("settings.general.pushToTalk.label")}
              </label>
              <div className="flex items-center gap-2">
                <ToggleSwitch
                  checked={newPushToTalk}
                  onChange={setNewPushToTalk}
                  disabled={isCreating}
                />
                <span className="text-xs text-mid-gray leading-snug">
                  {t("settings.general.pushToTalk.description")}
                </span>
              </div>
            </div>
          </div>

          {/* Include in Cycle */}
          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={newIncludeInCycle}
              onChange={(e) => setNewIncludeInCycle(e.target.checked)}
              disabled={isCreating}
              className="w-4 h-4 rounded border-mid-gray bg-background text-purple-500 focus:ring-purple-500/50"
            />
            <label className="text-xs font-semibold text-text/70">
              {t("settings.transcriptionProfiles.includeInCycle")}
            </label>
            <span className="text-xs text-mid-gray">
              {t("settings.transcriptionProfiles.includeInCycleDescription")}
            </span>
          </div>

          {/* Voice Model Prompt */}
          <div className="space-y-1">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <label className="text-xs font-semibold text-text/70">
                  {t("settings.transcriptionProfiles.systemPrompt")}
                </label>
                <div
                  className="relative flex items-center justify-center p-1"
                  title={t("settings.transcriptionProfiles.voiceModelPromptTooltip")}
                >
                  <svg
                    className="w-4 h-4 text-[#707070] cursor-help hover:text-[#ff4d8d] transition-colors duration-200"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                    />
                  </svg>
                </div>
              </div>
              <span
                className={`text-xs ${isNewPromptOverLimit ? "text-red-400" : "text-mid-gray"}`}
              >
                {newPromptLength}
                {promptLimit > 0 && ` / ${promptLimit}`}
              </span>
            </div>
            <textarea
              value={newSystemPrompt}
              onChange={(e) => setNewSystemPrompt(e.target.value)}
              placeholder={t(
                "settings.transcriptionProfiles.systemPromptPlaceholder",
              )}
              disabled={isCreating}
              rows={2}
              className={`w-full px-3 py-2 text-sm bg-[#1e1e1e]/80 border rounded-md resize-none transition-colors ${
                isNewPromptOverLimit
                  ? "border-red-400 focus:border-red-400"
                  : "border-[#3c3c3c] focus:border-[#4a4a4a]"
              } ${isCreating ? "opacity-40 cursor-not-allowed" : ""} text-[#e8e8e8] placeholder-[#6b6b6b]`}
            />
            {isNewPromptOverLimit && (
              <p className="text-xs text-red-400">
                {t("settings.transcriptionProfiles.systemPromptTooLong", {
                  limit: promptLimit,
                })}
              </p>
            )}
          </div>

          {/* LLM Post-Processing Section */}
          {isLlmConfigured && (
            <div className="space-y-2 pt-3 border-t border-mid-gray/10">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <label className="text-xs font-semibold text-text/70">
                    {t("settings.transcriptionProfiles.llmPostProcessing.title")}
                  </label>
                  <div
                    className="relative flex items-center justify-center p-1"
                    title={t("settings.transcriptionProfiles.llmPostProcessing.description")}
                  >
                    <svg
                      className="w-4 h-4 text-[#707070] cursor-help hover:text-[#ff4d8d] transition-colors duration-200"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                      />
                    </svg>
                  </div>
                </div>
                <ToggleSwitch
                  checked={newLlmEnabled}
                  onChange={setNewLlmEnabled}
                  disabled={isCreating}
                />
              </div>
              <p className="text-xs text-mid-gray">
                {t("settings.transcriptionProfiles.llmPostProcessing.description")}
              </p>

              {newLlmEnabled && (
                <div className="space-y-3 pl-3 border-l-2 border-purple-500/30">
                  {/* Prompt Override */}
                  <div className="space-y-1">
                    <label className="text-xs text-text/60">
                      {t("settings.transcriptionProfiles.llmPostProcessing.overridePrompt")}
                    </label>
                    <p className="text-xs text-mid-gray">
                      {t("settings.transcriptionProfiles.llmPostProcessing.overridePromptHint")}
                    </p>
                    {/* ${output} Variable Documentation */}
                    <div className="p-2 bg-blue-500/10 border border-blue-500/30 rounded text-xs space-y-1">
                      <div className="font-semibold text-blue-400">
                        {t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.title")}
                      </div>
                      <p className="text-text/70" dangerouslySetInnerHTML={{ __html: t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.description") }} />
                      <details className="cursor-pointer">
                        <summary className="text-blue-400 hover:text-blue-300">
                          {t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.howItWorks")}
                        </summary>
                        <ol className="list-decimal list-inside text-text/60 mt-1 space-y-0.5 pl-2">
                          <li>{t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.step1")}</li>
                          <li dangerouslySetInnerHTML={{ __html: t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.step2") }} />
                          <li>{t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.step3")}</li>
                          <li>{t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.step4")}</li>
                        </ol>
                        <div className="mt-2 space-y-1">
                          <div className="text-green-400">{t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.validExample")}</div>
                          <code className="block bg-green-500/10 px-2 py-1 rounded text-green-300 whitespace-pre-wrap">
                            {t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.validPrompt")}
                          </code>
                          <div className="text-red-400 mt-2">{t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.invalidExample")}</div>
                          <code className="block bg-red-500/10 px-2 py-1 rounded text-red-300">
                            {t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.invalidPrompt")}
                          </code>
                          <p className="text-red-400/80" dangerouslySetInnerHTML={{ __html: t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.invalidWarning") }} />
                        </div>
                        <p className="mt-2 text-text/60" dangerouslySetInnerHTML={{ __html: t("settings.transcriptionProfiles.llmPostProcessing.outputVariableDoc.tip") }} />
                      </details>
                    </div>
                    <textarea
                      value={newLlmPromptOverride || globalPromptText}
                      onChange={(e) => setNewLlmPromptOverride(e.target.value)}
                      placeholder={t("settings.transcriptionProfiles.llmPostProcessing.promptPlaceholder")}
                      disabled={isCreating}
                      rows={6}
                      className={`w-full px-3 py-2 text-sm bg-[#1e1e1e]/80 border rounded-md resize-none transition-colors border-[#3c3c3c] focus:border-[#4a4a4a] ${isCreating ? "opacity-40 cursor-not-allowed" : ""} text-[#e8e8e8] placeholder-[#6b6b6b]`}
                    />
                  </div>

                  {/* Model Override */}
                  <div className="space-y-1">
                    <label className="text-xs text-text/60">
                      {t("settings.transcriptionProfiles.llmPostProcessing.overrideModel")}
                    </label>
                    <p className="text-xs text-mid-gray">
                      {t("settings.transcriptionProfiles.llmPostProcessing.overrideModelHint")}
                    </p>
                    <div className="flex items-center gap-2">
                      <ModelSelect
                        value={newLlmModelOverride || ""}
                        options={llmModelOptions}
                        disabled={isCreating}
                        placeholder={
                          globalLlmModel
                            ? t("settings.transcriptionProfiles.llmPostProcessing.usingGlobalModel", { model: globalLlmModel })
                            : t("settings.transcriptionProfiles.llmPostProcessing.modelPlaceholder")
                        }
                        onSelect={(value) => setNewLlmModelOverride(value || null)}
                        onCreate={(value) => setNewLlmModelOverride(value)}
                        onBlur={() => {}}
                        className="flex-1"
                      />
                      <ResetButton
                        onClick={handleRefreshModels}
                        disabled={isFetchingModels || isCreating}
                        title="Fetch Models from Server, then you can type part of model name for searching it in drop-down."
                      >
                        <RefreshCcw className={`w-4 h-4 ${isFetchingModels ? "animate-spin" : ""}`} />
                      </ResetButton>
                    </div>
                  </div>
                </div>
              )}
            </div>
          )}

          {/* Create Button */}
          <div className="flex justify-end pt-1">
            <Button
              onClick={handleCreate}
              variant="primary"
              size="sm"
              disabled={!newName.trim() || isCreating || isNewPromptOverLimit}
              className="inline-flex items-center"
            >
              <Plus className="w-3.5 h-3.5 mr-1.5" />
              {t("settings.transcriptionProfiles.addProfile")}
            </Button>
          </div>
        </div>
      </SettingContainer>
    </SettingsGroup>
  );
};
