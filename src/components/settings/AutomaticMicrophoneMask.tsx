import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { Input } from "../ui/Input";
import { ResetButton } from "../ui/ResetButton";
import { SettingContainer } from "../ui/SettingContainer";
import { ToggleSwitch } from "../ui/ToggleSwitch";

interface AutomaticMicrophoneMaskProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const AutomaticMicrophoneMask: React.FC<
  AutomaticMicrophoneMaskProps
> = React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  const enabled =
    getSetting("selected_microphone_auto_switch_enabled") ?? false;
  const savedMask = getSetting("selected_microphone_name_pattern") ?? "";
  const [maskInput, setMaskInput] = useState(savedMask);

  useEffect(() => {
    setMaskInput(savedMask);
  }, [savedMask]);

  const commitMask = async () => {
    const normalized = maskInput.trim();
    if (normalized === savedMask) {
      return;
    }
    await updateSetting("selected_microphone_name_pattern", normalized);
  };

  const resetMask = async () => {
    setMaskInput("");
    await updateSetting("selected_microphone_name_pattern", "");
  };

  return (
    <>
      <ToggleSwitch
        checked={enabled}
        onChange={(checked) =>
          updateSetting("selected_microphone_auto_switch_enabled", checked)
        }
        isUpdating={isUpdating("selected_microphone_auto_switch_enabled")}
        label={t("settings.sound.microphone.autoSelect.label")}
        description={t("settings.sound.microphone.autoSelect.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />

      <SettingContainer
        title={t("settings.sound.microphone.autoSelectMask.title")}
        description={t("settings.sound.microphone.autoSelectMask.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
        layout="stacked"
        disabled={!enabled}
      >
        <div className="flex items-center gap-2">
          <Input
            value={maskInput}
            onChange={(event) => setMaskInput(event.target.value)}
            onBlur={() => void commitMask()}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.currentTarget.blur();
              }
            }}
            disabled={!enabled || isUpdating("selected_microphone_name_pattern")}
            placeholder={t("settings.sound.microphone.autoSelectMask.placeholder")}
            className="w-full"
          />
          <ResetButton
            onClick={() => void resetMask()}
            disabled={
              !enabled ||
              isUpdating("selected_microphone_name_pattern") ||
              savedMask.length === 0
            }
            ariaLabel={t("settings.sound.microphone.autoSelectMask.reset")}
            title={t("settings.sound.microphone.autoSelectMask.reset")}
          />
        </div>
      </SettingContainer>
    </>
  );
});

AutomaticMicrophoneMask.displayName = "AutomaticMicrophoneMask";
