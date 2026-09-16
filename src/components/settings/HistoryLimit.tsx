import React from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { CommittedNumberInput } from "./text-to-speech/CommittedNumberInput";
import { SettingContainer } from "../ui/SettingContainer";

interface HistoryLimitProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

const HISTORY_LIMIT_MIN = 0;
const HISTORY_LIMIT_MAX = 1000;

export const HistoryLimit: React.FC<HistoryLimitProps> = ({
  descriptionMode = "inline",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  const historyLimitRaw = Number(getSetting("history_limit") ?? 5);
  const historyLimit = Number.isFinite(historyLimitRaw)
    ? Math.min(
        HISTORY_LIMIT_MAX,
        Math.max(HISTORY_LIMIT_MIN, Math.round(historyLimitRaw)),
      )
    : 5;

  return (
    <SettingContainer
      title={t("settings.history.historyLimit.title")}
      description={t("settings.history.historyLimit.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
      layout="horizontal"
    >
      <div className="flex items-center space-x-2">
        <CommittedNumberInput
          min={HISTORY_LIMIT_MIN}
          max={HISTORY_LIMIT_MAX}
          value={historyLimit}
          onCommit={(value) => updateSetting("history_limit", value)}
          disabled={isUpdating("history_limit")}
          aria-label={t("settings.history.historyLimit.title")}
          className="w-20"
        />
        <span className="text-sm text-text">
          {t("settings.history.historyLimit.entries")}
        </span>
      </div>
    </SettingContainer>
  );
};
