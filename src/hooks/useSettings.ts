import { useEffect } from "react";
import { useSettingsStore } from "../stores/settingsStore";
import type { AppSettings as Settings, AudioDevice } from "@/bindings";

interface UseSettingsReturn {
  // State
  settings: Settings | null;
  isLoading: boolean;
  isUpdating: (key: string) => boolean;
  audioDevices: AudioDevice[];
  outputDevices: AudioDevice[];
  audioFeedbackEnabled: boolean;
  postProcessModelOptions: Record<string, string[]>;

  // Actions
  updateSetting: <K extends keyof Settings>(
    key: K,
    value: Settings[K],
  ) => Promise<void>;
  resetSetting: (key: keyof Settings) => Promise<void>;
  refreshSettings: () => Promise<void>;
  refreshAudioDevices: () => Promise<void>;
  refreshOutputDevices: () => Promise<void>;

  // Binding-specific actions
  updateBinding: (id: string, binding: string) => Promise<void>;
  resetBinding: (id: string) => Promise<void>;

  // Convenience getters
  getSetting: <K extends keyof Settings>(key: K) => Settings[K] | undefined;

  // Post-processing helpers
  setPostProcessProvider: (providerId: string) => Promise<void>;
  updatePostProcessBaseUrl: (
    providerId: string,
    baseUrl: string,
  ) => Promise<void>;
  updatePostProcessApiKey: (
    providerId: string,
    apiKey: string,
  ) => Promise<void>;
  updatePostProcessModel: (providerId: string, model: string) => Promise<void>;
  fetchPostProcessModels: (providerId: string) => Promise<string[]>;
  fetchLlmModels: (feature: "post_processing" | "ai_replace") => Promise<string[]>;
  setTranscriptionProvider: (providerId: string) => Promise<void>;
  updateRemoteSttBaseUrl: (baseUrl: string) => Promise<void>;
  updateRemoteSttModelId: (modelId: string) => Promise<void>;
  updateRemoteSttDebugCapture: (enabled: boolean) => Promise<void>;
  updateRemoteSttDebugMode: (mode: string) => Promise<void>;
  setAiReplaceProvider: (providerId: string | null) => Promise<void>;
  updateAiReplaceApiKey: (
    providerId: string,
    apiKey: string,
  ) => Promise<void>;
  updateAiReplaceModel: (providerId: string, model: string) => Promise<void>;
}

export const useSettings = (): UseSettingsReturn => {
  const store = useSettingsStore();

  // Initialize on first mount
  useEffect(() => {
    if (store.isLoading) {
      store.initialize();
    }
  }, [store.initialize, store.isLoading]);

  return {
    settings: store.settings,
    isLoading: store.isLoading,
    isUpdating: store.isUpdatingKey,
    audioDevices: store.audioDevices,
    outputDevices: store.outputDevices,
    audioFeedbackEnabled: store.settings?.audio_feedback || false,
    postProcessModelOptions: store.postProcessModelOptions,
    updateSetting: store.updateSetting,
    resetSetting: store.resetSetting,
    refreshSettings: store.refreshSettings,
    refreshAudioDevices: store.refreshAudioDevices,
    refreshOutputDevices: store.refreshOutputDevices,
    updateBinding: store.updateBinding,
    resetBinding: store.resetBinding,
    getSetting: store.getSetting,
    setPostProcessProvider: store.setPostProcessProvider,
    updatePostProcessBaseUrl: store.updatePostProcessBaseUrl,
    updatePostProcessApiKey: store.updatePostProcessApiKey,
    updatePostProcessModel: store.updatePostProcessModel,
    fetchPostProcessModels: store.fetchPostProcessModels,
    fetchLlmModels: store.fetchLlmModels,
    setTranscriptionProvider: store.setTranscriptionProvider,
    updateRemoteSttBaseUrl: store.updateRemoteSttBaseUrl,
    updateRemoteSttModelId: store.updateRemoteSttModelId,
    updateRemoteSttDebugCapture: store.updateRemoteSttDebugCapture,
    updateRemoteSttDebugMode: store.updateRemoteSttDebugMode,
    setAiReplaceProvider: store.setAiReplaceProvider,
    updateAiReplaceApiKey: store.updateAiReplaceApiKey,
    updateAiReplaceModel: store.updateAiReplaceModel,
  };
};
