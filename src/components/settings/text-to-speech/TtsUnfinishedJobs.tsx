import React, { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import {
  AlertCircle,
  Download,
  Loader2,
  Pause,
  Play,
  Trash2,
} from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/Button";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { Tooltip } from "@/components/ui/Tooltip";

type JobStatus =
  | "planned"
  | "preparing"
  | "running"
  | "retrying"
  | "paused"
  | "interrupted"
  | "failed"
  | "completed";

type TtsFileJob = {
  jobId: string;
  sourcePath: string;
  outputPath: string;
  provider: string;
  outputFormat: "mp3" | "wav";
  status: JobStatus;
  completedChunks: number;
  totalChunks: number;
  progressStage: "ai_cleanup" | "speech";
  partialAvailable: boolean;
  lastError?: string | null;
  createdAtMs: number;
  updatedAtMs: number;
};

type TtsStateEvent = {
  kind?: string | null;
  phase?: string | null;
  operationId?: string | number;
  operation_id?: string | number;
  completedChunks?: number;
  completed_chunks?: number;
  totalChunks?: number;
  total_chunks?: number;
};

type TtsUnfinishedJobsProps = {
  activeOperationId?: string | null;
  activeCompletedChunks?: number;
  activeTotalChunks?: number;
  activeProgressStage?: "ai_cleanup" | "speech";
};

const isInFlight = (job: TtsFileJob) =>
  job.status === "preparing" ||
  job.status === "running" ||
  job.status === "retrying";

const waitForUiTick = (delayMs: number) =>
  new Promise<void>((resolve) => window.setTimeout(resolve, delayMs));

const ActionTooltip: React.FC<{
  content?: string | null;
  children: React.ReactNode;
}> = ({ content, children }) =>
  content ? <Tooltip content={content}>{children}</Tooltip> : children;

const partialDefaultPath = (job: TtsFileJob) => {
  const suffix = `.${job.outputFormat}`;
  return job.outputPath.toLocaleLowerCase().endsWith(suffix)
    ? `${job.outputPath.slice(0, -suffix.length)}.partial${suffix}`
    : `${job.outputPath}.partial${suffix}`;
};

export const TtsUnfinishedJobs: React.FC<TtsUnfinishedJobsProps> = ({
  activeOperationId: parentOperationId = null,
  activeCompletedChunks,
  activeTotalChunks,
  activeProgressStage,
}) => {
  const { t } = useTranslation();
  const [jobs, setJobs] = useState<TtsFileJob[]>([]);
  const [loading, setLoading] = useState(true);
  const [activeJobId, setActiveJobId] = useState<string | null>(null);
  const [pausingJobId, setPausingJobId] = useState<string | null>(null);
  const [discardingJobId, setDiscardingJobId] = useState<string | null>(null);
  const [liveProgress, setLiveProgress] = useState<{
    completed: number;
    total: number;
    stage: "ai_cleanup" | "speech";
  } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const activeJobIdRef = useRef<string | null>(null);
  const operationIdRef = useRef<string | null>(null);
  const pendingResumeJobIdRef = useRef<string | null>(null);

  const updateOperationId = useCallback((nextOperationId: string | null) => {
    operationIdRef.current = nextOperationId;
  }, []);

  const refresh = useCallback(async () => {
    try {
      const result = await invoke<TtsFileJob[]>("list_tts_file_jobs");
      setJobs(result);
      const runningJob = result.find(isInFlight);
      if (runningJob) {
        const attachedToNewJob = activeJobIdRef.current !== runningJob.jobId;
        activeJobIdRef.current = runningJob.jobId;
        setActiveJobId(runningJob.jobId);
        if (attachedToNewJob) {
          updateOperationId(null);
          setLiveProgress({
            completed: runningJob.completedChunks,
            total: runningJob.totalChunks,
            stage: runningJob.progressStage,
          });
        }
        setError(null);
      } else if (!pendingResumeJobIdRef.current) {
        activeJobIdRef.current = null;
        setActiveJobId(null);
        updateOperationId(null);
        setLiveProgress(null);
      }
    } catch (refreshError) {
      setError(String(refreshError));
    } finally {
      setLoading(false);
    }
  }, [updateOperationId]);

  useEffect(() => {
    if (!parentOperationId) return;
    const runningJob = jobs.find(isInFlight);
    if (!runningJob) return;
    activeJobIdRef.current = runningJob.jobId;
    setActiveJobId(runningJob.jobId);
    updateOperationId(parentOperationId);
    setLiveProgress((previous) => {
      const stage = activeProgressStage ?? runningJob.progressStage;
      const stageChanged = previous !== null && previous.stage !== stage;
      return {
        completed: stageChanged
          ? (activeCompletedChunks ?? 0)
          : Math.max(
              previous?.completed ?? 0,
              runningJob.completedChunks,
              activeCompletedChunks ?? 0,
            ),
        total: stageChanged
          ? (activeTotalChunks ?? 0)
          : Math.max(
              previous?.total ?? 0,
              runningJob.totalChunks,
              activeTotalChunks ?? 0,
            ),
        stage,
      };
    });
    setError(null);
  }, [
    activeCompletedChunks,
    activeProgressStage,
    activeTotalChunks,
    jobs,
    parentOperationId,
    updateOperationId,
  ]);

  useEffect(() => {
    void refresh();
    const handleChanged = () => void refresh();
    window.addEventListener("aivorelay:tts-jobs-changed", handleChanged);
    return () =>
      window.removeEventListener("aivorelay:tts-jobs-changed", handleChanged);
  }, [refresh]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<TtsStateEvent>("tts://state", (event) => {
      if (
        disposed ||
        !activeJobIdRef.current ||
        event.payload.kind !== "file_conversion"
      ) {
        return;
      }
      const id = event.payload.operationId ?? event.payload.operation_id;
      if (id !== undefined) updateOperationId(String(id));
      const reportedCompleted =
        event.payload.completedChunks ?? event.payload.completed_chunks ?? 0;
      const reportedTotal =
        event.payload.totalChunks ?? event.payload.total_chunks ?? 0;
      setLiveProgress((previous) => {
        const reportedStage =
          event.payload.phase === "preprocessing"
            ? "ai_cleanup"
            : event.payload.phase === "synthesizing" ||
                event.payload.phase === "retrying" ||
                event.payload.phase === "ready" ||
                event.payload.phase === "completed"
              ? "speech"
              : (previous?.stage ?? "speech");
        const stageChanged =
          previous !== null && previous.stage !== reportedStage;
        return {
          completed: stageChanged
            ? reportedCompleted
            : Math.max(previous?.completed ?? 0, reportedCompleted),
          total: stageChanged
            ? reportedTotal
            : Math.max(previous?.total ?? 0, reportedTotal),
          stage: reportedStage,
        };
      });
      if (
        event.payload.phase === "completed" ||
        event.payload.phase === "cancelled" ||
        event.payload.phase === "error"
      ) {
        window.setTimeout(() => void refresh(), 250);
      }
    }).then((dispose) => {
      if (disposed) dispose();
      else unlisten = dispose;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [refresh, updateOperationId]);

  const resumeJob = async (job: TtsFileJob) => {
    pendingResumeJobIdRef.current = job.jobId;
    activeJobIdRef.current = job.jobId;
    setActiveJobId(job.jobId);
    updateOperationId(null);
    setLiveProgress({
      completed: job.completedChunks,
      total: job.totalChunks,
      stage: job.progressStage,
    });
    setError(null);
    try {
      await invoke("resume_tts_file_job", { jobId: job.jobId });
    } catch (resumeError) {
      setError(String(resumeError));
    } finally {
      pendingResumeJobIdRef.current = null;
      activeJobIdRef.current = null;
      setActiveJobId(null);
      updateOperationId(null);
      setLiveProgress(null);
      await refresh();
    }
  };

  const waitForActiveOperationId = async (jobId: string) => {
    let activeOperationId = operationIdRef.current;
    for (
      let attempt = 0;
      !activeOperationId && attempt < 1_300;
      attempt += 1
    ) {
      if (activeJobIdRef.current !== jobId) return null;
      await waitForUiTick(100);
      activeOperationId = operationIdRef.current;
    }
    if (!activeOperationId && activeJobIdRef.current === jobId) {
      throw new Error(
        t("textToSpeech.unfinishedJobs.activeOperationUnavailable"),
      );
    }
    return activeOperationId;
  };

  const pauseJob = async (job: TtsFileJob) => {
    setError(null);
    setPausingJobId(job.jobId);
    try {
      const activeOperationId = await waitForActiveOperationId(job.jobId);
      if (activeOperationId) {
        await invoke("cancel_tts_operation", {
          operationId: activeOperationId,
        });
      }
    } catch (pauseError) {
      setError(String(pauseError));
    } finally {
      setPausingJobId(null);
    }
  };

  const exportPartial = async (job: TtsFileJob) => {
    const destination = await save({
      defaultPath: partialDefaultPath(job),
      filters: [
        {
          name: job.outputFormat === "mp3" ? "MP3" : "WAV",
          extensions: [job.outputFormat],
        },
      ],
    });
    if (!destination) return;
    setError(null);
    try {
      await invoke("export_tts_file_job_partial", {
        jobId: job.jobId,
        destination,
      });
    } catch (exportError) {
      setError(String(exportError));
    }
  };

  const discardJob = async (job: TtsFileJob) => {
    const active = activeJobIdRef.current === job.jobId || isInFlight(job);
    const confirmationKey = active
      ? "textToSpeech.unfinishedJobs.discardActiveConfirm"
      : "textToSpeech.unfinishedJobs.discardConfirm";
    if (!window.confirm(t(confirmationKey))) return;
    setError(null);
    setDiscardingJobId(job.jobId);
    try {
      if (active) {
        const activeOperationId = await waitForActiveOperationId(job.jobId);
        if (activeOperationId) {
          await invoke("cancel_tts_operation", {
            operationId: activeOperationId,
          });
        }
      }

      let discardError: unknown = null;
      for (let attempt = 0; attempt < 100; attempt += 1) {
        try {
          await invoke("discard_tts_file_job", { jobId: job.jobId });
          discardError = null;
          break;
        } catch (retryError) {
          discardError = retryError;
          const message = String(retryError);
          if (
            !message.includes("Pause this TTS conversion") &&
            !message.includes("still active and cannot be discarded")
          ) {
            throw retryError;
          }
          await waitForUiTick(100);
        }
      }
      if (discardError) throw discardError;
      await refresh();
    } catch (discardError) {
      setError(String(discardError));
    } finally {
      setDiscardingJobId(null);
    }
  };

  if (!loading && jobs.length === 0 && !error) return null;

  return (
    <SettingsGroup
      title={t("textToSpeech.unfinishedJobs.title")}
      description={t("textToSpeech.unfinishedJobs.description")}
    >
      {loading && (
        <div className="flex items-center gap-2 px-6 py-4 text-sm text-[#b8b8b8]">
          <Loader2 className="h-4 w-4 animate-spin" />
          {t("common.loading")}
        </div>
      )}
      {jobs.map((job) => {
        const active = activeJobId === job.jobId || isInFlight(job);
        const hasActiveJob = activeJobId !== null || jobs.some(isInFlight);
        const controlsBusy =
          pausingJobId !== null || discardingJobId !== null;
        const controlsBusyReason = pausingJobId
          ? t("textToSpeech.unfinishedJobs.pauseInProgress")
          : discardingJobId
            ? t("textToSpeech.unfinishedJobs.discardInProgress")
            : null;
        const savePartialDisabled =
          job.status === "completed" ||
          !job.partialAvailable ||
          hasActiveJob ||
          controlsBusy;
        let savePartialDisabledReason: string | null = null;
        if (job.status === "completed") {
          savePartialDisabledReason = t(
            "textToSpeech.unfinishedJobs.savePartialDisabledCompleted",
          );
        } else if (hasActiveJob && job.partialAvailable) {
          savePartialDisabledReason = t(
            "textToSpeech.unfinishedJobs.savePartialDisabledActive",
          );
        } else if (hasActiveJob) {
          savePartialDisabledReason = t(
            "textToSpeech.unfinishedJobs.savePartialDisabledActiveEmpty",
          );
        } else if (!job.partialAvailable) {
          savePartialDisabledReason = t(
            "textToSpeech.unfinishedJobs.savePartialDisabledEmpty",
          );
        }
        const savePartialLabel = t(
          "textToSpeech.unfinishedJobs.savePartial",
        );
        const savePartialButton = (
          <Button
            variant="secondary"
            disabled={savePartialDisabled}
            aria-label={
              savePartialDisabledReason
                ? `${savePartialLabel}. ${savePartialDisabledReason}`
                : savePartialLabel
            }
            onClick={() => void exportPartial(job)}
          >
            <Download className="mr-2 inline h-4 w-4" />
            {savePartialLabel}
          </Button>
        );
        const completed = active
          ? (liveProgress?.completed ?? job.completedChunks)
          : job.completedChunks;
        const total = active
          ? (liveProgress?.total ?? job.totalChunks)
          : job.totalChunks;
        const progressStage = active
          ? (liveProgress?.stage ?? job.progressStage)
          : job.progressStage;
        const percent = total > 0 ? Math.round((completed / total) * 100) : 0;
        return (
          <div
            key={job.jobId}
            className="border-b border-white/[0.05] px-6 py-4 last:border-b-0"
          >
            <div className="flex flex-col gap-3">
              <div className="min-w-0">
                <div className="flex items-start gap-2 text-sm font-medium text-[#f2f2f2]">
                  <span className="shrink-0 text-xs font-semibold text-[#a0a0a0]">
                    {t("textToSpeech.unfinishedJobs.from")}
                  </span>
                  <span className="break-all" title={job.sourcePath}>
                    {job.sourcePath}
                  </span>
                </div>
                <div className="mt-1 flex items-start gap-2 text-xs text-[#888]">
                  <span className="shrink-0 font-semibold text-[#a0a0a0]">
                    {t("textToSpeech.unfinishedJobs.to")}
                  </span>
                  <span className="break-all" title={job.outputPath}>
                    {job.outputPath}
                  </span>
                </div>
                <div className="mt-2 text-xs text-[#b8b8b8]">
                  {t(
                    progressStage === "ai_cleanup"
                      ? "textToSpeech.unfinishedJobs.cleanupProgress"
                      : "textToSpeech.unfinishedJobs.speechProgress",
                    {
                      completed,
                      total,
                      percent,
                    },
                  )}
                </div>
                <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-[#252525]">
                  <div
                    className="h-full rounded-full bg-[#9b5de5]"
                    style={{ width: `${Math.min(100, percent)}%` }}
                  />
                </div>
                {job.lastError && !active && (
                  <div className="mt-2 flex items-start gap-1.5 text-xs text-red-300">
                    <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                    <span>{job.lastError}</span>
                  </div>
                )}
              </div>
              <div className="flex w-full flex-wrap gap-2">
                {job.status === "completed" ? null : active ? (
                  <ActionTooltip content={controlsBusyReason}>
                    <Button
                      variant="secondary"
                      disabled={controlsBusy}
                      onClick={() => void pauseJob(job)}
                    >
                      {pausingJobId === job.jobId ? (
                        <Loader2 className="mr-2 inline h-4 w-4 animate-spin" />
                      ) : (
                        <Pause className="mr-2 inline h-4 w-4" />
                      )}
                      {pausingJobId === job.jobId
                        ? t("textToSpeech.unfinishedJobs.pausing")
                        : t("textToSpeech.unfinishedJobs.pause")}
                    </Button>
                  </ActionTooltip>
                ) : (
                  <ActionTooltip
                    content={
                      controlsBusyReason ??
                      (hasActiveJob
                        ? t("textToSpeech.unfinishedJobs.resumeBlockedActive")
                        : null)
                    }
                  >
                    <Button
                      disabled={hasActiveJob || controlsBusy}
                      onClick={() => void resumeJob(job)}
                    >
                      <Play className="mr-2 inline h-4 w-4" />
                      {t("textToSpeech.unfinishedJobs.resume")}
                    </Button>
                  </ActionTooltip>
                )}
                {savePartialDisabledReason ? (
                  <Tooltip content={savePartialDisabledReason}>
                    {savePartialButton}
                  </Tooltip>
                ) : (
                  savePartialButton
                )}
                <ActionTooltip content={controlsBusyReason}>
                  <Button
                    variant="danger"
                    disabled={controlsBusy}
                    onClick={() => void discardJob(job)}
                  >
                    {discardingJobId === job.jobId ? (
                      <Loader2 className="mr-2 inline h-4 w-4 animate-spin" />
                    ) : (
                      <Trash2 className="mr-2 inline h-4 w-4" />
                    )}
                    {discardingJobId === job.jobId
                      ? t("textToSpeech.unfinishedJobs.discarding")
                      : t("textToSpeech.unfinishedJobs.discard")}
                  </Button>
                </ActionTooltip>
              </div>
            </div>
          </div>
        );
      })}
      {error && (
        <div role="alert" className="px-6 py-4 text-sm text-red-300">
          {error}
        </div>
      )}
    </SettingsGroup>
  );
};

export default TtsUnfinishedJobs;
