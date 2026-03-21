import React from "react";
import { useTranslation } from "react-i18next";
import { Slider } from "../../ui/Slider";
import { useSettings } from "../../../hooks/useSettings";

interface RecordingBufferProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

export const RecordingBuffer: React.FC<RecordingBufferProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const { settings, updateSetting } = useSettings();
  const value = (settings as any)?.extra_recording_buffer_ms ?? 0;

  return (
    <Slider
      value={value}
      onChange={(nextValue) =>
        updateSetting("extra_recording_buffer_ms" as any, nextValue as any)
      }
      min={0}
      max={1500}
      step={50}
      label={t("settings.debug.recordingBuffer.title")}
      description={t("settings.debug.recordingBuffer.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
      formatValue={(nextValue) => `${nextValue}ms`}
    />
  );
};
