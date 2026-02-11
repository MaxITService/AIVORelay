import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { SettingContainer } from "../ui/SettingContainer";
import { useSettings } from "../../hooks/useSettings";

interface AppendTrailingSpaceProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const AppendTrailingSpace: React.FC<AppendTrailingSpaceProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const enabled = getSetting("append_trailing_space") ?? false;

    return (
      <SettingContainer
        title={t("settings.debug.appendTrailingSpace.label")}
        description={
          <>
            <span>{t("settings.debug.appendTrailingSpace.description")}</span>
            <span className="block mt-1 text-red-400">
              Dangerous: can break forms, commands, and pasted code, and you can wonder where the space came from. Applies to Soniox realtime mode too.
            </span>
          </>
        }
        descriptionMode={descriptionMode}
        grouped={grouped}
      >
        <ToggleSwitch
          checked={enabled}
          onChange={(enabled) => updateSetting("append_trailing_space", enabled)}
          isUpdating={isUpdating("append_trailing_space")}
        />
      </SettingContainer>
    );
  });
