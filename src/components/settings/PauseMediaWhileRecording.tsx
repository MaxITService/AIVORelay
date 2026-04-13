import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface PauseMediaWhileRecordingToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const PauseMediaWhileRecording: React.FC<PauseMediaWhileRecordingToggleProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const pauseEnabled =
      (getSetting as any)("pause_media_while_recording") ?? false;

    return (
      <ToggleSwitch
        checked={pauseEnabled}
        onChange={(enabled) =>
          updateSetting("pause_media_while_recording" as any, enabled as any)
        }
        isUpdating={isUpdating("pause_media_while_recording")}
        label={t("settings.advanced.pauseMediaWhileRecording.label")}
        description={t(
          "settings.advanced.pauseMediaWhileRecording.description",
        )}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  });
