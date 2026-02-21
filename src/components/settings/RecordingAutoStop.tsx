import React from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { Input } from "../ui/Input";
import { SettingContainer } from "../ui/SettingContainer";

interface RecordingAutoStopProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

export const RecordingAutoStop: React.FC<RecordingAutoStopProps> = ({
  descriptionMode = "inline",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  const enabled = getSetting("recording_auto_stop_enabled" as any) ?? false;
  const timeoutRaw = getSetting("recording_auto_stop_timeout_seconds" as any) ?? 1800;
  const paste = getSetting("recording_auto_stop_paste" as any) ?? false;

  const timeout = Number.isFinite(timeoutRaw)
    ? Math.min(7200, Math.max(10, Number(timeoutRaw)))
    : 1800;

  const handleTimeoutChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const value = parseInt(event.target.value, 10);
    if (!isNaN(value)) {
      const clamped = Math.min(7200, Math.max(10, value));
      updateSetting("recording_auto_stop_timeout_seconds" as any, clamped);
    }
  };

  return (
    <div className="flex flex-col">
      <SettingContainer
        title={t("settings.advanced.autoStop.title")}
        description={t("settings.advanced.autoStop.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
        layout="horizontal"
      >
        <ToggleSwitch
          checked={enabled}
          onChange={(checked) =>
            updateSetting("recording_auto_stop_enabled" as any, checked)
          }
          disabled={isUpdating("recording_auto_stop_enabled" as any)}
        />
      </SettingContainer>

      {enabled && (
        <div className="pl-4 ml-6 border-l-2 border-surface-highlight py-2 space-y-4 relative -top-2">
          <SettingContainer
            title={t("settings.advanced.autoStop.timeoutTitle")}
            description={t("settings.advanced.autoStop.timeoutDescription")}
            descriptionMode={descriptionMode}
            grouped={true}
            layout="horizontal"
          >
            <div className="flex items-center space-x-2">
              <Input
                type="number"
                min={10}
                max={7200}
                value={timeout}
                onChange={handleTimeoutChange}
                disabled={isUpdating("recording_auto_stop_timeout_seconds" as any)}
                className="w-24 text-right"
              />
              <span className="text-sm text-text/70">
                {t("settings.advanced.autoStop.seconds")}
              </span>
            </div>
          </SettingContainer>

          <SettingContainer
            title={t("settings.advanced.autoStop.pasteTitle")}
            description={t("settings.advanced.autoStop.pasteDescription")}
            descriptionMode={descriptionMode}
            grouped={true}
            layout="horizontal"
          >
            <ToggleSwitch
              checked={paste}
              onChange={(checked) =>
                updateSetting("recording_auto_stop_paste" as any, checked)
              }
              disabled={isUpdating("recording_auto_stop_paste" as any)}
            />
          </SettingContainer>
        </div>
      )}
    </div>
  );
};
