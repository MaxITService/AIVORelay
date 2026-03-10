import { create } from "zustand";
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

interface TranscribeFileState {
  selectedFile: SelectedFile | null;
  outputMode: OutputMode;
  outputFormat: OutputFormat;
  overrideModelId: string | null;
  customWordsEnabledOverride: boolean;
  transcriptionResult: string;
  savedFilePath: string | null;
  error: string | null;
  isTranscribing: boolean;
  selectedProfileId: string | null;
  speakerArtifactPath: string | null;
  speakerProvider: DiarizedTranscriptProvider | null;
  speakerCards: EditableSpeakerCard[];
  isReapplyingSpeakerNames: boolean;
  setSelectedFile: (selectedFile: SelectedFile | null) => void;
  setOutputMode: (outputMode: OutputMode) => void;
  setOutputFormat: (outputFormat: OutputFormat) => void;
  setOverrideModelId: (overrideModelId: string | null) => void;
  setCustomWordsEnabledOverride: (customWordsEnabledOverride: boolean) => void;
  setTranscriptionResult: (transcriptionResult: string) => void;
  setSavedFilePath: (savedFilePath: string | null) => void;
  setError: (error: string | null) => void;
  setIsTranscribing: (isTranscribing: boolean) => void;
  setSelectedProfileId: (selectedProfileId: string | null) => void;
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

export const useTranscribeFileStore = create<TranscribeFileState>((set) => ({
  selectedFile: null,
  outputMode: "textarea",
  outputFormat: "text",
  overrideModelId: null,
  customWordsEnabledOverride: true,
  transcriptionResult: "",
  savedFilePath: null,
  error: null,
  isTranscribing: false,
  selectedProfileId: null,
  ...emptySpeakerState(),
  setSelectedFile: (selectedFile) => set({ selectedFile, ...emptySpeakerState() }),
  setOutputMode: (outputMode) => set({ outputMode }),
  setOutputFormat: (outputFormat) => set({ outputFormat }),
  setOverrideModelId: (overrideModelId) => set({ overrideModelId }),
  setCustomWordsEnabledOverride: (customWordsEnabledOverride) =>
    set({ customWordsEnabledOverride }),
  setTranscriptionResult: (transcriptionResult) => set({ transcriptionResult }),
  setSavedFilePath: (savedFilePath) => set({ savedFilePath }),
  setError: (error) => set({ error }),
  setIsTranscribing: (isTranscribing) => set({ isTranscribing }),
  setSelectedProfileId: (selectedProfileId) => set({ selectedProfileId }),
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
}));
