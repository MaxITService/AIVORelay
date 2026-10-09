import React from "react";
import { useTranslation } from "react-i18next";
import { Slider } from "../ui/Slider";
import { useSettings } from "../../hooks/useSettings";

export const VolumeSlider: React.FC<{ disabled?: boolean }> = ({
  disabled = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting } = useSettings();
  const audioFeedbackVolume = getSetting("audio_feedback_volume") ?? 1.0;

  return (
    <Slider
      id="settings-feedback-volume"
      value={audioFeedbackVolume}
      onChange={(value: number) =>
        updateSetting("audio_feedback_volume", value)
      }
      min={0}
      max={1}
      label={t("settings.sound.volume.title")}
      description={t("settings.sound.volume.description")}
      descriptionMode="tooltip"
      grouped
      formatValue={(value) => `${Math.round(value * 100)}%`}
      disabled={disabled}
    />
  );
};
