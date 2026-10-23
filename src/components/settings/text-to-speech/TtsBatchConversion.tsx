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
  Trash2,
  XCircle,
} from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { SettingContainer } from "@/components/ui/SettingContainer";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { ToggleSwitch } from "@/components/ui/ToggleSwitch";
import { Tooltip } from "@/components/ui/Tooltip";
import {
  ensureTtsBatchListener,
  entryKey,
  queuedRow,
  useTtsBatchQueueStore,
  type ActiveBatchSnapshot,
  type BatchFileResult,
  type BatchFileStatus,
  type BatchScanResult,
  type TtsOutputFormat,
} from "@/stores/ttsBatchQueueStore";

type BatchSourceMode = "files" | "folder";

const ActionTooltip: React.FC<{
  content?: string | null;
  children: React.ReactNode;
}> = ({ content, children }) =>
  content ? <Tooltip content={content}>{children}</Tooltip> : children;

type TtsChunkProgress = {
  completed_chunks?: number;
  completedChunks?: number;
  total_chunks?: number;
  totalChunks?: number;
};

type TtsBatchConversionProps = {
  outputFormat: TtsOutputFormat;
  mp3Bitrate: number;
  opusBitrate: number;
  flushPendingSettingsWrites: () => Promise<void>;
};

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
  opusBitrate,
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
  const [previewRows, setPreviewRows] = useState<BatchFileResult[]>([]);
  const [chunkProgress, setChunkProgress] = useState<TtsChunkProgress | null>(
    null,
  );
  const [clockMs, setClockMs] = useState(() => Date.now());
  const [localError, setLocalError] = useState<string | null>(null);
  const [queueNotice, setQueueNotice] = useState<string | null>(null);

  // The queue lives in a store so it keeps running while another settings
  // section is open and survives an app restart (paused, never auto-started).
  const queue = useTtsBatchQueueStore((state) => state.queue);
  const queueMeta = useTtsBatchQueueStore((state) => state.meta);
  const batchBusy = useTtsBatchQueueStore((state) => state.running);
  const batchStopping = useTtsBatchQueueStore((state) => state.stopping);
  const runScan = useTtsBatchQueueStore((state) => state.runScan);
  const runningEntries = useTtsBatchQueueStore((state) => state.runningEntries);
  const storeRows = useTtsBatchQueueStore((state) => state.rows);
  const progress = useTtsBatchQueueStore((state) => state.progress);
  const summary = useTtsBatchQueueStore((state) => state.summary);
  const currentFile = useTtsBatchQueueStore((state) => state.currentFile);
  const storeError = useTtsBatchQueueStore((state) => state.error);
  const error = localError ?? storeError;

  const hasStoredQueue = queue.length > 0;
  // While a queue exists, the source buttons append to it instead.
  const addMode = batchBusy || hasStoredQueue;
  const rows = storeRows.length > 0 ? storeRows : previewRows;
  const activeScan = runScan ?? scanResult;
  const waitingFiles = useMemo(() => {
    const running = new Set(runningEntries);
    return queue.filter((file) => !running.has(entryKey(file)));
  }, [queue, runningEntries]);

  const clearPreview = () => {
    setScanResult(null);
    setPreviewRows([]);
    setChunkProgress(null);
    setLocalError(null);
    useTtsBatchQueueStore.getState().resetRun();
  };

  // A format change invalidates every planned output extension. A remount
  // must not clear the results of a batch that finished meanwhile.
  const previewFormat = useRef(outputFormat);
  useEffect(() => {
    if (previewFormat.current === outputFormat) return;
    previewFormat.current = outputFormat;
    if (!useTtsBatchQueueStore.getState().running) clearPreview();
  }, [outputFormat]);

  useEffect(() => {
    let disposed = false;
    let unlistenChunks: (() => void) | undefined;
    const listenForChunks = listen<TtsChunkProgress>("tts://progress", (event) => {
      const state = useTtsBatchQueueStore.getState();
      if (disposed || !state.running || state.currentFile?.status !== "processing") {
        return;
      }
      setChunkProgress(event.payload);
    });
    void listenForChunks.then((disposeChunks) => {
      if (disposed) {
        disposeChunks();
        return;
      }
      unlistenChunks = disposeChunks;
    }).catch((error) => { if (!disposed) setLocalError(String(error)); });
    void ensureTtsBatchListener().then(async () => {
      if (disposed) return;
      const snapshot = await invoke<ActiveBatchSnapshot | null>(
        "get_active_tts_batch_progress",
      );
      if (disposed || !snapshot || snapshot.progress.done) return;
      const store = useTtsBatchQueueStore.getState();
      if (store.running) return;
      // A batch survived a webview reload: show it and keep following it.
      store.attachActiveBatch(snapshot, { mp3Bitrate, opusBitrate });
      setOutputDirectory(snapshot.scan.outputDirectory);
      setRecursive(snapshot.scan.recursive);
      if (snapshot.scan.inputDirectory) {
        setSourceMode("folder");
        setInputDirectory(snapshot.scan.inputDirectory);
      } else {
        setSourceMode("files");
        setInputPaths(snapshot.scan.files.map((file) => file.inputPath));
      }
      setClockMs(Date.now());
    }).catch((error) => { if (!disposed) setLocalError(String(error)); });
    return () => {
      disposed = true;
      unlistenChunks?.();
    };
  }, []);

  useEffect(() => {
    // Chunk progress belongs to one file; a new file starts from zero.
    setChunkProgress(null);
  }, [currentFile?.index, currentFile?.inputPath]);

  useEffect(() => {
    if (!batchBusy) return;
    const interval = window.setInterval(() => setClockMs(Date.now()), 1_000);
    return () => window.clearInterval(interval);
  }, [batchBusy]);

  /** Scans files or a folder and appends them to the existing queue. */
  const addToQueue = async (source: {
    inputPaths?: string[];
    inputDirectory?: string;
  }) => {
    const meta = useTtsBatchQueueStore.getState().meta;
    if (!meta) return;
    setScanning(true);
    setLocalError(null);
    setQueueNotice(null);
    try {
      await flushPendingSettingsWrites();
      const result = await invoke<BatchScanResult>("scan_tts_batch_files", {
        request: {
          inputDirectory: source.inputDirectory ?? null,
          inputPaths: source.inputPaths ?? [],
          outputDirectory: meta.outputDirectory,
          recursive: Boolean(source.inputDirectory) && recursive,
          outputFormat: meta.outputFormat,
          reservedOutputs: useTtsBatchQueueStore.getState().reservedOutputs(),
        },
      });
      const added = useTtsBatchQueueStore.getState().enqueue(result);
      setQueueNotice(
        added > 0
          ? t("textToSpeech.batch.queueAdded", { count: added })
          : t("textToSpeech.batch.queueNothingNew"),
      );
    } catch (scanError) {
      setLocalError(String(scanError));
    } finally {
      setScanning(false);
    }
  };

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
    if (addMode) {
      await addToQueue({ inputPaths: paths });
      return;
    }
    setSourceMode("files");
    setInputPaths(paths);
    setInputDirectory("");
    clearPreview();
  };

  const chooseInputDirectory = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected !== "string") return;
    if (addMode) {
      await addToQueue({ inputDirectory: selected });
      return;
    }
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
    !addMode &&
    !scanning &&
    outputDirectory.length > 0 &&
    (sourceMode === "folder"
      ? inputDirectory.length > 0
      : inputPaths.length > 0);

  const scanFiles = async () => {
    if (!canScan) return;
    setScanning(true);
    setLocalError(null);
    useTtsBatchQueueStore.getState().resetRun();
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
      setPreviewRows(result.files.map(queuedRow));
    } catch (scanError) {
      setScanResult(null);
      setPreviewRows([]);
      setLocalError(String(scanError));
    } finally {
      setScanning(false);
    }
  };

  const startBatch = async () => {
    if (!scanResult || scanResult.eligibleCount === 0 || addMode) return;
    setLocalError(null);
    setChunkProgress(null);
    setClockMs(Date.now());
    try {
      await flushPendingSettingsWrites();
    } catch (flushError) {
      setLocalError(String(flushError));
      return;
    }
    // Resolves when the whole queue, including files added meanwhile, is done.
    await useTtsBatchQueueStore
      .getState()
      .beginFromScan(scanResult, { mp3Bitrate, opusBitrate });
  };

  const resumeQueue = async () => {
    setLocalError(null);
    setClockMs(Date.now());
    try {
      await flushPendingSettingsWrites();
      await useTtsBatchQueueStore.getState().resume();
    } catch (error) {
      setLocalError(String(error));
    }
  };

  const cancelBatch = () => {
    void useTtsBatchQueueStore
      .getState()
      .stop(t("textToSpeech.batch.idUnavailable"));
  };

  const finished = progress?.finished ?? 0;
  const total = progress?.total ?? activeScan?.files.length ?? 0;
  const completedChunks =
    chunkProgress?.completed_chunks ?? chunkProgress?.completedChunks ?? 0;
  const totalChunks =
    chunkProgress?.total_chunks ?? chunkProgress?.totalChunks ?? 0;
  const currentFileFraction =
    currentFile && totalChunks > 0
      ? Math.min(1, completedChunks / totalChunks)
      : 0;
  const plannedWeights = activeScan?.files.map((file) =>
    Math.max(1, file.sourceCharacters),
  );
  const totalWeight = plannedWeights?.reduce((sum, weight) => sum + weight, 0);
  const displayedWeight = plannedWeights
    ? rows.reduce((sum, row) => {
        const weight = plannedWeights[row.index] ?? 1;
        if (row.status === "processing") {
          return sum + weight * currentFileFraction;
        }
        return row.status === "queued" ? sum : sum + weight;
      }, 0)
    : null;
  const completedUnits =
    displayedWeight ?? Math.min(total, finished + currentFileFraction);
  const progressUnits = totalWeight ?? total;
  const progressPercent =
    progressUnits > 0
      ? Math.round((completedUnits / progressUnits) * 100)
      : 0;
  const estimatedProcessedWeight = plannedWeights
    ? rows.reduce((sum, row) => {
        const weight = plannedWeights[row.index] ?? 1;
        if (row.status === "completed") return sum + weight;
        if (row.status === "processing") {
          return sum + weight * currentFileFraction;
        }
        return sum;
      }, 0)
    : 0;
  const estimatedRemainingWeight = plannedWeights
    ? rows.reduce((sum, row) => {
        const weight = plannedWeights[row.index] ?? 1;
        if (row.status === "queued") return sum + weight;
        if (row.status === "processing") {
          return sum + weight * (1 - currentFileFraction);
        }
        return sum;
      }, 0)
    : 0;
  const elapsedMs = progress?.workStartedAtMs
    ? Math.max(0, clockMs - progress.workStartedAtMs)
    : 0;
  const etaMs =
    batchBusy &&
    elapsedMs > 0 &&
    estimatedProcessedWeight > 0 &&
    estimatedRemainingWeight > 0
      ? (elapsedMs / estimatedProcessedWeight) * estimatedRemainingWeight
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
  const selectedSource = addMode
    ? t("textToSpeech.batch.queueSummary", { count: queue.length })
    : sourceMode === "folder"
      ? inputDirectory
      : t("textToSpeech.batch.selectedFiles", { count: inputPaths.length });
  const displayedOutputDirectory =
    addMode && queueMeta ? queueMeta.outputDirectory : outputDirectory;
  const shownFormat = runScan?.outputFormat ?? outputFormat;
  const shownMp3Bitrate = runScan ? (queueMeta?.mp3Bitrate ?? mp3Bitrate) : mp3Bitrate;
  const shownOpusBitrate = runScan
    ? (queueMeta?.opusBitrate ?? opusBitrate)
    : opusBitrate;
  const scanTooltip = scanning
    ? t("textToSpeech.batch.disabledWhileScanning")
    : null;
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
      id="tts-batch-conversion"
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
          <ActionTooltip content={scanTooltip}>
            <Button
              variant={
                !addMode && sourceMode === "folder" ? "primary" : "secondary"
              }
              disabled={scanning}
              onClick={() => void chooseInputDirectory()}
            >
              <FolderOpen className="mr-2 inline h-4 w-4" />
              {addMode
                ? t("textToSpeech.batch.addFolder")
                : t("textToSpeech.batch.chooseFolder")}
            </Button>
          </ActionTooltip>
          <ActionTooltip content={scanTooltip}>
            <Button
              variant={
                !addMode && sourceMode === "files" ? "primary" : "secondary"
              }
              disabled={scanning}
              onClick={() => void chooseFiles()}
            >
              <Files className="mr-2 inline h-4 w-4" />
              {addMode
                ? t("textToSpeech.batch.addFiles")
                : t("textToSpeech.batch.chooseFiles")}
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
        disabled={scanning || (!addMode && sourceMode !== "folder")}
        onChange={(next) => {
          setRecursive(next);
          if (!addMode) clearPreview();
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
          format: (addMode && queueMeta ? queueMeta.outputFormat : outputFormat).toUpperCase(),
        })}
        descriptionMode="inline"
      >
        <div className="flex gap-2">
          <Input
            className="min-w-0 flex-1"
            readOnly
            value={displayedOutputDirectory}
            placeholder={t("textToSpeech.batch.noOutput")}
          />
          <ActionTooltip
            content={
              addMode
                ? t("textToSpeech.batch.queueActive")
                : scanning
                  ? t("textToSpeech.batch.disabledWhileScanning")
                  : null
            }
          >
            <Button
              variant="secondary"
              disabled={addMode || scanning}
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
            addMode
              ? t("textToSpeech.batch.queueActive")
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
        {scanResult && !addMode && (
          <span className="text-xs text-[#a0a0a0]">
            {t("textToSpeech.batch.scanCount", {
              eligible: scanResult.eligibleCount,
              total: scanResult.files.length,
            })}
          </span>
        )}
      </div>

      {activeScan && (
        <div className="space-y-3 border-t border-white/[0.05] px-6 py-4">
          <div className="flex items-center justify-between gap-3">
            <h4 className="text-sm font-medium text-[#e8e8e8]">
              {t("textToSpeech.batch.previewTitle")}
            </h4>
            <span className="rounded-full bg-white/[0.06] px-2.5 py-1 text-[11px] text-[#b8b8b8]">
              {shownFormat.toUpperCase()}
              {shownFormat === "mp3"
                ? ` · ${shownMp3Bitrate} kb/s`
                : shownFormat === "opus"
                  ? ` · ${shownOpusBitrate} kb/s`
                  : ""}
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
          {activeScan.warnings.length > 0 && (
            <div className="space-y-1 rounded-lg border border-amber-400/30 bg-amber-400/10 p-3 text-xs text-amber-200">
              {activeScan.warnings.map((warning, index) => (
                <p key={`${index}-${warning}`}>{warning}</p>
              ))}
            </div>
          )}
        </div>
      )}

      {waitingFiles.length > 0 && (
        <div className="space-y-3 border-t border-white/[0.05] px-6 py-4">
          <div className="flex items-center justify-between gap-3">
            <h4 className="text-sm font-medium text-[#e8e8e8]">
              {t("textToSpeech.batch.queueTitle", {
                count: waitingFiles.length,
              })}
            </h4>
          </div>
          <p className="text-xs text-[#8f8f8f]">
            {batchBusy
              ? t("textToSpeech.batch.queueRunningHint")
              : t("textToSpeech.batch.queueIdleHint")}
          </p>
          <div className="max-h-60 overflow-auto rounded-lg border border-white/[0.07] bg-black/15">
            {waitingFiles.map((file) => (
              <div
                key={file.inputPath}
                className="grid grid-cols-[minmax(0,1fr)_auto] gap-3 border-b border-white/[0.05] px-3 py-2.5 last:border-b-0"
              >
                <div className="min-w-0">
                  <div className="flex items-center gap-2 text-xs text-[#e2e2e2]">
                    <FileText className="h-3.5 w-3.5 shrink-0 text-[#9b5de5]" />
                    <span className="truncate" title={file.inputPath}>
                      {file.relativePath}
                    </span>
                  </div>
                  <div
                    className="mt-1 truncate pl-5 text-[11px] text-[#727272]"
                    title={file.outputPath}
                  >
                    → {file.outputPath}
                  </div>
                </div>
                <div className="flex items-center gap-1.5 self-start text-[11px] text-[#b6b6b6]">
                  <CircleDashed className="h-3.5 w-3.5" />
                  {t("textToSpeech.batch.status.queued")}
                </div>
              </div>
            ))}
          </div>
          <div className="flex flex-wrap items-center gap-2">
            {!batchBusy && (
              <Button onClick={resumeQueue}>
                <Play className="mr-2 inline h-4 w-4" />
                {t("textToSpeech.batch.queueResume")}
              </Button>
            )}
            <Button
              variant="secondary"
              onClick={() => useTtsBatchQueueStore.getState().clearWaiting()}
            >
              <Trash2 className="mr-2 inline h-4 w-4" />
              {t("textToSpeech.batch.queueClear")}
            </Button>
          </div>
        </div>
      )}
      {queueNotice && (
        <p
          role="status"
          className="border-t border-white/[0.05] px-6 py-3 text-xs text-[#b8b8b8]"
        >
          {queueNotice}
        </p>
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
            addMode
              ? t("textToSpeech.batch.queueActive")
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
              addMode ||
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
              onClick={cancelBatch}
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
