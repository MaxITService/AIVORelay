import { create } from "zustand";
import { persist } from "zustand/middleware";
import { commands } from "@/bindings";
import type {
  DeepgramFileTranscriptionOptions,
  DiarizedTranscriptProvider,
  FileTranscriptionChunkTraceEntry,
  FileTranscriptionSpeakerSession,
  SonioxFileTranscriptionOptions,
} from "@/bindings";

export type OutputMode = "textarea" | "file";
export type OutputFormat = "text" | "srt" | "vtt";

export type FileTranscriptionPhase =
  | "idle"
  | "transcribing"
  | "cancelling"
  | "completed"
  | "cancelled"
  | "failed";

export interface FileTranscriptionRequest {
  filePath: string;
  profileId: string | null;
  saveToFile: boolean;
  outputFormat: OutputFormat;
  modelOverride: string | null;
  modelLabel?: string | null;
  customWordsEnabledOverride: boolean | null;
  sonioxOptionsOverride: SonioxFileTranscriptionOptions | null;
  deepgramOptionsOverride: DeepgramFileTranscriptionOptions | null;
  retryableRemoteApi: boolean;
}

type RawChunkingTraceEntry = Partial<FileTranscriptionChunkTraceEntry> & {
  chunk_index?: number;
  start_secs?: number;
  end_secs?: number;
  duration_secs?: number;
};

export interface SelectedFile {
  path: string;
  name: string;
  size: number;
  audioUrl: string | null;
  previewAssetPath: string | null;
  durationSeconds?: number | null;
}

export interface EditableSpeakerCard {
  speakerId: number;
  defaultName: string;
  name: string;
}

interface FileModelUiConfig {
  outputMode: OutputMode;
  outputFormat: OutputFormat;
  customWordsEnabledOverride: boolean;
}

interface TranscribeFileState {
  selectedFile: SelectedFile | null;
  outputMode: OutputMode;
  outputFormat: OutputFormat;
  customWordsEnabledOverride: boolean;
  transcriptionResult: string;
  savedFilePath: string | null;
  error: string | null;
  transcriptionRunId: number;
  transcriptionPhase: FileTranscriptionPhase;
  transcriptionRequest: FileTranscriptionRequest | null;
  isTranscriptionCommandPending: boolean;
  isCancellationCommandPending: boolean;
  cancelRequestedAt: number | null;
  infoMessage: string | null;
  infoMessageKey: "transcribeFile.cancelled" | null;
  chunkingTrace: FileTranscriptionChunkTraceEntry[];
  fileRetryRequest: FileTranscriptionRequest | null;
  activeModelKey: string | null;
  modelUiConfigs: Record<string, FileModelUiConfig>;
  speakerArtifactPath: string | null;
  speakerProvider: DiarizedTranscriptProvider | null;
  speakerCards: EditableSpeakerCard[];
  isReapplyingSpeakerNames: boolean;
  setSelectedFile: (selectedFile: SelectedFile | null) => void;
  setOutputMode: (outputMode: OutputMode) => void;
  setOutputFormat: (outputFormat: OutputFormat) => void;
  setCustomWordsEnabledOverride: (customWordsEnabledOverride: boolean) => void;
  setTranscriptionResult: (transcriptionResult: string) => void;
  setSavedFilePath: (savedFilePath: string | null) => void;
  setError: (error: string | null) => void;
  startTranscription: (request: FileTranscriptionRequest) => Promise<void>;
  cancelTranscription: () => Promise<void>;
  activateModelUiConfig: (modelKey: string) => void;
  setSpeakerSession: (
    speakerSession: FileTranscriptionSpeakerSession | null,
  ) => void;
  clearSpeakerSession: () => void;
  updateSpeakerCardName: (speakerId: number, name: string) => void;
  applySpeakerCardNames: (names: string[]) => void;
  setIsReapplyingSpeakerNames: (isReapplyingSpeakerNames: boolean) => void;
}

const emptySpeakerState = () => ({
  speakerArtifactPath: null as string | null,
  speakerProvider: null as DiarizedTranscriptProvider | null,
  speakerCards: [] as EditableSpeakerCard[],
  isReapplyingSpeakerNames: false,
});

const speakerSessionState = (
  speakerSession: FileTranscriptionSpeakerSession | null,
) => ({
  speakerArtifactPath: speakerSession?.artifact_path ?? null,
  speakerProvider: speakerSession?.provider ?? null,
  speakerCards:
    speakerSession?.speakers.map((speaker) => ({
      speakerId: speaker.speaker_id,
      defaultName: speaker.default_name,
      name: speaker.default_name,
    })) ?? [],
  isReapplyingSpeakerNames: false,
});

const emptyTranscriptionOutput = () => ({
  transcriptionResult: "",
  savedFilePath: null as string | null,
  error: null as string | null,
  infoMessage: null as string | null,
  infoMessageKey: null as "transcribeFile.cancelled" | null,
  chunkingTrace: [] as FileTranscriptionChunkTraceEntry[],
  fileRetryRequest: null as FileTranscriptionRequest | null,
  ...emptySpeakerState(),
});

const normalizeFiniteNumber = (...values: unknown[]): number | null => {
  for (const value of values) {
    if (typeof value === "number" && Number.isFinite(value)) {
      return value;
    }
  }
  return null;
};

const normalizeChunkingTrace = (
  value: unknown,
): FileTranscriptionChunkTraceEntry[] => {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.flatMap((rawEntry) => {
    const entry = rawEntry as RawChunkingTraceEntry;
    const chunkIndex = normalizeFiniteNumber(
      entry.chunkIndex,
      entry.chunk_index,
    );
    const startSecs = normalizeFiniteNumber(entry.startSecs, entry.start_secs);
    const endSecs = normalizeFiniteNumber(entry.endSecs, entry.end_secs);
    const durationSecs = normalizeFiniteNumber(
      entry.durationSecs,
      entry.duration_secs,
      startSecs != null && endSecs != null ? endSecs - startSecs : null,
    );
    const reason =
      typeof entry.reason === "string" && entry.reason.trim().length > 0
        ? entry.reason
        : null;

    if (
      chunkIndex == null ||
      startSecs == null ||
      endSecs == null ||
      durationSecs == null ||
      reason == null
    ) {
      return [];
    }

    return [{ chunkIndex, startSecs, endSecs, durationSecs, reason }];
  });
};

const isCancellationMessage = (value: unknown): boolean => {
  const normalized = String(value ?? "").toLowerCase();
  return normalized.includes("cancelled") || normalized.includes("canceled");
};

export const useTranscribeFileStore = create<TranscribeFileState>()(
  persist(
    (set, get) => ({
  selectedFile: null,
  outputMode: "textarea",
  outputFormat: "text",
  customWordsEnabledOverride: true,
  transcriptionRunId: 0,
  transcriptionPhase: "idle",
  transcriptionRequest: null,
  isTranscriptionCommandPending: false,
  isCancellationCommandPending: false,
  cancelRequestedAt: null,
  activeModelKey: null,
  modelUiConfigs: {},
  ...emptyTranscriptionOutput(),
  setSelectedFile: (selectedFile) =>
    set((state) => ({
      selectedFile,
      ...emptyTranscriptionOutput(),
      ...(state.isTranscriptionCommandPending || state.isCancellationCommandPending
        ? {}
        : {
            transcriptionPhase: "idle" as const,
            transcriptionRequest: null,
            cancelRequestedAt: null,
          }),
    })),
  setOutputMode: (outputMode) =>
    set((state) => ({
      outputMode,
      modelUiConfigs: state.activeModelKey
        ? {
            ...state.modelUiConfigs,
            [state.activeModelKey]: {
              outputMode,
              outputFormat: state.outputFormat,
              customWordsEnabledOverride: state.customWordsEnabledOverride,
            },
          }
        : state.modelUiConfigs,
    })),
  setOutputFormat: (outputFormat) =>
    set((state) => ({
      outputFormat,
      modelUiConfigs: state.activeModelKey
        ? {
            ...state.modelUiConfigs,
            [state.activeModelKey]: {
              outputMode: state.outputMode,
              outputFormat,
              customWordsEnabledOverride: state.customWordsEnabledOverride,
            },
          }
        : state.modelUiConfigs,
    })),
  setCustomWordsEnabledOverride: (customWordsEnabledOverride) =>
    set((state) => ({
      customWordsEnabledOverride,
      modelUiConfigs: state.activeModelKey
        ? {
            ...state.modelUiConfigs,
            [state.activeModelKey]: {
              outputMode: state.outputMode,
              outputFormat: state.outputFormat,
              customWordsEnabledOverride,
            },
          }
        : state.modelUiConfigs,
    })),
  setTranscriptionResult: (transcriptionResult) => set({ transcriptionResult }),
  setSavedFilePath: (savedFilePath) => set({ savedFilePath }),
  setError: (error) => set({ error }),
  startTranscription: async (request) => {
    if (get().isTranscriptionCommandPending || get().isCancellationCommandPending) {
      return;
    }

    const snapshot: FileTranscriptionRequest = {
      ...request,
      sonioxOptionsOverride: request.sonioxOptionsOverride
        ? {
            ...request.sonioxOptionsOverride,
            languageHints: request.sonioxOptionsOverride.languageHints?.slice() ?? null,
          }
        : null,
      deepgramOptionsOverride: request.deepgramOptionsOverride
        ? { ...request.deepgramOptionsOverride }
        : null,
    };
    const runId = get().transcriptionRunId + 1;
    const acceptsResult = () =>
      get().transcriptionRunId === runId &&
      get().transcriptionPhase === "transcribing";
    const fail = (error: unknown) => {
      if (!acceptsResult()) return;
      const cancelled = isCancellationMessage(error);
      set({
        transcriptionPhase: cancelled ? "cancelled" : "failed",
        error: cancelled ? null : String(error),
        infoMessageKey: cancelled ? "transcribeFile.cancelled" : null,
        fileRetryRequest: !cancelled && snapshot.retryableRemoteApi ? snapshot : null,
        chunkingTrace: [],
        ...emptySpeakerState(),
      });
    };

    set({
      ...emptyTranscriptionOutput(),
      transcriptionRunId: runId,
      transcriptionPhase: "transcribing",
      transcriptionRequest: snapshot,
      isTranscriptionCommandPending: true,
      isCancellationCommandPending: false,
      cancelRequestedAt: null,
    });

    try {
      const result = await commands.transcribeAudioFile(
        snapshot.filePath,
        snapshot.profileId,
        snapshot.saveToFile,
        snapshot.outputFormat,
        snapshot.modelOverride,
        snapshot.customWordsEnabledOverride,
        snapshot.sonioxOptionsOverride,
        snapshot.deepgramOptionsOverride,
      );

      if (!acceptsResult()) return;
      if (result.status === "ok") {
        set({
          transcriptionPhase: "completed",
          transcriptionResult: result.data.text,
          savedFilePath: result.data.saved_file_path ?? null,
          infoMessage: result.data.info_message ?? null,
          chunkingTrace: normalizeChunkingTrace(result.data.chunking_trace),
          ...speakerSessionState(result.data.speaker_session ?? null),
        });
      } else {
        fail(result.error);
      }
    } catch (error) {
      fail(error);
    } finally {
      if (get().transcriptionRunId === runId) {
        // Keep the cancellation phase until both invokes settle. A late cancel
        // must never reach the next transcription.
        set((state) => ({
          isTranscriptionCommandPending: false,
          ...(state.transcriptionPhase === "cancelling" && !state.isCancellationCommandPending
            ? { transcriptionPhase: "cancelled" as const, cancelRequestedAt: null }
            : {}),
        }));
      }
    }
  },
  cancelTranscription: async () => {
    const state = get();
    if (!state.isTranscriptionCommandPending || state.transcriptionPhase !== "transcribing") {
      return;
    }

    const runId = state.transcriptionRunId;
    const selectedFile = state.selectedFile;
    set({
      ...emptyTranscriptionOutput(),
      transcriptionPhase: "cancelling",
      isCancellationCommandPending: true,
      cancelRequestedAt: Date.now(),
      infoMessageKey: "transcribeFile.cancelled",
    });

    try {
      await commands.cancelFileTranscription();
    } catch (error) {
      if (get().transcriptionRunId === runId && get().selectedFile === selectedFile) {
        set({ error: String(error), infoMessageKey: null });
      }
    } finally {
      if (get().transcriptionRunId === runId) {
        set((current) => ({
          isCancellationCommandPending: false,
          ...(!current.isTranscriptionCommandPending
            ? { transcriptionPhase: "cancelled" as const, cancelRequestedAt: null }
            : {}),
        }));
      }
    }
  },
  activateModelUiConfig: (modelKey) =>
    set((state) => {
      const existing = state.modelUiConfigs[modelKey];
      if (existing) {
        return {
          activeModelKey: modelKey,
          outputMode: existing.outputMode,
          outputFormat: existing.outputFormat,
          customWordsEnabledOverride: existing.customWordsEnabledOverride,
        };
      }
      const initial = {
        outputMode: "textarea" as const,
        outputFormat: "text" as const,
        customWordsEnabledOverride: true,
      };
      return {
        activeModelKey: modelKey,
        ...initial,
        modelUiConfigs: { ...state.modelUiConfigs, [modelKey]: initial },
      };
    }),
  setSpeakerSession: (speakerSession) =>
    set(speakerSessionState(speakerSession)),
  clearSpeakerSession: () => set({ ...emptySpeakerState() }),
  updateSpeakerCardName: (speakerId, name) =>
    set((state) => ({
      speakerCards: state.speakerCards.map((card) =>
        card.speakerId === speakerId ? { ...card, name } : card,
      ),
    })),
  applySpeakerCardNames: (names) =>
    set((state) => ({
      speakerCards: state.speakerCards.map((card, index) => ({
        ...card,
        name: names[index]?.trim() ? names[index].trim() : card.defaultName,
      })),
    })),
  setIsReapplyingSpeakerNames: (isReapplyingSpeakerNames) =>
    set({ isReapplyingSpeakerNames }),
    }),
    {
      name: "aivorelay-transcribe-file-model-ui-v1",
      partialize: (state) => ({ modelUiConfigs: state.modelUiConfigs }),
    },
  ),
);
