import React from "react";
import { useTranslation } from "react-i18next";
import { ProgressBar, ProgressData } from "../shared";
import { LANGUAGE_DETECTOR_ID } from "../settings/languageDetectorDownload";
import { formatDownloadTime, remainingDownloadSeconds, type DownloadStats } from "./downloadStats";

interface DownloadProgress {
  model_id: string;
  downloaded: number;
  total: number;
  percentage: number;
}

interface DownloadProgressDisplayProps {
  downloadProgress: Map<string, DownloadProgress>;
  downloadStats: Map<string, DownloadStats>;
  modelNames?: Map<string, string>;
  className?: string;
}

const DownloadProgressDisplay: React.FC<DownloadProgressDisplayProps> = ({
  downloadProgress,
  downloadStats,
  modelNames,
  className = "",
}) => {
  const { t } = useTranslation();
  if (downloadProgress.size === 0) {
    return null;
  }

  const progressData: ProgressData[] = Array.from(
    downloadProgress.values(),
  ).map((progress) => {
    const stats = downloadStats.get(progress.model_id);
    const remaining = remainingDownloadSeconds(progress.total, progress.downloaded, stats);
    return {
      id: progress.model_id,
      label: progress.model_id === LANGUAGE_DETECTOR_ID
        ? t("settings.spokenLanguage.detectorModel") : modelNames?.get(progress.model_id) ?? progress.model_id,
      percentage: progress.percentage,
      speed: stats?.speed,
      remainingTime: remaining === undefined ? undefined : formatDownloadTime(remaining),
    };
  });

  return (
    <ProgressBar
      progress={progressData}
      className={className}
      showSpeed={downloadProgress.size === 1}
      size="medium"
    />
  );
};

export default DownloadProgressDisplay;
