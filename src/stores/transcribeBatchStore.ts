import { create } from "zustand";
import { persist } from "zustand/middleware";
import { commands } from "@/bindings";
import { type as osType } from "@tauri-apps/plugin-os";
import { isCancellationMessage, newQueueId, pathKey } from "@/lib/batchQueueUtils";
import {
  useTranscribeFileStore,
  type FileTranscriptionRequest,
} from "./transcribeFileStore";

// Input paths are case-sensitive except on Windows (see pathKey).
const inputPathKey = (path: string) => pathKey(path, osType() === "windows");

export type TranscribeBatchStatus = "queued" | "processing" | "completed" | "failed";
export type TranscribeBatchWaitingReason = "singleFile" | "recording" | null;

export interface TranscribeBatchItem {
  id: string;
  path: string;
  name: string;
  status: TranscribeBatchStatus;
  error: string | null;
  savedFilePath: string | null;
  request: FileTranscriptionRequest;
}

interface TranscribeBatchState {
  items: TranscribeBatchItem[];
  running: boolean;
  stopping: boolean;
  activeItemId: string | null;
  waitingReason: TranscribeBatchWaitingReason;
  error: string | null;
  addFiles: (requests: FileTranscriptionRequest[]) => number;
  removeItem: (id: string) => void;
  clearFinished: () => void;
  retryFailed: () => void;
  start: () => Promise<void>;
  stop: () => Promise<void>;
}

const waitForTick = () => new Promise<void>((resolve) => setTimeout(resolve, 1_000));
let pendingCancellation: Promise<void> | null = null;

export const useTranscribeBatchStore = create<TranscribeBatchState>()(
  persist(
    (set, get) => {
      const updateItem = (id: string, patch: Partial<TranscribeBatchItem>) =>
        set((state) => ({
          items: state.items.map((item) => item.id === id ? { ...item, ...patch } : item),
        }));

      return {
        items: [],
        running: false,
        stopping: false,
        activeItemId: null,
        waitingReason: null,
        error: null,
        addFiles: (requests) => {
          const knownPaths = new Set(
            get().items
              .filter((item) => item.status === "queued" || item.status === "processing")
              .map((item) => inputPathKey(item.path)),
          );
          const added: TranscribeBatchItem[] = [];
          for (const request of requests) {
            if (knownPaths.has(inputPathKey(request.filePath))) continue;
            knownPaths.add(inputPathKey(request.filePath));
            added.push({
              id: newQueueId(),
              path: request.filePath,
              name: request.filePath.split(/[\\/]/).pop() || request.filePath,
              status: "queued",
              error: null,
              savedFilePath: null,
              request: {
                ...request,
                saveToFile: true,
                sonioxOptionsOverride: request.sonioxOptionsOverride
                  ? {
                      ...request.sonioxOptionsOverride,
                      languageHints: request.sonioxOptionsOverride.languageHints?.slice() ?? null,
                    }
                  : null,
                deepgramOptionsOverride: request.deepgramOptionsOverride
                  ? { ...request.deepgramOptionsOverride }
                  : null,
              },
            });
          }
          if (added.length > 0) set((state) => ({ items: [...state.items, ...added] }));
          return added.length;
        },
        removeItem: (id) => set((state) => ({
          items: state.items.filter((item) => item.id !== id || item.status === "processing"),
        })),
        clearFinished: () => set((state) => ({
          items: state.items.filter((item) => item.status === "queued" || item.status === "processing"),
        })),
        retryFailed: () => {
          const knownPaths = new Set(
            get().items
              .filter((item) => item.status === "queued" || item.status === "processing")
              .map((item) => inputPathKey(item.path)),
          );
          set((state) => ({
            items: state.items.map((item) => {
              if (item.status !== "failed" || knownPaths.has(inputPathKey(item.path))) return item;
              knownPaths.add(inputPathKey(item.path));
              return { ...item, status: "queued", error: null };
            }),
          }));
        },
        start: async () => {
          if (get().running || !get().items.some((item) => item.status === "queued")) return;
          set({ running: true, stopping: false, error: null });
          try {
            while (!get().stopping) {
              const next = get().items.find((item) => item.status === "queued");
              if (!next) break;
              const single = useTranscribeFileStore.getState();
              if (single.isTranscriptionCommandPending || single.isCancellationCommandPending) {
                set({ waitingReason: "singleFile" });
                await waitForTick();
                continue;
              }
              const recording = await commands.getFileTranscriptionRecordingState(next.request.modelOverride);
              if (get().stopping) break;
              // The user may remove an item while the admission check is pending.
              if (!get().items.some((item) => item.id === next.id && item.status === "queued")) continue;
              if (recording.blocksFileTranscription) {
                set({ waitingReason: "recording" });
                await waitForTick();
                continue;
              }
              const currentSingle = useTranscribeFileStore.getState();
              if (currentSingle.isTranscriptionCommandPending || currentSingle.isCancellationCommandPending) continue;
              set({ activeItemId: next.id, waitingReason: null });
              updateItem(next.id, { status: "processing", error: null });
              const request = next.request;
              try {
                const result = await commands.transcribeAudioFile(
                  request.filePath,
                  request.profileId,
                  true,
                  request.outputFormat,
                  request.modelOverride,
                  request.customWordsEnabledOverride,
                  request.sonioxOptionsOverride,
                  request.deepgramOptionsOverride,
                );
                if (result.status === "error") throw result.error;
                updateItem(next.id, { status: "completed", savedFilePath: result.data.saved_file_path ?? null });
              } catch (error) {
                const cancelled = get().stopping && isCancellationMessage(error);
                updateItem(next.id, {
                  status: cancelled ? "queued" : "failed",
                  error: cancelled ? null : String(error),
                });
              } finally {
                set({ activeItemId: null });
              }
            }
          } catch (error) {
            // An admission check failure pauses the queue without losing a file.
            set({ error: String(error) });
          } finally {
            // A late cancel must never reach a subsequent run or single-file job.
            await pendingCancellation;
            set({ running: false, stopping: false, activeItemId: null, waitingReason: null });
          }
        },
        stop: async () => {
          if (!get().running || get().stopping) return;
          set({ stopping: true, error: null });
          if (!get().activeItemId) return;
          pendingCancellation = commands.cancelFileTranscription()
            .catch((error) => { set({ error: String(error) }); })
            .finally(() => { pendingCancellation = null; });
          await pendingCancellation;
        },
      };
    },
    {
      name: "aivorelay-transcribe-file-batch-v1",
      partialize: (state) => ({ items: state.items }),
      merge: (persisted, current) => ({
        ...current,
        items: ((persisted as { items?: TranscribeBatchItem[] } | undefined)?.items ?? [])
          .map((item) => item.status === "processing" ? { ...item, status: "queued" as const } : item),
      }),
    },
  ),
);
