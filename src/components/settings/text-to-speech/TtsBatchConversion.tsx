import React, { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  AlertCircle,
  CheckCircle2,
  CircleDashed,
  FileText,
  Files,
  FolderOpen,
  Loader2,
  Play,
  ScanSearch,
  SkipForward,
  Square,
  XCircle,
} from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { SettingContainer } from "@/components/ui/SettingContainer";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { ToggleSwitch } from "@/components/ui/ToggleSwitch";
import { Tooltip } from "@/components/ui/Tooltip";

type TtsOutputFormat = "mp3" | "wav";
type BatchSourceMode = "files" | "folder";

const ActionTooltip: React.FC<{
  content?: string | null;
  children: React.ReactNode;
}> = ({ content, children }) =>
  content ? <Tooltip content={content}>{children}</Tooltip> : children;

const waitForUiTick = (delayMs: number) =>
  new Promise<void>((resolve) => window.setTimeout(resolve, delayMs));
type BatchFileStatus =
  | "queued"
  | "processing"
  | "completed"
  | "skipped"
  | "failed";

type BatchFilePlan = {
  inputPath: string;
  relativePath: string;
  outputPath: string;
  scanError?: string | null;
};

type BatchScanResult = {
  inputDirectory?: string | null;
  outputDirectory: string;
  recursive: boolean;
  outputFormat: TtsOutputFormat;
  files: BatchFilePlan[];
  eligibleCount: number;
  warnings: string[];
};

type BatchFileResult = {
  index: number;
  inputPath: string;
  relativePath: string;
  outputPath: string;
  status: BatchFileStatus;
  error?: string | null;
  warning?: string | null;
  operationId?: string | null;
  resumedChunks: number;
};

type BatchSummary = {
  clientId: string;
  batchId: string;
  total: number;
  finished: number;
  completed: number;
  skipped: number;
  failed: number;
  cancelled: boolean;
  files: BatchFileResult[];
};

type BatchProgress = Omit<BatchSummary, "files"> & {
  done: boolean;
  startedAtMs: number;
  message?: string | null;
  file?: BatchFileResult | null;
};

type TtsChunkProgress = {
  completed_chunks?: number;
  completedChunks?: number;
  total_chunks?: number;
  totalChunks?: number;
};

type TtsBatchConversionProps = {
  outputFormat: TtsOutputFormat;
  mp3Bitrate: number;
  flushPendingSettingsWrites: () => Promise<void>;
};

const newClientId = () => {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `batch-${Date.now()}-${Math.random().toString(36).slice(2)}`;
};

const queuedRow = (file: BatchFilePlan, index: number): BatchFileResult => ({
  index,
  inputPath: file.inputPath,
  relativePath: file.relativePath,
  outputPath: file.outputPath,
  status: "queued",
  error: file.scanError ?? null,
  warning: null,
  operationId: null,
  resumedChunks: 0,
});

const statusAppearance: Record<
  BatchFileStatus,
  { icon: React.ComponentType<{ className?: string }>; className: string }
> = {
  queued: { icon: CircleDashed, className: "text-[#b6b6b6]" },
  processing: { icon: Loader2, className: "text-[#d7b9ff]" },
  completed: { icon: CheckCircle2, className: "text-green-300" },
  skipped: { icon: SkipForward, className: "text-amber-300" },
  failed: { icon: XCircle, className: "text-red-300" },
};

export const TtsBatchConversion: React.FC<TtsBatchConversionProps> = ({
  outputFormat,
  mp3Bitrate,
  flushPendingSettingsWrites,
}) => {
  const { t } = useTranslation();
  const [sourceMode, setSourceMode] = useState<BatchSourceMode>("folder");
  const [inputDirectory, setInputDirectory] = useState("");
  const [inputPaths, setInputPaths] = useState<string[]>([]);
  const [outputDirectory, setOutputDirectory] = useState("");
  const [recursive, setRecursive] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [scanResult, setScanResult] = useState<BatchScanResult | null>(null);
  const [rows, setRows] = useState<BatchFileResult[]>([]);
  const [batchBusy, setBatchBusy] = useState(false);
  const [batchStopping, setBatchStopping] = useState(false);
  const [progress, setProgress] = useState<BatchProgress | null>(null);
  const [summary, setSummary] = useState<BatchSummary | null>(null);
  const [currentFile, setCurrentFile] = useState<BatchFileResult | null>(null);
  const [chunkProgress, setChunkProgress] = useState<TtsChunkProgress | null>(null);
  const [clockMs, setClockMs] = useState(() => Date.now());
  const [error, setError] = useState<string | null>(null);
  const activeClientIdRef = useRef<string | null>(null);
  const batchBusyRef = useRef(false);
  const batchIdRef = useRef<string | null>(null);
  const batchStartedAtRef = useRef<number | null>(null);
  const processingFileRef = useRef<BatchFileResult | null>(null);

  const clearPreview = () => {
    setScanResult(null);
    setRows([]);
    setProgress(null);
    setSummary(null);
    setCurrentFile(null);
    setChunkProgress(null);
    batchIdRef.current = null;
    batchStartedAtRef.current = null;
    processingFileRef.current = null;
    setError(null);
    activeClientIdRef.current = null;
  };

  useEffect(() => {
    if (!batchBusyRef.current) clearPreview();
    // A format change invalidates every planned output extension.
  }, [outputFormat]);

  useEffect(() => {
    let disposed = false;
    let unlistenBatch: (() => void) | undefined;
    let unlistenChunks: (() => void) | undefined;
    const listenForBatch = listen<BatchProgress>("tts://batch-progress", (event) => {
      if (disposed || !batchBusyRef.current) return;
      const next = event.payload;
      if (next.clientId !== activeClientIdRef.current) return;
      batchIdRef.current = next.batchId;
      setProgress(next);
      const file = next.file;
      if (file) {
        const processingFile = file.status === "processing" ? file : null;
        processingFileRef.current = processingFile;
        setCurrentFile(processingFile);
        setChunkProgress(null);
        setRows((current) =>
          current.map((row) => (row.index === file.index ? file : row)),
        );
      }
      if (next.done) {
        batchBusyRef.current = false;
        processingFileRef.current = null;
        activeClientIdRef.current = null;
        setBatchBusy(false);
        setBatchStopping(false);
        setCurrentFile(null);
        setChunkProgress(null);
      }
    });
    const listenForChunks = listen<TtsChunkProgress>("tts://progress", (event) => {
      if (
        disposed ||
        !batchBusyRef.current ||
        !processingFileRef.current
      ) {
        return;
      }
      setChunkProgress(event.payload);
    });
    void Promise.all([listenForBatch, listenForChunks]).then(
      async ([disposeBatch, disposeChunks]) => {
        if (disposed) {
          disposeBatch();
          disposeChunks();
          return;
        }
        unlistenBatch = disposeBatch;
        unlistenChunks = disposeChunks;
        const active = await invoke<BatchProgress | null>(
          "get_active_tts_batch_progress",
        );
        if (disposed || !active || active.done) return;
        activeClientIdRef.current = active.clientId;
        batchIdRef.current = active.batchId;
        batchBusyRef.current = true;
        const latest = await invoke<BatchProgress | null>(
          "get_active_tts_batch_progress",
        );
        if (
          disposed ||
          !latest ||
          latest.done ||
          latest.clientId !== active.clientId
        ) {
          batchBusyRef.current = false;
          activeClientIdRef.current = null;
          return;
        }
        batchStartedAtRef.current = latest.startedAtMs;
        processingFileRef.current =
          latest.file?.status === "processing" ? latest.file : null;
        setBatchBusy(true);
        setProgress(latest);
        setCurrentFile(processingFileRef.current);
        setClockMs(Date.now());
      },
    );
    return () => {
      disposed = true;
      unlistenBatch?.();
      unlistenChunks?.();
    };
  }, []);

  useEffect(() => {
    if (!batchBusy) return;
    const interval = window.setInterval(() => setClockMs(Date.now()), 1_000);
    return () => window.clearInterval(interval);
  }, [batchBusy]);

  const chooseFiles = async () => {
    const selected = await open({
      directory: false,
      multiple: true,
      filters: [
        {
          name: t("textToSpeech.conversion.textMarkdownFiles"),
          extensions: ["txt", "md"],
        },
      ],
    });
    const paths = Array.isArray(selected)
      ? selected
      : typeof selected === "string"
        ? [selected]
        : [];
    if (paths.length === 0) return;
    setSourceMode("files");
    setInputPaths(paths);
    setInputDirectory("");
    clearPreview();
  };

  const chooseInputDirectory = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected !== "string") return;
    setSourceMode("folder");
    setInputDirectory(selected);
    setInputPaths([]);
    clearPreview();
  };

  const chooseOutputDirectory = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected !== "string") return;
    setOutputDirectory(selected);
    clearPreview();
  };

  const canScan =
    !batchBusy &&
    !scanning &&
    outputDirectory.length > 0 &&
    (sourceMode === "folder"
      ? inputDirectory.length > 0
      : inputPaths.length > 0);

  const scanFiles = async () => {
    if (!canScan) return;
    setScanning(true);
    setError(null);
    setSummary(null);
    setProgress(null);
    batchIdRef.current = null;
    try {
      await flushPendingSettingsWrites();
      const result = await invoke<BatchScanResult>("scan_tts_batch_files", {
        request: {
          inputDirectory: sourceMode === "folder" ? inputDirectory : null,
          inputPaths: sourceMode === "files" ? inputPaths : [],
          outputDirectory,
          recursive: sourceMode === "folder" && recursive,
          outputFormat,
        },
      });
      setScanResult(result);
      setRows(result.files.map(queuedRow));
    } catch (scanError) {
      setScanResult(null);
      setRows([]);
      setError(String(scanError));
    } finally {
      setScanning(false);
    }
  };

  const startBatch = async () => {
    if (!scanResult || scanResult.eligibleCount === 0 || batchBusy) return;
    const clientId = newClientId();
    activeClientIdRef.current = clientId;
    batchBusyRef.current = true;
    setBatchBusy(true);
    batchIdRef.current = null;
    setError(null);
    setSummary(null);
    setProgress(null);
    setCurrentFile(null);
    setChunkProgress(null);
    batchStartedAtRef.current = Date.now();
    processingFileRef.current = null;
    setClockMs(Date.now());
    setRows(scanResult.files.map(queuedRow));
    try {
      await flushPendingSettingsWrites();
      const result = await invoke<BatchSummary>("convert_tts_batch", {
        request: {
          clientId,
          scan: scanResult,
          mp3Bitrate,
        },
      });
      batchIdRef.current = result.batchId;
      setRows(result.files);
      setSummary(result);
      setProgress((current) => ({
        clientId: result.clientId,
        batchId: result.batchId,
        total: result.total,
        finished: result.finished,
        completed: result.completed,
        skipped: result.skipped,
        failed: result.failed,
        cancelled: result.cancelled,
        done: true,
        startedAtMs: current?.startedAtMs ?? Date.now(),
        message: current?.message ?? null,
        file: null,
      }));
    } catch (batchError) {
      setError(String(batchError));
    } finally {
      batchBusyRef.current = false;
      setBatchBusy(false);
      setBatchStopping(false);
      window.dispatchEvent(new Event("aivorelay:tts-jobs-changed"));
    }
  };

  const cancelBatch = async () => {
    setBatchStopping(true);
    setError(null);
    try {
      let activeBatchId = batchIdRef.current;
      for (
        let attempt = 0;
        !activeBatchId && batchBusyRef.current && attempt < 1_300;
        attempt += 1
      ) {
        await waitForUiTick(100);
        activeBatchId = batchIdRef.current;
      }
      if (!activeBatchId) {
        if (!batchBusyRef.current) {
          setBatchStopping(false);
          return;
        }
        throw new Error(t("textToSpeech.batch.idUnavailable"));
      }
      await invoke("cancel_tts_batch", { batchId: activeBatchId });
    } catch (cancelError) {
      setError(String(cancelError));
      setBatchStopping(false);
    }
  };

  const finished = progress?.finished ?? 0;
  const total = progress?.total ?? scanResult?.files.length ?? 0;
  const completedChunks =
    chunkProgress?.completed_chunks ?? chunkProgress?.completedChunks ?? 0;
  const totalChunks =
    chunkProgress?.total_chunks ?? chunkProgress?.totalChunks ?? 0;
  const currentFileFraction =
    currentFile && totalChunks > 0
      ? Math.min(1, completedChunks / totalChunks)
      : 0;
  const completedUnits = Math.min(total, finished + currentFileFraction);
  const progressPercent =
    total > 0 ? Math.round((completedUnits / total) * 100) : 0;
  const elapsedMs = batchStartedAtRef.current
    ? Math.max(0, clockMs - batchStartedAtRef.current)
    : 0;
  const etaMs =
    batchBusy && completedUnits > 0 && completedUnits < total
      ? (elapsedMs / completedUnits) * (total - completedUnits)
      : null;
  const formatDuration = (durationMs: number) => {
    const seconds = Math.max(1, Math.ceil(durationMs / 1_000));
    if (seconds < 60) {
      return t("textToSpeech.batch.durationSeconds", { count: seconds });
    }
    const minutes = Math.ceil(seconds / 60);
    if (minutes < 60) {
      return t("textToSpeech.batch.durationMinutes", { count: minutes });
    }
    return t("textToSpeech.batch.durationHoursMinutes", {
      hours: Math.floor(minutes / 60),
      minutes: minutes % 60,
    });
  };
  const selectedSource =
    sourceMode === "folder"
      ? inputDirectory
      : t("textToSpeech.batch.selectedFiles", { count: inputPaths.length });
  const normalProgressMessage = useMemo(() => {
    if (!progress?.message) return null;
    if (progress.message.includes("Waiting for")) {
      return t("textToSpeech.batch.waitingForTts");
    }
    if (progress.cancelled) return t("textToSpeech.batch.cancelled");
    if (progress.done) return t("textToSpeech.batch.finished");
    return t("textToSpeech.batch.running");
  }, [progress, t]);

  return (
    <SettingsGroup
      title={t("textToSpeech.batch.title")}
      description={t("textToSpeech.batch.description")}
    >
      <SettingContainer
        grouped
        layout="stacked"
        title={t("textToSpeech.batch.sourceTitle")}
        description={t("textToSpeech.batch.sourceDescription")}
        descriptionMode="inline"
      >
        <div className="mb-3 flex flex-wrap gap-2">
          <ActionTooltip
            content={
              batchBusy
                ? t("textToSpeech.batch.disabledWhileRunning")
                : scanning
                  ? t("textToSpeech.batch.disabledWhileScanning")
                  : null
            }
          >
            <Button
              variant={sourceMode === "folder" ? "primary" : "secondary"}
              disabled={batchBusy || scanning}
              onClick={() => void chooseInputDirectory()}
            >
              <FolderOpen className="mr-2 inline h-4 w-4" />
              {t("textToSpeech.batch.chooseFolder")}
            </Button>
          </ActionTooltip>
          <ActionTooltip
            content={
              batchBusy
                ? t("textToSpeech.batch.disabledWhileRunning")
                : scanning
                  ? t("textToSpeech.batch.disabledWhileScanning")
                  : null
            }
          >
            <Button
              variant={sourceMode === "files" ? "primary" : "secondary"}
              disabled={batchBusy || scanning}
              onClick={() => void chooseFiles()}
            >
              <Files className="mr-2 inline h-4 w-4" />
              {t("textToSpeech.batch.chooseFiles")}
            </Button>
          </ActionTooltip>
        </div>
        <Input
          readOnly
          value={selectedSource}
          placeholder={t("textToSpeech.batch.noSource")}
        />
      </SettingContainer>

      <ToggleSwitch
        grouped
        checked={recursive}
        disabled={sourceMode !== "folder" || batchBusy || scanning}
        onChange={(next) => {
          setRecursive(next);
          clearPreview();
        }}
        label={t("textToSpeech.batch.recursive")}
        description={t("textToSpeech.batch.recursiveDescription")}
        descriptionMode="inline"
      />

      <SettingContainer
        grouped
        layout="stacked"
        title={t("textToSpeech.batch.outputTitle")}
        description={t("textToSpeech.batch.outputDescription", {
          format: outputFormat.toUpperCase(),
        })}
        descriptionMode="inline"
      >
        <div className="flex gap-2">
          <Input
            className="min-w-0 flex-1"
            readOnly
            value={outputDirectory}
            placeholder={t("textToSpeech.batch.noOutput")}
          />
          <ActionTooltip
            content={
              batchBusy
                ? t("textToSpeech.batch.disabledWhileRunning")
                : scanning
                  ? t("textToSpeech.batch.disabledWhileScanning")
                  : null
            }
          >
            <Button
              variant="secondary"
              disabled={batchBusy || scanning}
              onClick={() => void chooseOutputDirectory()}
            >
              <FolderOpen className="mr-2 inline h-4 w-4" />
              {t("textToSpeech.batch.chooseOutput")}
            </Button>
          </ActionTooltip>
        </div>
      </SettingContainer>

      <div className="flex flex-wrap items-center gap-3 px-6 py-4">
        <ActionTooltip
          content={
            batchBusy
              ? t("textToSpeech.batch.disabledWhileRunning")
              : scanning
                ? t("textToSpeech.batch.disabledWhileScanning")
                : !selectedSource
                  ? t("textToSpeech.batch.chooseSourceFirst")
                  : !outputDirectory
                    ? t("textToSpeech.batch.chooseOutputFirst")
                    : null
          }
        >
          <Button
            variant="secondary"
            disabled={!canScan}
            onClick={() => void scanFiles()}
          >
            {scanning ? (
              <Loader2 className="mr-2 inline h-4 w-4 animate-spin" />
            ) : (
              <ScanSearch className="mr-2 inline h-4 w-4" />
            )}
            {scanning
              ? t("textToSpeech.batch.scanning")
              : t("textToSpeech.batch.scan")}
          </Button>
        </ActionTooltip>
        {scanResult && (
          <span className="text-xs text-[#a0a0a0]">
            {t("textToSpeech.batch.scanCount", {
              eligible: scanResult.eligibleCount,
              total: scanResult.files.length,
            })}
          </span>
        )}
      </div>

      {scanResult && (
        <div className="space-y-3 border-t border-white/[0.05] px-6 py-4">
          <div className="flex items-center justify-between gap-3">
            <h4 className="text-sm font-medium text-[#e8e8e8]">
              {t("textToSpeech.batch.previewTitle")}
            </h4>
            <span className="rounded-full bg-white/[0.06] px-2.5 py-1 text-[11px] text-[#b8b8b8]">
              {outputFormat.toUpperCase()}
              {outputFormat === "mp3" ? ` · ${mp3Bitrate} kb/s` : ""}
            </span>
          </div>
          {rows.length === 0 ? (
            <p className="text-sm text-[#8f8f8f]">
              {t("textToSpeech.batch.noFiles")}
            </p>
          ) : (
            <div className="max-h-80 overflow-auto rounded-lg border border-white/[0.07] bg-black/15">
              {rows.map((row) => {
                const appearance = statusAppearance[row.status];
                const StatusIcon = appearance.icon;
                return (
                  <div
                    key={`${row.index}-${row.inputPath}`}
                    className="grid grid-cols-[minmax(0,1fr)_auto] gap-3 border-b border-white/[0.05] px-3 py-2.5 last:border-b-0"
                  >
                    <div className="min-w-0">
                      <div className="flex items-center gap-2 text-xs text-[#e2e2e2]">
                        <FileText className="h-3.5 w-3.5 shrink-0 text-[#9b5de5]" />
                        <span className="truncate" title={row.inputPath}>
                          {row.relativePath}
                        </span>
                      </div>
                      <div
                        className="mt-1 truncate pl-5 text-[11px] text-[#727272]"
                        title={row.outputPath}
                      >
                        → {row.outputPath}
                      </div>
                      {(row.error || row.warning) && (
                        <p
                          className={`mt-1 break-words pl-5 text-[11px] ${
                            row.error ? "text-red-300" : "text-amber-300"
                          }`}
                        >
                          {row.error ?? row.warning}
                        </p>
                      )}
                    </div>
                    <div
                      className={`flex items-center gap-1.5 self-start text-[11px] ${appearance.className}`}
                    >
                      <StatusIcon
                        className={`h-3.5 w-3.5 ${
                          row.status === "processing" ? "animate-spin" : ""
                        }`}
                      />
                      {t(`textToSpeech.batch.status.${row.status}`)}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
          {scanResult.warnings.length > 0 && (
            <div className="space-y-1 rounded-lg border border-amber-400/30 bg-amber-400/10 p-3 text-xs text-amber-200">
              {scanResult.warnings.map((warning, index) => (
                <p key={`${index}-${warning}`}>{warning}</p>
              ))}
            </div>
          )}
        </div>
      )}

      {(batchBusy || progress) && total > 0 && (
        <div className="space-y-2 border-t border-white/[0.05] px-6 py-4">
          <div className="flex items-center justify-between text-xs text-[#b8b8b8]">
            <span>
              {normalProgressMessage ?? t("textToSpeech.batch.ready")} ·{" "}
              {finished} / {total}
            </span>
            <span>{progressPercent}%</span>
          </div>
          {batchBusy && (
            <div className="flex flex-wrap items-center justify-between gap-2 text-[11px] text-[#8f8f8f]">
              <span className="min-w-0 truncate" title={currentFile?.inputPath}>
                {currentFile
                  ? t("textToSpeech.batch.currentFile", {
                      file: currentFile.relativePath,
                    })
                  : t("textToSpeech.batch.waitingForCurrentFile")}
                {currentFile && totalChunks > 0
                  ? ` · ${completedChunks} / ${totalChunks}`
                  : ""}
              </span>
              <span className="shrink-0">
                {etaMs === null
                  ? t("textToSpeech.batch.etaCalculating")
                  : t("textToSpeech.batch.eta", {
                      time: formatDuration(etaMs),
                    })}
              </span>
            </div>
          )}
          <div className="h-2 overflow-hidden rounded-full bg-[#252525]">
            <div
              className="h-full rounded-full bg-[#9b5de5] transition-[width]"
              style={{ width: `${progressPercent}%` }}
            />
          </div>
        </div>
      )}

      <div className="flex flex-wrap items-center gap-3 border-t border-white/[0.05] px-6 py-4">
        <ActionTooltip
          content={
            batchBusy
              ? t("textToSpeech.batch.disabledWhileRunning")
              : scanning
                ? t("textToSpeech.batch.disabledWhileScanning")
                : !scanResult
                  ? t("textToSpeech.batch.scanFirst")
                  : scanResult.eligibleCount === 0
                    ? t("textToSpeech.batch.noEligibleFiles")
                    : null
          }
        >
          <Button
            disabled={
              !scanResult ||
              scanResult.eligibleCount === 0 ||
              batchBusy ||
              scanning
            }
            onClick={() => void startBatch()}
          >
            {batchBusy ? (
              <Loader2 className="mr-2 inline h-4 w-4 animate-spin" />
            ) : (
              <Play className="mr-2 inline h-4 w-4" />
            )}
            {batchBusy
              ? t("textToSpeech.batch.running")
              : t("textToSpeech.batch.start")}
          </Button>
        </ActionTooltip>
        {batchBusy && (
          <ActionTooltip
            content={
              batchStopping
                ? t("textToSpeech.batch.stopAlreadyRequested")
                : t("textToSpeech.batch.stopDescription")
            }
          >
            <Button
              variant="danger"
              disabled={batchStopping}
              onClick={() => void cancelBatch()}
            >
              {batchStopping ? (
                <Loader2 className="mr-2 inline h-3.5 w-3.5 animate-spin" />
              ) : (
                <Square className="mr-2 inline h-3.5 w-3.5" />
              )}
              {batchStopping
                ? t("textToSpeech.batch.stopping")
                : t("textToSpeech.batch.stop")}
            </Button>
          </ActionTooltip>
        )}
      </div>

      {error && (
        <div
          role="alert"
          className="flex items-start gap-2 border-t border-white/[0.05] px-6 py-4 text-sm text-red-300"
        >
          <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" />
          <span className="break-words">{error}</span>
        </div>
      )}

      {summary && (
        <div
          role="status"
          aria-live="polite"
          className="border-t border-white/[0.05] px-6 py-4"
        >
          <div className="flex items-center gap-2 text-sm font-medium text-[#e8e8e8]">
            {summary.cancelled ? (
              <AlertCircle className="h-4 w-4 text-amber-300" />
            ) : (
              <CheckCircle2 className="h-4 w-4 text-green-300" />
            )}
            {summary.cancelled
              ? t("textToSpeech.batch.cancelled")
              : t("textToSpeech.batch.summaryTitle")}
          </div>
          <div className="mt-3 grid grid-cols-3 gap-3 text-center">
            {[
              [
                t("textToSpeech.batch.completed"),
                summary.completed,
                "text-green-300",
              ],
              [
                t("textToSpeech.batch.skipped"),
                summary.skipped,
                "text-amber-300",
              ],
              [t("textToSpeech.batch.failed"), summary.failed, "text-red-300"],
            ].map(([label, value, className]) => (
              <div
                key={String(label)}
                className="rounded-lg border border-white/[0.06] bg-black/15 p-3"
              >
                <div className={`text-lg font-semibold ${className}`}>
                  {value}
                </div>
                <div className="mt-1 text-[11px] text-[#8f8f8f]">{label}</div>
              </div>
            ))}
          </div>
        </div>
      )}
    </SettingsGroup>
  );
};

export default TtsBatchConversion;
