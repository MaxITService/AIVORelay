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
  partialAvailable: boolean;
  lastError?: string | null;
  createdAtMs: number;
  updatedAtMs: number;
};

type TtsStateEvent = {
  kind?: string | null;
  operationId?: string | number;
  operation_id?: string | number;
  completedChunks?: number;
  completed_chunks?: number;
  totalChunks?: number;
  total_chunks?: number;
};

const partialDefaultPath = (job: TtsFileJob) => {
  const suffix = `.${job.outputFormat}`;
  return job.outputPath.toLocaleLowerCase().endsWith(suffix)
    ? `${job.outputPath.slice(0, -suffix.length)}.partial${suffix}`
    : `${job.outputPath}.partial${suffix}`;
};

export const TtsUnfinishedJobs: React.FC = () => {
  const { t } = useTranslation();
  const [jobs, setJobs] = useState<TtsFileJob[]>([]);
  const [loading, setLoading] = useState(true);
  const [activeJobId, setActiveJobId] = useState<string | null>(null);
  const [operationId, setOperationId] = useState<string | null>(null);
  const [liveProgress, setLiveProgress] = useState<{
    completed: number;
    total: number;
  } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const activeJobIdRef = useRef<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const result = await invoke<TtsFileJob[]>("list_tts_file_jobs");
      setJobs(result);
    } catch (refreshError) {
      setError(String(refreshError));
    } finally {
      setLoading(false);
    }
  }, []);

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
      if (id !== undefined) setOperationId(String(id));
      setLiveProgress({
        completed:
          event.payload.completedChunks ?? event.payload.completed_chunks ?? 0,
        total: event.payload.totalChunks ?? event.payload.total_chunks ?? 0,
      });
    }).then((dispose) => {
      if (disposed) dispose();
      else unlisten = dispose;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const resumeJob = async (job: TtsFileJob) => {
    activeJobIdRef.current = job.jobId;
    setActiveJobId(job.jobId);
    setOperationId(null);
    setLiveProgress(null);
    setError(null);
    try {
      await invoke("resume_tts_file_job", { jobId: job.jobId });
    } catch (resumeError) {
      setError(String(resumeError));
    } finally {
      activeJobIdRef.current = null;
      setActiveJobId(null);
      setOperationId(null);
      setLiveProgress(null);
      await refresh();
    }
  };

  const pauseJob = async () => {
    if (!operationId) return;
    try {
      await invoke("cancel_tts_operation", { operationId });
    } catch (pauseError) {
      setError(String(pauseError));
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
    if (!window.confirm(t("textToSpeech.unfinishedJobs.discardConfirm")))
      return;
    setError(null);
    try {
      await invoke("discard_tts_file_job", { jobId: job.jobId });
      await refresh();
    } catch (discardError) {
      setError(String(discardError));
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
        const active = activeJobId === job.jobId;
        const completed = active
          ? (liveProgress?.completed ?? job.completedChunks)
          : job.completedChunks;
        const total = active
          ? (liveProgress?.total ?? job.totalChunks)
          : job.totalChunks;
        const percent = total > 0 ? Math.round((completed / total) * 100) : 0;
        return (
          <div
            key={job.jobId}
            className="border-b border-white/[0.05] px-6 py-4 last:border-b-0"
          >
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div className="min-w-0 flex-1">
                <div className="truncate text-sm font-medium text-[#f2f2f2]">
                  {job.sourcePath}
                </div>
                <div className="mt-1 break-all text-xs text-[#888]">
                  {job.outputPath}
                </div>
                <div className="mt-2 text-xs text-[#b8b8b8]">
                  {t("textToSpeech.unfinishedJobs.progress", {
                    completed,
                    total,
                    percent,
                  })}
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
              <div className="flex flex-wrap gap-2">
                {job.status === "completed" ? null : active ? (
                  <Button
                    variant="secondary"
                    disabled={!operationId}
                    onClick={() => void pauseJob()}
                  >
                    <Pause className="mr-2 inline h-4 w-4" />
                    {t("textToSpeech.unfinishedJobs.pause")}
                  </Button>
                ) : (
                  <Button
                    disabled={activeJobId !== null}
                    onClick={() => void resumeJob(job)}
                  >
                    <Play className="mr-2 inline h-4 w-4" />
                    {t("textToSpeech.unfinishedJobs.resume")}
                  </Button>
                )}
                <Button
                  variant="secondary"
                  disabled={
                    job.status === "completed" ||
                    !job.partialAvailable ||
                    activeJobId !== null
                  }
                  onClick={() => void exportPartial(job)}
                >
                  <Download className="mr-2 inline h-4 w-4" />
                  {t("textToSpeech.unfinishedJobs.savePartial")}
                </Button>
                <Button
                  variant="danger"
                  disabled={activeJobId !== null}
                  onClick={() => void discardJob(job)}
                >
                  <Trash2 className="mr-2 inline h-4 w-4" />
                  {t("textToSpeech.unfinishedJobs.discard")}
                </Button>
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
