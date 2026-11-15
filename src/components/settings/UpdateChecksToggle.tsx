import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";
import { useUpdateChecksLocked } from "../../hooks/useUpdateChecksLocked";

interface UpdateChecksToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const UpdateChecksToggle: React.FC<UpdateChecksToggleProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const updateChecksLocked = useUpdateChecksLocked();
  const updateChecksEnabled = getSetting("update_checks_enabled") ?? true;

  return (
    <ToggleSwitch
      id="settings-update-checks"
      checked={updateChecksLocked ? false : updateChecksEnabled}
      onChange={(enabled) => updateSetting("update_checks_enabled", enabled)}
      isUpdating={isUpdating("update_checks_enabled")}
      disabled={updateChecksLocked !== false}
      label={t("settings.debug.updateChecks.label")}
      description={
        updateChecksLocked
          ? t("settings.debug.updateChecks.lockedDescription")
          : t("settings.debug.updateChecks.description")
      }
      descriptionMode={descriptionMode}
      grouped={grouped}
    />
  );
};
