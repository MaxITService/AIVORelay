import { create } from "zustand";
import { persist } from "zustand/middleware";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { type as osType } from "@tauri-apps/plugin-os";
import { outputPathKey, pathKey, uniqueOutputPath, newQueueId } from "@/lib/batchQueueUtils";

// Input paths are case-sensitive except on Windows (see pathKey).
const inputPathKey = (path: string) => pathKey(path, osType() === "windows");

export type TtsOutputFormat = "mp3" | "opus" | "wav";

export type BatchFileStatus =
  | "queued"
  | "processing"
  | "completed"
  | "skipped"
  | "failed";

export type BatchFilePlan = {
  inputPath: string;
  relativePath: string;
  outputPath: string;
  sourceCharacters: number;
  scanError?: string | null;
};

/**
 * A queued file plus the scan root its relative path was computed against;
 * the backend re-checks the relative path against that same root.
 */
export type QueuedBatchFile = BatchFilePlan & {
  sourceDirectory?: string | null;
  sourceRecursive?: boolean;
};

export type BatchScanResult = {
  inputDirectory?: string | null;
  outputDirectory: string;
  recursive: boolean;
  outputFormat: TtsOutputFormat;
  files: BatchFilePlan[];
  eligibleCount: number;
  warnings: string[];
};

export type BatchFileResult = {
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

export type BatchSummary = {
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

export type BatchProgress = Omit<BatchSummary, "files"> & {
  done: boolean;
  startedAtMs: number;
  workStartedAtMs?: number | null;
  message?: string | null;
  file?: BatchFileResult | null;
};

export type ActiveBatchSnapshot = {
  progress: BatchProgress;
  scan: BatchScanResult;
  files: BatchFileResult[];
};

/** Settings frozen when a queue starts; every batch of the queue reuses them. */
export type TtsBatchQueueMeta = {
  outputDirectory: string;
  outputFormat: TtsOutputFormat;
  mp3Bitrate: number;
  opusBitrate: number;
};

export const queuedRow = (
  file: BatchFilePlan,
  index: number,
): BatchFileResult => ({
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

export const rowsFromSnapshot = (
  snapshot: ActiveBatchSnapshot,
): BatchFileResult[] => {
  const rows = snapshot.scan.files.map(queuedRow);
  for (const file of [...snapshot.files, snapshot.progress.file].filter(
    (item): item is BatchFileResult => !!item,
  )) {
    if (file.index >= 0 && file.index < rows.length) rows[file.index] = file;
  }
  return rows;
};

interface TtsBatchQueueState {
  /** Persisted: meta of the queue, needed to resume it after a restart. */
  meta: TtsBatchQueueMeta | null;
  /**
   * Persisted: every file that is not finished yet, in run order. It includes
   * the files of the batch that is running now and the files waiting behind it.
   */
  queue: QueuedBatchFile[];
  /**
   * Persisted: input path keys converted since the queue started. A rescan
   * plans a `-N` output beside an existing result, so re-adding a converted
   * file would otherwise convert it again.
   */
  completedInputs: string[];
  running: boolean;
  stopping: boolean;
  /** True when the running backend batch was adopted after a webview reload. */
  attached: boolean;
  activeClientId: string | null;
  batchId: string | null;
  runScan: BatchScanResult | null;
  /** Queue entry keys of the files in the running backend batch. */
  runningEntries: string[];
  rows: BatchFileResult[];
  progress: BatchProgress | null;
  summary: BatchSummary | null;
  currentFile: BatchFileResult | null;
  error: string | null;
  /** Replaces the queue with the scanned files and starts converting. */
  beginFromScan: (
    scan: BatchScanResult,
    bitrates: { mp3Bitrate: number; opusBitrate: number },
  ) => Promise<void>;
  /** Starts converting the saved queue (after Stop or an app restart). */
  resume: () => Promise<void>;
  /** Appends scanned files behind the queue; returns how many were new. */
  enqueue: (scan: BatchScanResult) => number;
  /** Outputs owned by queued or running files; pass them to the scan. */
  reservedOutputs: () => string[];
  /** Removes files that have not started; the running batch is untouched. */
  clearWaiting: () => void;
  resetRun: () => void;
  stop: (idUnavailableMessage: string) => Promise<void>;
  attachActiveBatch: (
    snapshot: ActiveBatchSnapshot,
    bitrates: { mp3Bitrate: number; opusBitrate: number },
  ) => void;
  setError: (error: string | null) => void;
}

const waitForTick = (delayMs: number) =>
  new Promise<void>((resolve) => window.setTimeout(resolve, delayMs));

const isFileFinishedForQueue = (
  status: BatchFileStatus,
  cancelled: boolean,
): boolean =>
  status === "completed" ||
  (!cancelled && (status === "failed" || status === "skipped"));

const withSource = (
  scan: Pick<BatchScanResult, "inputDirectory" | "recursive">,
  files: BatchFilePlan[],
): QueuedBatchFile[] =>
  files.map((file) => ({
    ...file,
    sourceDirectory: scan.inputDirectory ?? null,
    sourceRecursive: Boolean(scan.inputDirectory) && scan.recursive,
  }));

// A failed file re-added during its batch gets another output name, so its
// new queue entry must not be taken for the finished or running original.
export const entryKey = (file: Pick<BatchFilePlan, "inputPath" | "outputPath">) =>
  `${inputPathKey(file.inputPath)}|${outputPathKey(file.outputPath)}`;

const sourceKey = (file: QueuedBatchFile) =>
  `${file.sourceRecursive ? "r" : "-"}:${inputPathKey(file.sourceDirectory ?? "")}`;

const toPlan = ({
  sourceDirectory: _sourceDirectory,
  sourceRecursive: _sourceRecursive,
  ...file
}: QueuedBatchFile): BatchFilePlan => file;

const addCompleted = (
  completed: string[],
  files: Pick<BatchFileResult, "inputPath" | "status">[],
): string[] => {
  const keys = new Set(completed);
  for (const file of files) {
    if (file.status === "completed") keys.add(inputPathKey(file.inputPath));
  }
  return keys.size === completed.length ? completed : [...keys];
};

let listenerRegistered: Promise<void> | null = null;
// Retain events received during the recovery lookup, before the store can
// identify the surviving batch. Only the most recently observed batch is kept.
let observedBatch: {
  progress: BatchProgress;
  files: Map<number, BatchFileResult>;
} | null = null;

export const useTtsBatchQueueStore = create<TtsBatchQueueState>()(
  persist(
    (set, get) => {
      const removeFromQueue = (keys: Set<string>) =>
        set((state) => ({
          queue: state.queue.filter((file) => !keys.has(entryKey(file))),
        }));

      const runQueue = async () => {
        const initial = get();
        if (initial.running || !initial.meta || initial.queue.length === 0) return;
        set({ running: true, stopping: false, summary: null, error: null });
        try {
          // Events must reach the store before the first batch starts.
          await ensureTtsBatchListener();
          for (;;) {
            const state = get();
            const meta = state.meta;
            if (state.stopping || !meta || state.queue.length === 0) break;
            // One batch covers the leading files of one scan root, because the
            // backend checks every relative path against the batch root.
            const head = state.queue[0];
            const files: QueuedBatchFile[] = [];
            for (const file of state.queue) {
              if (sourceKey(file) !== sourceKey(head)) break;
              files.push(file);
            }
            const scan: BatchScanResult = {
              inputDirectory: head.sourceDirectory ?? null,
              outputDirectory: meta.outputDirectory,
              recursive: head.sourceRecursive ?? false,
              outputFormat: meta.outputFormat,
              files: files.map(toPlan),
              eligibleCount: files.filter((file) => !file.scanError).length,
              warnings: [],
            };
            const clientId = newQueueId();
            set({
              activeClientId: clientId,
              batchId: null,
              runScan: scan,
              runningEntries: files.map(entryKey),
              rows: files.map(queuedRow),
              progress: null,
              currentFile: null,
            });

            let result: BatchSummary;
            try {
              result = await invoke<BatchSummary>("convert_tts_batch", {
                request: {
                  clientId,
                  scan,
                  mp3Bitrate: meta.mp3Bitrate,
                  opusBitrate: meta.opusBitrate,
                },
              });
            } catch (batchError) {
              set({ error: String(batchError) });
              break;
            }

            const finished = new Set(
              result.files
                .filter((file) =>
                  isFileFinishedForQueue(file.status, result.cancelled),
                )
                .map(entryKey),
            );
            if (!result.cancelled) {
              // Nothing of a completed batch may run again.
              for (const file of files) finished.add(entryKey(file));
            }
            removeFromQueue(finished);
            set((current) => ({
              completedInputs: addCompleted(current.completedInputs, result.files),
              rows: result.files,
              summary: result,
              progress: {
                clientId: result.clientId,
                batchId: result.batchId,
                total: result.total,
                finished: result.finished,
                completed: result.completed,
                skipped: result.skipped,
                failed: result.failed,
                cancelled: result.cancelled,
                done: true,
                startedAtMs: current.progress?.startedAtMs ?? Date.now(),
                workStartedAtMs: current.progress?.workStartedAtMs ?? null,
                message: current.progress?.message ?? null,
                file: null,
              },
              activeClientId: null,
              runningEntries: [],
              currentFile: null,
            }));
            // Other scan roots and files added meanwhile run in the next turn.
            if (result.cancelled || get().stopping) break;
          }
        } catch (error) {
          set({ error: String(error) });
        } finally {
          set({
            running: false,
            stopping: false,
            attached: false,
            activeClientId: null,
            runningEntries: [],
            currentFile: null,
          });
          window.dispatchEvent(new Event("aivorelay:tts-jobs-changed"));
        }
      };

      return {
        meta: null,
        queue: [],
        completedInputs: [],
        running: false,
        stopping: false,
        attached: false,
        activeClientId: null,
        batchId: null,
        runScan: null,
        runningEntries: [],
        rows: [],
        progress: null,
        summary: null,
        currentFile: null,
        error: null,
        beginFromScan: async (scan, bitrates) => {
          if (get().running) return;
          set({
            meta: {
              outputDirectory: scan.outputDirectory,
              outputFormat: scan.outputFormat,
              mp3Bitrate: bitrates.mp3Bitrate,
              opusBitrate: bitrates.opusBitrate,
            },
            queue: withSource(scan, scan.files),
            completedInputs: [],
          });
          await runQueue();
        },
        resume: async () => {
          await runQueue();
        },
        enqueue: (scan) => {
          const { queue, completedInputs, reservedOutputs } = get();
          const knownInputs = new Set([
            ...queue.map((file) => inputPathKey(file.inputPath)),
            ...completedInputs,
          ]);
          const takenOutputs = new Set(reservedOutputs().map(outputPathKey));
          const added: QueuedBatchFile[] = [];
          for (const file of withSource(scan, scan.files)) {
            const inputKey = inputPathKey(file.inputPath);
            if (knownInputs.has(inputKey)) continue;
            // The scan already avoided disk and queue names; this only covers
            // files queued while the scan was running.
            const outputPath = takenOutputs.has(outputPathKey(file.outputPath))
              ? uniqueOutputPath(file.outputPath, takenOutputs)
              : file.outputPath;
            knownInputs.add(inputKey);
            takenOutputs.add(outputPathKey(outputPath));
            added.push({ ...file, outputPath });
          }
          if (added.length > 0) set({ queue: [...queue, ...added] });
          return added.length;
        },
        reservedOutputs: () => {
          const { queue, runScan, running } = get();
          return [...queue, ...(running ? runScan?.files ?? [] : [])]
            .map((file) => file.outputPath);
        },
        clearWaiting: () =>
          set((state) => {
            const running = new Set(state.runningEntries);
            return {
              queue: state.queue.filter((file) => running.has(entryKey(file))),
            };
          }),
        resetRun: () => {
          if (get().running) return;
          set({
            runScan: null,
            rows: [],
            progress: null,
            summary: null,
            currentFile: null,
            batchId: null,
            error: null,
          });
        },
        stop: async (idUnavailableMessage) => {
          if (!get().running || get().stopping) return;
          set({ stopping: true, error: null });
          try {
            let batchId = get().batchId;
            for (
              let attempt = 0;
              !batchId && get().running && attempt < 1_300;
              attempt += 1
            ) {
              await waitForTick(100);
              batchId = get().batchId;
            }
            if (!batchId) {
              if (!get().running) {
                set({ stopping: false });
                return;
              }
              throw new Error(idUnavailableMessage);
            }
            await invoke("cancel_tts_batch", { batchId });
          } catch (cancelError) {
            set({ error: String(cancelError), stopping: false });
          }
        },
        attachActiveBatch: (snapshot, bitrates) => {
          if (get().running) return;
          const observed = observedBatch?.progress.clientId === snapshot.progress.clientId
            ? observedBatch : null;
          const active = observed && (
            observed.progress.done || observed.progress.finished > snapshot.progress.finished
          ) ? observed.progress : snapshot.progress;
          const rows = rowsFromSnapshot(snapshot);
          for (const file of observed?.files.values() ?? []) {
            const previous = rows[file.index];
            if (previous && (previous.status === "queued" || (
              previous.status === "processing" && file.status !== "queued"
            ))) rows[file.index] = file;
          }
          const finished = new Set(rows
            .filter((row) => isFileFinishedForQueue(row.status, active.cancelled))
            .map(entryKey));
          const pending = new Map(
            [...withSource(snapshot.scan, snapshot.scan.files), ...get().queue]
              .filter((file) => !finished.has(entryKey(file)))
              .map((file) => [inputPathKey(file.inputPath), file]),
          );
          set({
            meta: get().meta ?? {
              outputDirectory: snapshot.scan.outputDirectory,
              outputFormat: snapshot.scan.outputFormat,
              ...bitrates,
            },
            queue: [...pending.values()],
            completedInputs: addCompleted(get().completedInputs, rows),
            running: !active.done,
            attached: !active.done,
            stopping: false,
            activeClientId: active.done ? null : active.clientId,
            batchId: active.batchId,
            runScan: snapshot.scan,
            runningEntries: active.done ? [] : snapshot.scan.files.map(entryKey),
            rows,
            progress: active,
            summary: active.done ? {
              clientId: active.clientId,
              batchId: active.batchId,
              total: active.total,
              finished: active.finished,
              completed: active.completed,
              skipped: active.skipped,
              failed: active.failed,
              cancelled: active.cancelled,
              files: rows,
            } : null,
            currentFile: !active.done && active.file?.status === "processing" ? active.file : null,
            error: null,
          });
        },
        setError: (error) => set({ error }),
      };
    },
    {
      name: "aivorelay-tts-batch-queue-v1",
      partialize: (state) => ({
        meta: state.meta,
        queue: state.queue,
        completedInputs: state.completedInputs,
      }),
    },
  ),
);

/**
 * Registers the app-lifetime listener that keeps the queue store in sync with
 * the backend batch events. It outlives the settings page, so a queue keeps
 * running while the user visits another section.
 */
export const ensureTtsBatchListener = (): Promise<void> => {
  if (listenerRegistered) return listenerRegistered;
  listenerRegistered = listen<BatchProgress>("tts://batch-progress", (event) => {
    const store = useTtsBatchQueueStore;
    const state = store.getState();
    const next = event.payload;
    if (observedBatch?.progress.clientId !== next.clientId) {
      observedBatch = { progress: next, files: new Map() };
    }
    observedBatch.progress = next;
    if (next.file) observedBatch.files.set(next.file.index, next.file);
    if (!state.running || next.clientId !== state.activeClientId) return;

    const file = next.file;
    const patch: Partial<TtsBatchQueueState> = {
      batchId: next.batchId,
      progress: next,
    };
    if (file) {
      patch.currentFile = file.status === "processing" ? file : null;
      patch.rows = state.rows.map((row) =>
        row.index === file.index ? file : row,
      );
      if (file.status === "completed") {
        patch.completedInputs = addCompleted(state.completedInputs, [file]);
      }
      if (isFileFinishedForQueue(file.status, next.cancelled)) {
        const key = entryKey(file);
        patch.queue = state.queue.filter((planned) => entryKey(planned) !== key);
      }
    }
    if (next.done && state.attached) {
      patch.running = false;
      patch.attached = false;
      patch.stopping = false;
      patch.activeClientId = null;
      patch.runningEntries = [];
      patch.currentFile = null;
      patch.summary = {
        clientId: next.clientId,
        batchId: next.batchId,
        total: next.total,
        finished: next.finished,
        completed: next.completed,
        skipped: next.skipped,
        failed: next.failed,
        cancelled: next.cancelled,
        files: patch.rows ?? state.rows,
      };
    }
    store.setState(patch);

    if (next.done && state.attached) {
      window.dispatchEvent(new Event("aivorelay:tts-jobs-changed"));
      if (!next.cancelled && !state.stopping && store.getState().queue.length > 0) {
        // Let the backend release its batch lease before the next batch.
        window.setTimeout(() => void store.getState().resume(), 500);
      }
    }
  }).then(() => undefined).catch((error) => {
    listenerRegistered = null;
    throw error;
  });
  return listenerRegistered;
};
