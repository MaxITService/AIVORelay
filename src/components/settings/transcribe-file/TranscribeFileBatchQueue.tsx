import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import {
  FileAudio,
  Files,
  Loader2,
  Play,
  RotateCcw,
  Square,
  Trash2,
} from "lucide-react";
import { Button } from "@/components/ui/Button";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { useTranscribeBatchStore } from "@/stores/transcribeBatchStore";

interface TranscribeFileBatchQueueProps {
  onAddFiles: (paths: string[]) => void;
  supportedExtensions: string[];
  disabledReason: string | null;
  notice: string | null;
}

export const TranscribeFileBatchQueue: React.FC<TranscribeFileBatchQueueProps> = ({
  onAddFiles,
  supportedExtensions,
  disabledReason,
  notice,
}) => {
  const { t } = useTranslation();
  const {
    items,
    running,
    stopping,
    waitingReason,
    error,
    start,
    stop,
    removeItem,
    clearFinished,
    retryFailed,
  } = useTranscribeBatchStore();
  const [dialogError, setDialogError] = useState<string | null>(null);
  const [choosing, setChoosing] = useState(false);
  const waiting = items.filter((item) => item.status === "queued").length;
  const hasFailed = items.some((item) => item.status === "failed");
  const hasFinished = hasFailed || items.some((item) => item.status === "completed");

  const chooseFiles = async () => {
    setDialogError(null);
    setChoosing(true);
    try {
      const selected = await open({
        multiple: true,
        filters: [
          {
            name: t("transcribeFile.batch.audioFiles"),
            extensions: supportedExtensions,
          },
        ],
      });
      if (selected) onAddFiles(Array.isArray(selected) ? selected : [selected]);
    } catch (error) {
      setDialogError(String(error));
    } finally {
      setChoosing(false);
    }
  };

  return (
    <SettingsGroup
      id="transcribe-file-batch"
      title={t("transcribeFile.batch.title")}
      description={t("transcribeFile.batch.description")}
    >
      <div className="space-y-3 px-4 py-4">
        <p className="text-xs text-[#a0a0a0]">
          {t("transcribeFile.batch.savedToDocuments")}
        </p>
        <p className="text-xs text-[#a0a0a0]">
          {t("transcribeFile.batch.restartHint")}
        </p>
        <div className="flex flex-wrap items-center gap-2">
          <Button
            variant="secondary"
            disabled={choosing || !!disabledReason}
            title={disabledReason ?? undefined}
            onClick={() => void chooseFiles()}
          >
            <Files className="mr-2 inline h-4 w-4" />
            {t("transcribeFile.batch.addFiles")}
          </Button>
          {running ? (
            <Button
              variant="danger"
              disabled={stopping}
              onClick={() => void stop()}
            >
              {stopping ? (
                <Loader2 className="mr-2 inline h-4 w-4 animate-spin" />
              ) : (
                <Square className="mr-2 inline h-4 w-4" />
              )}
              {t(stopping ? "transcribeFile.batch.stopping" : "transcribeFile.batch.stop")}
            </Button>
          ) : (
            <Button
              disabled={waiting === 0 || !!disabledReason}
              title={disabledReason ?? undefined}
              onClick={() => void start()}
            >
              <Play className="mr-2 inline h-4 w-4" />
              {t("transcribeFile.batch.resume")}
            </Button>
          )}
          <Button
            variant="secondary"
            disabled={!hasFailed || stopping}
            onClick={retryFailed}
          >
            <RotateCcw className="mr-2 inline h-4 w-4" />
            {t("transcribeFile.batch.retryFailed")}
          </Button>
          <Button
            variant="secondary"
            disabled={!hasFinished}
            onClick={clearFinished}
          >
            <Trash2 className="mr-2 inline h-4 w-4" />
            {t("transcribeFile.batch.clearFinished")}
          </Button>
        </div>
        <p role="status" className="text-xs text-[#b8b8b8]">
          {t("transcribeFile.batch.queueCount", { count: waiting })}
          {running && (
            <span>
              {" · "}
              {t(stopping
                ? "transcribeFile.batch.stopping"
                : waitingReason
                  ? `transcribeFile.batch.waiting.${waitingReason}`
                  : "transcribeFile.batch.running")}
            </span>
          )}
          {!running && waiting > 0 && (
            <span> · {t("transcribeFile.batch.paused")}</span>
          )}
        </p>
        {notice && (
          <p role="status" className="text-xs text-[#b8b8b8]">{notice}</p>
        )}
        {(dialogError || error) && (
          <p role="alert" className="break-words text-sm text-red-300">
            {dialogError || error}
          </p>
        )}
        {items.length > 0 && (
          <div className="max-h-96 overflow-auto rounded-lg border border-white/[0.07] bg-black/15">
            {items.map((item) => (
              <div
                key={item.id}
                className="flex items-start gap-3 border-b border-white/[0.05] px-3 py-2.5 last:border-b-0"
              >
                <FileAudio className="mt-0.5 h-4 w-4 shrink-0 text-[#9b5de5]" />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-xs text-[#e2e2e2]" title={item.path}>
                    {item.name}
                  </p>
                  {item.savedFilePath && (
                    <p className="mt-1 break-all text-[11px] text-green-300">
                      {item.savedFilePath}
                    </p>
                  )}
                  {item.error && (
                    <p className="mt-1 break-words text-[11px] text-red-300">
                      {item.error}
                    </p>
                  )}
                </div>
                <span className={`flex items-center gap-1 text-[11px] ${item.status === "failed" ? "text-red-300" : item.status === "completed" ? "text-green-300" : "text-[#b8b8b8]"}`}>
                  {item.status === "processing" && (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  )}
                  {t(`transcribeFile.batch.status.${item.status}`)}
                </span>
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={item.status === "processing"}
                  title={t("transcribeFile.batch.remove")}
                  aria-label={t("transcribeFile.batch.remove")}
                  onClick={() => removeItem(item.id)}
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </Button>
              </div>
            ))}
          </div>
        )}
      </div>
    </SettingsGroup>
  );
};
