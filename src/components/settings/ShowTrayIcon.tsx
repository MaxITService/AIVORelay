import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { Slider } from "../ui/Slider";
import { useSettings } from "../../hooks/useSettings";

interface ShowTrayIconProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  id?: string;
}

export const ShowTrayIcon: React.FC<ShowTrayIconProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false, id }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating, refreshSettings } =
      useSettings();

    const showTrayIcon = getSetting("show_tray_icon") ?? true;
    const trayIconBlinkingEnabled =
      getSetting("tray_icon_blinking_enabled") ?? false;
    const trayIconBlinkOnRecording =
      getSetting("tray_icon_blink_on_recording") ?? false;
    const trayIconBlinkOnProcessing =
      getSetting("tray_icon_blink_on_processing") ?? true;
    const trayIconBlinkFrequencyHz =
      getSetting("tray_icon_blink_frequency_hz") ?? 1;

    const changeTrayIconBlinking = async (enabled: boolean) => {
      try {
        await updateSetting("tray_icon_blinking_enabled", enabled, {
          throwOnError: true,
        });
        if (enabled) await refreshSettings();
      } catch {
        // updateSetting already reports the failure and restores the prior value.
      }
    };

    return (
      <div
        id={id}
        tabIndex={id ? -1 : undefined}
        className="space-y-3 outline-none"
      >
        <ToggleSwitch
          checked={showTrayIcon}
          onChange={(enabled) => updateSetting("show_tray_icon", enabled)}
          isUpdating={isUpdating("show_tray_icon")}
          label={t("settings.userInterface.showTrayIcon.label")}
          description={t("settings.userInterface.showTrayIcon.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
          tooltipPosition="bottom"
        />

        {showTrayIcon && (
          <>
            <ToggleSwitch
              checked={trayIconBlinkingEnabled}
              onChange={(enabled) => void changeTrayIconBlinking(enabled)}
              isUpdating={isUpdating("tray_icon_blinking_enabled")}
              label={t(
                "settings.userInterface.trayIconBlinking.label",
                "Blink Tray Icon",
              )}
              description={t(
                "settings.userInterface.trayIconBlinking.description",
                "Flashes the tray icon between the idle state and the active ear icon during processing and recording.",
              )}
              descriptionMode={descriptionMode}
              grouped={grouped}
              tooltipPosition="bottom"
            />

            {trayIconBlinkingEnabled && (
              <div className="pl-4 space-y-3 border-l-2 border-primary/20 ml-2">
                <ToggleSwitch
                  checked={trayIconBlinkOnProcessing}
                  onChange={(enabled) =>
                    updateSetting("tray_icon_blink_on_processing", enabled)
                  }
                  isUpdating={isUpdating("tray_icon_blink_on_processing")}
                  label={t(
                    "settings.userInterface.trayIconBlinkOnProcessing.label",
                    "Blink during processing & finalization",
                  )}
                  description={t(
                    "settings.userInterface.trayIconBlinkOnProcessing.description",
                    "Flash between the idle logo and the ear icon during transcription, post-processing, and finalization.",
                  )}
                  descriptionMode={descriptionMode}
                  grouped={grouped}
                  tooltipPosition="bottom"
                />

                <ToggleSwitch
                  checked={trayIconBlinkOnRecording}
                  onChange={(enabled) =>
                    updateSetting("tray_icon_blink_on_recording", enabled)
                  }
                  isUpdating={isUpdating("tray_icon_blink_on_recording")}
                  label={t(
                    "settings.userInterface.trayIconBlinkOnRecording.label",
                    "Blink during recording",
                  )}
                  description={t(
                    "settings.userInterface.trayIconBlinkOnRecording.description",
                    "Flash between the idle logo and the ear icon while audio recording is in progress.",
                  )}
                  descriptionMode={descriptionMode}
                  grouped={grouped}
                  tooltipPosition="bottom"
                />

                <Slider
                  value={trayIconBlinkFrequencyHz}
                  onChange={(val) =>
                    updateSetting("tray_icon_blink_frequency_hz", val)
                  }
                  min={0.5}
                  max={10}
                  step={0.5}
                  label={t(
                    "settings.userInterface.trayIconBlinkFrequency.label",
                    "Blinking Frequency (Hz)",
                  )}
                  description={t(
                    "settings.userInterface.trayIconBlinkFrequency.description",
                    "Sets the blink rate from 0.5 to 10 Hz. At 1 Hz, the icon changes every 0.5 seconds.",
                  )}
                  formatValue={(v) =>
                    `${Number.isInteger(v) ? v.toFixed(0) : v.toFixed(1)} Hz`
                  }
                  descriptionMode={descriptionMode}
                  grouped={grouped}
                />
              </div>
            )}
          </>
        )}
      </div>
    );
  },
);
