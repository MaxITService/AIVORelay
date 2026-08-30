import { create } from "zustand";
import { persist } from "zustand/middleware";
import type {
  DiarizedTranscriptProvider,
  FileTranscriptionSpeakerSession,
} from "@/bindings";

export type OutputMode = "textarea" | "file";
export type OutputFormat = "text" | "srt" | "vtt";

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
  isTranscribing: boolean;
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
  setIsTranscribing: (isTranscribing: boolean) => void;
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

export const useTranscribeFileStore = create<TranscribeFileState>()(
  persist(
    (set) => ({
  selectedFile: null,
  outputMode: "textarea",
  outputFormat: "text",
  customWordsEnabledOverride: true,
  transcriptionResult: "",
  savedFilePath: null,
  error: null,
  isTranscribing: false,
  activeModelKey: null,
  modelUiConfigs: {},
  ...emptySpeakerState(),
  setSelectedFile: (selectedFile) => set({ selectedFile, ...emptySpeakerState() }),
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
  setIsTranscribing: (isTranscribing) => set({ isTranscribing }),
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
    set({
      speakerArtifactPath: speakerSession?.artifact_path ?? null,
      speakerProvider: speakerSession?.provider ?? null,
      speakerCards:
        speakerSession?.speakers.map((speaker) => ({
          speakerId: speaker.speaker_id,
          defaultName: speaker.default_name,
          name: speaker.default_name,
        })) ?? [],
      isReapplyingSpeakerNames: false,
    }),
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
