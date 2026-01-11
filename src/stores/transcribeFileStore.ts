import { create } from "zustand";

export type OutputMode = "textarea" | "file";
export type OutputFormat = "text" | "srt" | "vtt";

export interface SelectedFile {
  path: string;
  name: string;
  size: number;
  audioUrl: string;
}

interface TranscribeFileState {
  selectedFile: SelectedFile | null;
  outputMode: OutputMode;
  outputFormat: OutputFormat;
  overrideModelId: string | null;
  transcriptionResult: string;
  savedFilePath: string | null;
  error: string | null;
  isTranscribing: boolean;
  selectedProfileId: string | null;
  setSelectedFile: (selectedFile: SelectedFile | null) => void;
  setOutputMode: (outputMode: OutputMode) => void;
  setOutputFormat: (outputFormat: OutputFormat) => void;
  setOverrideModelId: (overrideModelId: string | null) => void;
  setTranscriptionResult: (transcriptionResult: string) => void;
  setSavedFilePath: (savedFilePath: string | null) => void;
  setError: (error: string | null) => void;
  setIsTranscribing: (isTranscribing: boolean) => void;
  setSelectedProfileId: (selectedProfileId: string | null) => void;
}

export const useTranscribeFileStore = create<TranscribeFileState>((set) => ({
  selectedFile: null,
  outputMode: "textarea",
  outputFormat: "text",
  overrideModelId: null,
  transcriptionResult: "",
  savedFilePath: null,
  error: null,
  isTranscribing: false,
  selectedProfileId: null,
  setSelectedFile: (selectedFile) => set({ selectedFile }),
  setOutputMode: (outputMode) => set({ outputMode }),
  setOutputFormat: (outputFormat) => set({ outputFormat }),
  setOverrideModelId: (overrideModelId) => set({ overrideModelId }),
  setTranscriptionResult: (transcriptionResult) => set({ transcriptionResult }),
  setSavedFilePath: (savedFilePath) => set({ savedFilePath }),
  setError: (error) => set({ error }),
  setIsTranscribing: (isTranscribing) => set({ isTranscribing }),
  setSelectedProfileId: (selectedProfileId) => set({ selectedProfileId }),
}));
