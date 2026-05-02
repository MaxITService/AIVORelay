import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface ShowTrayShortcutGuideProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const ShowTrayShortcutGuide: React.FC<ShowTrayShortcutGuideProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const showTrayShortcutGuide =
      (getSetting("show_tray_shortcut_guide" as any) as boolean | undefined) ??
      true;

    return (
      <ToggleSwitch
        checked={showTrayShortcutGuide}
        onChange={(enabled) =>
          updateSetting("show_tray_shortcut_guide" as any, enabled)
        }
        isUpdating={isUpdating("show_tray_shortcut_guide")}
        label={t("settings.userInterface.showTrayShortcutGuide.label")}
        description={t(
          "settings.userInterface.showTrayShortcutGuide.description",
        )}
        descriptionMode={descriptionMode}
        grouped={grouped}
        tooltipPosition="bottom"
      />
    );
  });
