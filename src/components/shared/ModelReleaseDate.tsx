import React from "react";
import { useTranslation } from "react-i18next";
import {
  formatModelReleaseDate,
  getModelReleaseDate,
} from "../../lib/utils/modelReleaseDate";

interface ModelReleaseDateProps {
  modelId: string;
  className?: string;
}

export const ModelReleaseDate: React.FC<ModelReleaseDateProps> = ({
  modelId,
  className = "",
}) => {
  const { i18n, t } = useTranslation();
  const releaseDate = getModelReleaseDate(modelId);

  if (!releaseDate) return null;

  return (
    <span
      className={`tabular-nums ${className}`}
      title={t("modelSelector.releaseDateTooltip", "Model release date")}
    >
      {formatModelReleaseDate(releaseDate, i18n.language)}
    </span>
  );
};
