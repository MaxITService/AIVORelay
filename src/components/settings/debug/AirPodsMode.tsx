import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { useSettings } from "../../../hooks/useSettings";

export const AirPodsMode: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  return (
    <ToggleSwitch
      id="settings-airpods-mode"
      checked={Boolean(getSetting("airpods_mode" as any) ?? false)}
      onChange={(enabled) => updateSetting("airpods_mode" as any, enabled as any)}
      isUpdating={isUpdating("airpods_mode" as any)}
      label={t("settings.advanced.airpodsMode.label")}
      description={t("settings.advanced.airpodsMode.description")}
      descriptionMode="inline"
      grouped={true}
    />
  );
};
