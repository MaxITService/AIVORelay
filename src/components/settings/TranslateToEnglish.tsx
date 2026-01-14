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

// Local models that don't support translation
const unsupportedLocalModels = [
  "parakeet-tdt-0.6b-v2",
  "parakeet-tdt-0.6b-v3",
  "turbo",
];

export const TranslateToEnglish: React.FC<TranslateToEnglishProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const { currentModel, loadCurrentModel, models } = useModels();

    const translateToEnglish = getSetting("translate_to_english") || false;
    const transcriptionProvider =
      getSetting("transcription_provider") || "local";
    const remoteModelId = getSetting("remote_stt")?.model_id || "";
    const isRemoteProvider =
      transcriptionProvider === "remote_openai_compatible";

    // Track whether the remote model supports translation
    const [remoteSupportsTranslation, setRemoteSupportsTranslation] =
      useState(false);

    // Check remote model translation support
    const checkRemoteTranslationSupport = useCallback(async () => {
      if (isRemoteProvider) {
        const result = await commands.remoteSttSupportsTranslation();
        if (result.status === "ok") {
          setRemoteSupportsTranslation(result.data);
        }
      }
    }, [isRemoteProvider]);

    // Check translation support when provider or remote model changes
    useEffect(() => {
      checkRemoteTranslationSupport();
    }, [checkRemoteTranslationSupport, remoteModelId]);

    // Determine if translation is disabled
    const isDisabledTranslation = useMemo(() => {
      if (isRemoteProvider) {
        // For remote: disabled if model doesn't support translation
        return !remoteSupportsTranslation;
      }
      // For local: disabled if model is in unsupported list
      return unsupportedLocalModels.includes(currentModel);
    }, [isRemoteProvider, remoteSupportsTranslation, currentModel]);

    const description = useMemo(() => {
      if (isRemoteProvider && !remoteSupportsTranslation) {
        return t(
          "settings.advanced.translateToEnglish.descriptionRemoteUnsupported",
        );
      }
      if (!isRemoteProvider && unsupportedLocalModels.includes(currentModel)) {
        const currentModelDisplayName = models.find(
          (model) => model.id === currentModel,
        )?.name;
        return t(
          "settings.advanced.translateToEnglish.descriptionUnsupported",
          {
            model: currentModelDisplayName,
          },
        );
      }

      return t("settings.advanced.translateToEnglish.description");
    }, [
      t,
      models,
      currentModel,
      isRemoteProvider,
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
