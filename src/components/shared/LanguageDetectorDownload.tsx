import React from "react";
import { useTranslation } from "react-i18next";
import { useLanguageDetectorDownload } from "@/hooks/useLanguageDetectorDownload";
import { Button } from "../ui/Button";

export const LanguageDetectorDownload: React.FC<{ disabled?: boolean }> = ({ disabled = false }) => {
  const { t } = useTranslation();
  const { status, starting, error, download, cancel } = useLanguageDetectorDownload();
  const progress = status?.progress;
  return (
    <div className="space-y-2 text-xs">
      <div className="font-medium">{t("settings.spokenLanguage.detectorModel")}</div>
      <p className="text-text/70">{progress || starting
        ? t("settings.spokenLanguage.downloading")
        : status?.downloaded
          ? t("settings.spokenLanguage.downloaded")
          : t("settings.spokenLanguage.missing")}</p>
      <p className="text-text/60">{t("settings.spokenLanguage.downloadGate")}</p>
      {progress && <div className="flex items-center gap-2">
        <progress aria-label={t("settings.spokenLanguage.detectorModel")} value={progress.percentage} max={100} className="h-1.5 min-w-0 flex-1 accent-amber-400" />
        <span className="tabular-nums">{Math.round(progress.percentage)}%</span>
      </div>}
      {progress || starting
        ? <Button variant="secondary" size="sm" disabled={!progress} onClick={() => void cancel()}>{t("common.cancel", "Cancel")}</Button>
        : status && !status.downloaded && <Button variant="secondary" size="sm" disabled={disabled} onClick={() => void download()}>{t("settings.spokenLanguage.download", "Download detector")}</Button>}
      {error && <p role="alert" className="break-words text-red-400">{error}</p>}
    </div>
  );
};
