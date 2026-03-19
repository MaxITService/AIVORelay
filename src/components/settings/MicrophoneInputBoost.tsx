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
    const { settings, getSetting, updateMicrophoneInputBoostForDevice } =
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

    const [draftValue, setDraftValue] = React.useState(microphoneInputBoostDb);
    const [isInteracting, setIsInteracting] = React.useState(false);
    const [isSaving, setIsSaving] = React.useState(false);
    const latestValueRef = React.useRef(microphoneInputBoostDb);

    React.useEffect(() => {
      latestValueRef.current = draftValue;
    }, [draftValue]);

    React.useEffect(() => {
      if (isInteracting || isSaving) {
        return;
      }

      setDraftValue(microphoneInputBoostDb);
      latestValueRef.current = microphoneInputBoostDb;
    }, [isInteracting, isSaving, microphoneInputBoostDb, deviceKey]);

    const commitValue = React.useCallback(
      async (nextValue?: number) => {
        const valueToCommit = Math.max(
          0,
          Math.min(12, nextValue ?? latestValueRef.current),
        );
        setIsInteracting(false);

        if (valueToCommit === microphoneInputBoostDb) {
          setDraftValue(microphoneInputBoostDb);
          latestValueRef.current = microphoneInputBoostDb;
          return;
        }

        setIsSaving(true);
        try {
          await updateMicrophoneInputBoostForDevice(
            selectedMicrophone,
            valueToCommit,
          );
        } finally {
          setIsSaving(false);
        }
      },
      [
        microphoneInputBoostDb,
        selectedMicrophone,
        updateMicrophoneInputBoostForDevice,
      ],
    );

    React.useEffect(() => {
      if (!isInteracting) {
        return;
      }

      const handleInteractionEnd = () => {
        void commitValue();
      };

      window.addEventListener("mouseup", handleInteractionEnd);
      window.addEventListener("touchend", handleInteractionEnd);
      window.addEventListener("touchcancel", handleInteractionEnd);

      return () => {
        window.removeEventListener("mouseup", handleInteractionEnd);
        window.removeEventListener("touchend", handleInteractionEnd);
        window.removeEventListener("touchcancel", handleInteractionEnd);
      };
    }, [commitValue, isInteracting]);

    const handleReset = async () => {
      setDraftValue(0);
      latestValueRef.current = 0;
      await commitValue(0);
    };

    const displayValue = draftValue <= 0 ? 0 : draftValue;
    const isBusy = disabled || isSaving;

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
        layout="stacked"
        disabled={disabled}
      >
        <div className="w-full space-y-3">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <span className="text-xs text-[#808080] break-all">
              {t("settings.sound.microphone.boost.appliesTo", {
                defaultValue: "Applies to: {{name}}",
                name: selectedMicrophone,
              })}
            </span>
            <div className="flex items-center gap-3 ml-auto">
              <span className="text-sm text-[#9b5de5] font-mono min-w-[52px] text-right">
                {displayValue <= 0
                  ? t("settings.sound.microphone.boost.off", "Off")
                  : `+${displayValue.toFixed(0)} dB`}
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
            value={displayValue}
            onChange={(e) => {
              const nextValue = parseFloat(e.target.value);
              setDraftValue(nextValue);
              latestValueRef.current = nextValue;
            }}
            onMouseDown={() => setIsInteracting(true)}
            onTouchStart={() => setIsInteracting(true)}
            onBlur={() => {
              if (!isInteracting) {
                void commitValue();
              }
            }}
            onKeyUp={(event) => {
              if (
                event.key.startsWith("Arrow") ||
                event.key === "Home" ||
                event.key === "End" ||
                event.key === "PageUp" ||
                event.key === "PageDown"
              ) {
                void commitValue();
              }
            }}
            className="w-full h-2 rounded-full appearance-none cursor-pointer focus:outline-none focus:ring-2 focus:ring-[#9b5de5]/40 disabled:opacity-40 disabled:cursor-not-allowed"
            style={{
              background: `linear-gradient(to right, #9b5de5 ${
                (displayValue / 12) * 100
              }%, #252525 ${(displayValue / 12) * 100}%)`,
            }}
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
