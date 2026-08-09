import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";
import { getPostProcessingAvailability } from "../../lib/postProcessingAvailability";

interface PostProcessingToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const PostProcessingToggle: React.FC<PostProcessingToggleProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { settings, getSetting, updateSetting, isUpdating } = useSettings();

    const configuredEnabled = getSetting("post_process_enabled") || false;
    const availability = getPostProcessingAvailability(settings);
    const enabled = availability.available && configuredEnabled;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(enabled) => updateSetting("post_process_enabled", enabled)}
        isUpdating={isUpdating("post_process_enabled")}
        disabled={!availability.available}
        label={t("settings.debug.postProcessingToggle.label")}
        description={
          availability.available
            ? t("settings.debug.postProcessingToggle.description")
            : t(
                "settings.postProcessing.unavailableDirectRealtime",
                "LLM post-processing is unavailable while realtime text is inserted directly into the target application. Use Preview or a non-live output route. Your saved choice is preserved for compatible routes.",
              )
        }
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  });
