import React, { useMemo, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { useModels } from "../../hooks/useModels";
import { SettingContainer } from "../ui/SettingContainer";
import { Textarea } from "../ui/Textarea";
import { commands } from "../../bindings";

// Character limits - exported for reuse
export const WHISPER_CHAR_LIMIT = 896;
export const DEEPGRAM_CHAR_LIMIT = 2000;
export const DEFAULT_CHAR_LIMIT = 896;

// Approximate token calculation
const CHARS_PER_TOKEN = 4;

// Model patterns
const WHISPER_PATTERNS = ["whisper"];
const DEEPGRAM_PATTERNS = ["deepgram", "nova"];
const NO_PROMPT_PATTERNS = ["parakeet", "nemo", "canary"];

export interface ModelPromptInfo {
  supportsPrompt: boolean;
  isWhisperLike: boolean;
  isParakeet: boolean;
  charLimit: number;
  modelId: string;
}

export function getModelPromptInfo(
  modelId: string,
  engineType?: string
): ModelPromptInfo {
  const lower = modelId.toLowerCase();

  // Check engine type first (for local models)
  if (engineType) {
    if (engineType === "Parakeet") {
      return {
        supportsPrompt: false, // Parakeet uses word boosting, not string prompts
        isWhisperLike: false,
        isParakeet: true,
        charLimit: 0,
        modelId,
      };
    }
    if (engineType === "Whisper") {
      return {
        supportsPrompt: true,
        isWhisperLike: true,
        isParakeet: false,
        charLimit: WHISPER_CHAR_LIMIT,
        modelId,
      };
    }
  }

  // Check by model ID patterns
  if (NO_PROMPT_PATTERNS.some((p) => lower.includes(p))) {
    return {
      supportsPrompt: false,
      isWhisperLike: false,
      isParakeet: lower.includes("parakeet"),
      charLimit: 0,
      modelId,
    };
  }

  if (WHISPER_PATTERNS.some((p) => lower.includes(p))) {
    return {
      supportsPrompt: true,
      isWhisperLike: true,
      isParakeet: false,
      charLimit: WHISPER_CHAR_LIMIT,
      modelId,
    };
  }

  if (DEEPGRAM_PATTERNS.some((p) => lower.includes(p))) {
    return {
      supportsPrompt: true,
      isWhisperLike: false,
      isParakeet: false,
      charLimit: DEEPGRAM_CHAR_LIMIT,
      modelId,
    };
  }

  // Unknown - assume supports prompt with default limit
  return {
    supportsPrompt: true,
    isWhisperLike: false,
    isParakeet: false,
    charLimit: DEFAULT_CHAR_LIMIT,
    modelId,
  };
}

export const TranscriptionSystemPrompt: React.FC<{ grouped?: boolean }> = ({
  grouped,
}) => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();
  const { getModelInfo, currentModel: modelIdFromHook } = useModels();

  // Determine active model ID
  const transcriptionProvider = settings?.transcription_provider;
  const isRemoteOpenAi = transcriptionProvider === "remote_openai_compatible";
  const isSoniox = transcriptionProvider === "remote_soniox";

  const activeModelId = useMemo(() => {
    if (isRemoteOpenAi) {
      return settings?.remote_stt?.model_id || "remote-unknown";
    }
    return settings?.selected_model || modelIdFromHook || "";
  }, [
    isRemoteOpenAi,
    settings?.remote_stt?.model_id,
    settings?.selected_model,
    modelIdFromHook,
  ]);

  // Get model info for prompt configuration
  const modelInfo = useMemo(() => {
    if (!activeModelId) {
      return { supportsPrompt: true, isWhisperLike: true, isParakeet: false, charLimit: WHISPER_CHAR_LIMIT, modelId: "" };
    }

    if (isRemoteOpenAi) {
      return getModelPromptInfo(activeModelId);
    }

    const localModelInfo = getModelInfo(activeModelId);
    return getModelPromptInfo(activeModelId, localModelInfo?.engine_type);
  }, [activeModelId, isRemoteOpenAi, getModelInfo]);

  // Get current prompt for this model
  const currentPrompt = useMemo(() => {
    return settings?.transcription_prompts?.[activeModelId] || "";
  }, [settings?.transcription_prompts, activeModelId]);

  // Handle prompt change
  const handleChange = useCallback(
    async (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const newValue = e.target.value;
      
      // Call backend to update
      await commands.changeTranscriptionPromptSetting(activeModelId, newValue);
      
      // Refresh settings to update UI
      refreshSettings();
    },
    [activeModelId, refreshSettings]
  );

  if (isSoniox) {
    return null;
  }

  // Don't show for models without prompt support
  if (!modelInfo.supportsPrompt) {
    return null;
  }

  const charCount = currentPrompt.length;
  const approxTokens = Math.ceil(charCount / CHARS_PER_TOKEN);
  const percentUsed = modelInfo.charLimit > 0 ? (charCount / modelInfo.charLimit) * 100 : 0;
  const isNearLimit = percentUsed > 80;
  const isOverLimit = charCount > modelInfo.charLimit;

  return (
    <SettingContainer
      title={t("settings.general.transcriptionSystemPrompt.title")}
      description={t("settings.general.transcriptionSystemPrompt.tooltip")}
      grouped={grouped}
      layout="stacked"
      descriptionMode="tooltip"
    >
      <div className="space-y-3 w-full pt-1">
        <p className="text-xs text-mid-gray/80 -mt-2 mb-1">
          {t("settings.general.transcriptionSystemPrompt.description")}
        </p>
        {/* Model indicator */}
        <div className="flex items-center gap-2 text-[10px] text-mid-gray/60">
          <span className="px-1.5 py-0.5 rounded bg-white/5 border border-white/10">
            {activeModelId || "No model"}
          </span>
          {modelInfo.isWhisperLike && (
            <span className="text-amber-500/60">{t("settings.general.transcriptionSystemPrompt.whisperLabel", "Whisper")}</span>
          )}
        </div>

        <div className="relative group">
          <Textarea
            value={currentPrompt}
            onChange={handleChange}
            placeholder={t("settings.general.transcriptionSystemPrompt.placeholder")}
            className="w-full min-h-[100px] pr-4 py-3 placeholder:text-mid-gray/50 border-mid-gray/30 focus:border-logo-primary transition-colors resize-none"
          />

          {/* Counter badge */}
          <div className="absolute bottom-3 right-3 pointer-events-none flex gap-2">
            {/* Token estimate */}
            <div className="px-2 py-0.5 rounded text-[10px] font-mono backdrop-blur-md border bg-white/5 text-mid-gray/60 border-white/5">
              {t("settings.general.transcriptionSystemPrompt.tokensApprox", "~{{count}} tok", { count: approxTokens })}
            </div>
            {/* Character count */}
            <div
              className={`px-2 py-0.5 rounded text-[10px] font-mono backdrop-blur-md border ${
                isOverLimit
                  ? "bg-red-500/10 text-red-400/90 border-red-500/20"
                  : isNearLimit
                    ? "bg-amber-500/10 text-amber-500/80 border-amber-500/20"
                    : "bg-white/5 text-mid-gray/60 border-white/5"
              }`}
            >
              {charCount} / {modelInfo.charLimit}
            </div>
          </div>
        </div>

        {/* Limit warning */}
        {isNearLimit && (
          <div
            className={`flex items-start space-x-2 p-2 rounded border animate-in fade-in slide-in-from-top-1 duration-200 ${
              isOverLimit
                ? "bg-red-500/5 border-red-500/10"
                : "bg-amber-500/5 border-amber-500/10"
            }`}
          >
            <svg
              className={`w-4 h-4 mt-0.5 flex-shrink-0 ${isOverLimit ? "text-red-400/70" : "text-amber-500/70"}`}
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
              />
            </svg>
            <p
              className={`text-[11px] italic leading-tight ${isOverLimit ? "text-red-400/80" : "text-amber-500/80"}`}
            >
              {isOverLimit
                ? t("settings.general.transcriptionSystemPrompt.overLimit", {
                    limit: modelInfo.charLimit,
                  })
                : t("settings.general.transcriptionSystemPrompt.nearLimit", {
                    limit: modelInfo.charLimit,
                  })}
            </p>
          </div>
        )}

        {/* Remote API note for unknown models */}
        {isRemoteOpenAi && !modelInfo.isWhisperLike && !isNearLimit && (
          <div className="flex items-start space-x-2 p-2 rounded bg-blue-500/5 border border-blue-500/10">
            <svg
              className="w-4 h-4 text-blue-400/70 mt-0.5 flex-shrink-0"
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
            <p className="text-[11px] text-blue-400/80 italic leading-tight">
              {t("settings.general.transcriptionSystemPrompt.remoteNote")}
            </p>
          </div>
        )}
      </div>
    </SettingContainer>
  );
};
