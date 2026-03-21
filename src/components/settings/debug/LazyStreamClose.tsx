import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { useSettings } from "../../../hooks/useSettings";

interface LazyStreamCloseProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const LazyStreamClose: React.FC<LazyStreamCloseProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const enabled = Boolean((getSetting("lazy_stream_close" as any) as any) ?? false);

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(nextEnabled) =>
          updateSetting("lazy_stream_close" as any, nextEnabled as any)
        }
        isUpdating={isUpdating("lazy_stream_close" as any)}
        label={t("settings.advanced.lazyStreamClose.label")}
        description={t("settings.advanced.lazyStreamClose.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
