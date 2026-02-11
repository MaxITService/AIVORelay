import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { SettingContainer } from "../ui/SettingContainer";
import { useSettings } from "../../hooks/useSettings";

interface FilterSilenceProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const FilterSilence: React.FC<FilterSilenceProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const enabled = getSetting("filter_silence") ?? true;
    const sonioxRealtimeNote = t(
      "settings.debug.filterSilence.sonioxRealtimeNote",
      "Does not apply to Soniox realtime mode.",
    );

    return (
      <SettingContainer
        title={
          <>
            <span>{t("settings.debug.filterSilence.label")}</span>
            <span className="ml-1 text-red-400">{sonioxRealtimeNote}</span>
          </>
        }
        description={t("settings.debug.filterSilence.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      >
        <ToggleSwitch
          checked={enabled}
          onChange={(enabled) => updateSetting("filter_silence", enabled)}
          isUpdating={isUpdating("filter_silence")}
        />
      </SettingContainer>
    );
  },
);
