import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface QueueRecordingDuringProcessingToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const QueueRecordingDuringProcessing: React.FC<QueueRecordingDuringProcessingToggleProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const queueEnabled =
      (getSetting as any)("queue_recording_during_processing") ?? false;

    return (
      <ToggleSwitch
        id="settings-queue-recording-during-processing"
        checked={queueEnabled}
        onChange={(enabled) =>
          updateSetting(
            "queue_recording_during_processing" as any,
            enabled as any,
          )
        }
        isUpdating={isUpdating("queue_recording_during_processing")}
        label={t("settings.advanced.queueRecordingDuringProcessing.label")}
        description={t(
          "settings.advanced.queueRecordingDuringProcessing.description",
        )}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  });
