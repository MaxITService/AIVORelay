import React from "react";
import { useTranslation } from "react-i18next";
import { AutomaticMicrophoneMask } from "../AutomaticMicrophoneMask";
import { MicrophoneInputBoost } from "../MicrophoneInputBoost";
import { MicrophoneNoiseCancellation } from "../MicrophoneNoiseCancellation";
import { MicrophoneSelector } from "../MicrophoneSelector";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { OutputDeviceSelector } from "../OutputDeviceSelector";
import { AudioFeedback } from "../AudioFeedback";
import { useSettings } from "../../../hooks/useSettings";
import { VolumeSlider } from "../VolumeSlider";
import { TranscriptionProfiles } from "../TranscriptionProfiles";
import { Button } from "../../ui/Button";
import { useNavigationStore } from "../../../stores/navigationStore";

export const GeneralSettings: React.FC = () => {
  const { t } = useTranslation();
  const { audioFeedbackEnabled, settings } = useSettings();
  const anyFeedbackEnabled =
    audioFeedbackEnabled || Boolean(settings?.result_ready_audio_feedback);
  const openHelp = useNavigationStore((state) => state.openHelp);

  return (
    <div className="max-w-3xl w-full mx-auto space-y-8 pb-12">
      <div>
        <TranscriptionProfiles />
      </div>

      <div>
        <SettingsGroup title={t("settings.sound.title")}>
          <MicrophoneSelector descriptionMode="tooltip" grouped={true} />
          <MicrophoneInputBoost descriptionMode="tooltip" grouped={true} />
          <MicrophoneNoiseCancellation
            descriptionMode="tooltip"
            grouped={true}
          />
          <AutomaticMicrophoneMask descriptionMode="tooltip" grouped={true} />
          <div>
            <AudioFeedback descriptionMode="tooltip" grouped={true} />
            <OutputDeviceSelector
              descriptionMode="tooltip"
              grouped={true}
              disabled={!anyFeedbackEnabled}
            />
            <VolumeSlider disabled={!anyFeedbackEnabled} />
          </div>
        </SettingsGroup>
      </div>

      <div className="flex flex-col gap-3 rounded-lg border border-[#ff4d8d]/25 bg-[#1a1a1a] p-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="min-w-0">
          <p className="text-sm font-semibold text-[#f5f5f5]">
            {t("settings.generalHelp.title")}
          </p>
          <p className="mt-1 text-xs leading-relaxed text-[#b8b8b8]">
            {t("settings.generalHelp.description")}
          </p>
        </div>
        <Button
          variant="secondary"
          size="sm"
          onClick={() => openHelp("help-transcription")}
          className="shrink-0 whitespace-nowrap"
        >
          {t("settings.generalHelp.action")}
        </Button>
      </div>
    </div>
  );
};
