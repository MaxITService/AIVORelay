import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface MuteWhileRecordingToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const MuteWhileRecording: React.FC<MuteWhileRecordingToggleProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const muteEnabled = getSetting("mute_while_recording") ?? false;
    const muteImmediately = getSetting("mute_immediately") ?? false;

    return (
      <>
        <ToggleSwitch
          id="settings-mute-while-recording"
          checked={muteEnabled}
          onChange={(enabled) => updateSetting("mute_while_recording", enabled)}
          isUpdating={isUpdating("mute_while_recording")}
          label={t("settings.advanced.muteWhileRecording.label")}
          description={t("settings.advanced.muteWhileRecording.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        />
        {muteEnabled && (
          <div className="ms-6 border-s border-mid-gray/20">
            <ToggleSwitch
              id="settings-mute-immediately"
              checked={muteImmediately}
              onChange={(enabled) => updateSetting("mute_immediately", enabled)}
              isUpdating={isUpdating("mute_immediately")}
              label={t("settings.advanced.muteImmediately.label")}
              description={t("settings.advanced.muteImmediately.description")}
              descriptionMode={descriptionMode}
              grouped={grouped}
            />
          </div>
        )}
      </>
    );
  });
