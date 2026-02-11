import React, { useEffect, useMemo, useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";
import { useModels } from "../../hooks/useModels";
import { commands } from "@/bindings";

interface TranslateToEnglishProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const TranslateToEnglish: React.FC<TranslateToEnglishProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const { currentModel, loadCurrentModel, models } = useModels();

    const translateToEnglish = getSetting("translate_to_english") || false;
    const transcriptionProvider =
      getSetting("transcription_provider") || "local";
    const remoteModelId = getSetting("remote_stt")?.model_id || "";
    const isRemoteOpenAiProvider =
      transcriptionProvider === "remote_openai_compatible";
    const isSonioxProvider = transcriptionProvider === "remote_soniox";

    // Track whether the remote model supports translation
    const [remoteSupportsTranslation, setRemoteSupportsTranslation] =
      useState(false);
    const currentModelInfo = models.find((model) => model.id === currentModel);

    // Check remote model translation support
    const checkRemoteTranslationSupport = useCallback(async () => {
      if (isRemoteOpenAiProvider) {
        const result = await commands.remoteSttSupportsTranslation();
        setRemoteSupportsTranslation(result);
      } else {
        setRemoteSupportsTranslation(false);
      }
    }, [isRemoteOpenAiProvider]);

    // Check translation support when provider or remote model changes
    useEffect(() => {
      checkRemoteTranslationSupport();
    }, [checkRemoteTranslationSupport, remoteModelId]);

    // Determine if translation is disabled
    const isDisabledTranslation = useMemo(() => {
      if (isSonioxProvider) {
        return true;
      }
      if (isRemoteOpenAiProvider) {
        // For remote: disabled if model doesn't support translation
        return !remoteSupportsTranslation;
      }
      // For local: disabled if model is in unsupported list
      if (currentModelInfo) {
        return !currentModelInfo.supports_translation;
      }
      // Conservative fallback for unknown local model metadata
      return false;
    }, [
      isSonioxProvider,
      isRemoteOpenAiProvider,
      remoteSupportsTranslation,
      currentModelInfo,
    ]);

    const description = useMemo(() => {
      if (isSonioxProvider) {
        return t(
          "settings.advanced.translateToEnglish.descriptionRemoteUnsupported",
        );
      }
      if (isRemoteOpenAiProvider && !remoteSupportsTranslation) {
        return t(
          "settings.advanced.translateToEnglish.descriptionRemoteUnsupported",
        );
      }
      if (
        !isRemoteOpenAiProvider &&
        !isSonioxProvider &&
        currentModelInfo &&
        !currentModelInfo.supports_translation
      ) {
        return t(
          "settings.advanced.translateToEnglish.descriptionUnsupported",
          {
            model: currentModelInfo.name,
          },
        );
      }

      return t("settings.advanced.translateToEnglish.description");
    }, [
      t,
      currentModelInfo,
      isSonioxProvider,
      isRemoteOpenAiProvider,
      remoteSupportsTranslation,
    ]);

    // Listen for model state changes to update UI reactively
    useEffect(() => {
      const modelStateUnlisten = listen("model-state-changed", () => {
        loadCurrentModel();
      });

      return () => {
        modelStateUnlisten.then((fn) => fn());
      };
    }, [loadCurrentModel]);

    return (
      <ToggleSwitch
        checked={translateToEnglish}
        onChange={(enabled) => updateSetting("translate_to_english", enabled)}
        isUpdating={isUpdating("translate_to_english")}
        disabled={isDisabledTranslation}
        label={t("settings.advanced.translateToEnglish.label")}
        description={description}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
