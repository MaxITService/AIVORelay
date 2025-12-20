import React, { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";
import { useModels } from "../../hooks/useModels";

interface TranslateToEnglishProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

const unsupportedTranslationModels = [
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
    const isRemoteProvider =
      transcriptionProvider === "remote_openai_compatible";
    const isDisabledTranslation =
      unsupportedTranslationModels.includes(currentModel);

    const description = useMemo(() => {
      if (isRemoteProvider) {
        return t(
          "settings.advanced.translateToEnglish.descriptionRemoteUnsupported",
        );
      }
      if (isDisabledTranslation) {
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
    }, [t, models, currentModel, isDisabledTranslation, isRemoteProvider]);

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
        disabled={isDisabledTranslation || isRemoteProvider}
        label={t("settings.advanced.translateToEnglish.label")}
        description={description}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
