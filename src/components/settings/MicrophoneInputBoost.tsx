import React from "react";
import { useTranslation } from "react-i18next";
import { AlertTriangle, RotateCcw } from "lucide-react";
import { SettingContainer } from "../ui/SettingContainer";
import { useSettings } from "../../hooks/useSettings";

type MicSettingKey = "selected_microphone" | "live_sound_microphone";

interface MicrophoneInputBoostProps {
  settingKey?: MicSettingKey;
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  titleOverride?: string;
  disabled?: boolean;
}

const DEFAULT_MICROPHONE_INPUT_BOOST_DEVICE_KEY = "__default__";

export const MicrophoneInputBoost: React.FC<MicrophoneInputBoostProps> = React.memo(
  ({
    settingKey = "selected_microphone",
    descriptionMode = "tooltip",
    grouped = false,
    titleOverride,
    disabled = false,
  }) => {
    const { t } = useTranslation();
    const { settings, getSetting, isUpdating, updateMicrophoneInputBoostForDevice } =
      useSettings();

    const selectedMicrophone =
      getSetting(settingKey) === "default"
        ? "Default"
        : getSetting(settingKey) || "Default";
    const deviceKey =
      selectedMicrophone === "Default"
        ? DEFAULT_MICROPHONE_INPUT_BOOST_DEVICE_KEY
        : selectedMicrophone;
    const boostMap = (((settings as any)?.microphone_input_boost_db_by_device ?? {}) as Record<
      string,
      number
    >);
    const microphoneInputBoostDb =
      boostMap[deviceKey] ??
      (((settings as any)?.microphone_input_boost_db as number | undefined) ?? 0);
    const updateKey = `microphone_input_boost_db_by_device:${deviceKey}`;
    const isBusy = disabled || isUpdating(updateKey);

    const handleReset = async () => {
      await updateMicrophoneInputBoostForDevice(selectedMicrophone, 0);
    };

    const handleChange = async (value: number) => {
      await updateMicrophoneInputBoostForDevice(selectedMicrophone, value);
    };

    return (
      <SettingContainer
        title={
          titleOverride ??
          t("settings.sound.microphone.boost.title", "Microphone Input Boost")
        }
        description={t(
          "settings.sound.microphone.boost.description",
          "Saved separately for each microphone device.",
        )}
        descriptionMode={descriptionMode}
        grouped={grouped}
        disabled={disabled}
      >
        <div className="space-y-3">
          <div className="flex items-center justify-between gap-3">
            <span className="text-xs text-[#808080]">
              {t("settings.sound.microphone.boost.appliesTo", {
                defaultValue: "Applies to: {{name}}",
                name: selectedMicrophone,
              })}
            </span>
            <div className="flex items-center gap-3">
              <span className="text-sm text-[#9b5de5] font-mono min-w-[52px] text-right">
                {microphoneInputBoostDb <= 0
                  ? t("settings.sound.microphone.boost.off", "Off")
                  : `+${microphoneInputBoostDb.toFixed(0)} dB`}
              </span>
              <button
                onClick={() => void handleReset()}
                disabled={isBusy}
                className="p-1.5 text-[#606060] hover:text-[#f5f5f5] transition-colors rounded-md hover:bg-[#333333]"
                title={t("common.reset", "Reset")}
              >
                <RotateCcw className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>

          <input
            type="range"
            min="0"
            max="12"
            step="1"
            value={microphoneInputBoostDb}
            onChange={(e) => void handleChange(parseFloat(e.target.value))}
            className="w-full h-2 bg-[#252525] rounded-lg appearance-none cursor-pointer accent-[#9b5de5]"
            disabled={isBusy}
          />

          <div className="flex justify-between text-xs text-[#606060]">
            <span>{t("settings.sound.microphone.boost.off", "Off")}</span>
            <span>{t("settings.sound.microphone.boost.max", "+12 dB")}</span>
          </div>

          <div className="p-3 bg-amber-500/10 border border-amber-500/30 rounded-lg">
            <div className="flex items-start gap-2">
              <AlertTriangle className="w-4 h-4 text-amber-300 mt-0.5 flex-shrink-0" />
              <p className="text-xs text-amber-100/90">
                {t(
                  "settings.sound.microphone.boost.warning",
                  "May help a quiet mic, but it can also amplify room noise, breathing, keyboard sounds, and clipping.",
                )}
              </p>
            </div>
          </div>
        </div>
      </SettingContainer>
    );
  },
);

MicrophoneInputBoost.displayName = "MicrophoneInputBoost";
