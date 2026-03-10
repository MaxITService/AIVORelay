import React from "react";
import { useTranslation } from "react-i18next";
import { Dropdown } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";
import { ResetButton } from "../ui/ResetButton";
import { useSettings } from "../../hooks/useSettings";

type MicSettingKey = "selected_microphone" | "live_sound_microphone";

interface MicrophoneSelectorProps {
  settingKey?: MicSettingKey;
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  descriptionOverride?: string;
  titleOverride?: string;
  disabled?: boolean;
}

export const MicrophoneSelector: React.FC<MicrophoneSelectorProps> = React.memo(
  ({
    settingKey = "selected_microphone",
    descriptionMode = "tooltip",
    grouped = false,
    descriptionOverride,
    titleOverride,
    disabled = false,
  }) => {
    const { t } = useTranslation();
    const {
      getSetting,
      updateSetting,
      resetSetting,
      isUpdating,
      isLoading,
      audioDevices,
      refreshAudioDevices,
    } = useSettings();

    const selectedMicrophone =
      getSetting(settingKey) === "default"
        ? "Default"
        : getSetting(settingKey) || "Default";

    const handleMicrophoneSelect = async (deviceName: string) => {
      await updateSetting(settingKey, deviceName);
    };

    const handleReset = async () => {
      await resetSetting(settingKey);
    };

    const microphoneOptions = audioDevices.map((device) => ({
      value: device.name,
      label: device.name,
    }));

    return (
      <SettingContainer
        title={titleOverride ?? t("settings.sound.microphone.title")}
        description={
          descriptionOverride ?? t("settings.sound.microphone.description")
        }
        descriptionMode={descriptionMode}
        grouped={grouped}
        disabled={disabled}
      >
        <div className="flex items-center space-x-1">
          <Dropdown
            options={microphoneOptions}
            selectedValue={selectedMicrophone}
            onSelect={handleMicrophoneSelect}
            placeholder={
              isLoading || audioDevices.length === 0
                ? t("settings.sound.microphone.loading")
                : t("settings.sound.microphone.placeholder")
            }
            disabled={
              disabled ||
              isUpdating(settingKey) ||
              isLoading ||
              audioDevices.length === 0
            }
            onRefresh={refreshAudioDevices}
          />
          <ResetButton
            onClick={handleReset}
            disabled={disabled || isUpdating(settingKey) || isLoading}
          />
        </div>
      </SettingContainer>
    );
  },
);
