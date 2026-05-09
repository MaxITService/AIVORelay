import React from "react";
import { useTranslation } from "react-i18next";
import { Wind } from "lucide-react";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { SettingContainer } from "../ui/SettingContainer";
import { useSettings } from "../../hooks/useSettings";

interface MicrophoneNoiseCancellationProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const MicrophoneNoiseCancellation: React.FC<MicrophoneNoiseCancellationProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const enabled =
      (getSetting("microphone_noise_cancellation_enabled") ?? false) === true;

    return (
      <SettingContainer
        title={
          <span className="inline-flex items-center gap-2">
            <Wind className="h-4 w-4 text-[#9b5de5]" />
            <span>
              {t(
                "settings.sound.microphone.noiseCancellation.title",
                "Noise Cancellation",
              )}
            </span>
          </span>
        }
        description={t(
          "settings.sound.microphone.noiseCancellation.description",
          "Uses RNNoise to reduce steady background noise from microphone input before voice detection and speech-to-text. Leave it off if your voice becomes metallic or clipped.",
        )}
        descriptionMode={descriptionMode}
        grouped={grouped}
      >
        <ToggleSwitch
          checked={enabled}
          onChange={(checked) =>
            updateSetting("microphone_noise_cancellation_enabled", checked)
          }
          isUpdating={isUpdating("microphone_noise_cancellation_enabled")}
        />
      </SettingContainer>
    );
  });

MicrophoneNoiseCancellation.displayName = "MicrophoneNoiseCancellation";
