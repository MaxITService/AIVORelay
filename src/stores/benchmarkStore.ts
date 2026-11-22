import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, VadBackend } from "@/bindings";
import {
  isElevenLabsLiveSelection,
  isGeminiLiveSelection,
  isReson8Selection,
  STT_MODEL_IDS,
  type SttModelSelection,
} from "@/lib/sttModelSelection";

export interface BenchmarkClipInfo {
  id: string;
  name: string;
  durationSeconds: number;
  sampleCount: number;
  deviceName: string;
  recordedAtMs: number;
}

export interface BenchmarkRunRequest {
  clipId: string;
  profileId: string | null;
  modelSelectionOverride: SttModelSelection | null;
  boostDbOverride: number | null;
  noiseCancellationOverride: boolean | null;
  filterSilenceOverride: boolean | null;
  vadThresholdOverride: number | null;
  vadBackendOverride: VadBackend | null;
  label: string | null;
}

export interface BenchmarkRunResult {
  id: string;
  clipId: string;
  clipName: string;
  timestampMs: number;
  label: string;
  profileName: string;
  providerLabel: string;
  modelLabel: string;
  isLive: boolean;
  text: string;
  rawText: string | null;
  llmPostProcessed: boolean;
  error: string | null;
  durationMs: number;
  clipDurationSeconds: number;
  processedDurationSeconds: number;
  settingsText: string;
}

export interface BenchmarkStateSnapshot {
  clips: BenchmarkClipInfo[];
  results: BenchmarkRunResult[];
}

export type BenchmarkTargetMode = "active" | "profile" | "custom";
export type BenchmarkResultsFilter = "selected" | "all";

export interface BenchmarkProgress {
  current: number;
  total: number;
  stepLabel: string;
}

export const BOOST_SWEEP_DB_LEVELS = [
  0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17,
] as const;

export const VAD_SWEEP_THRESHOLDS = [
  0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9,
] as const;

const BENCHMARK_CANCELLED_MESSAGE = "Benchmark run cancelled";

const isSelectionLiveForSettings = (
  settings: AppSettings | null,
  selection: SttModelSelection | null,
): boolean => {
  if (!settings) return false;
  if (!selection) {
    const provider = settings.transcription_provider;
    if (provider === "remote_soniox") {
      return (
        Boolean(settings.soniox_live_enabled) &&
        (settings.soniox_model || "").includes("-rt-")
      );
    }
    if (provider === "remote_deepgram") {
      return Boolean(settings.deepgram_live_enabled);
    }
    if (provider === "remote_openai_compatible") {
      const sel: SttModelSelection = {
        provider: "remote_openai_compatible",
        provider_preset: settings.remote_stt?.provider_preset || "",
        model_id: settings.remote_stt?.model_id || "",
      };
      return (
        isGeminiLiveSelection(sel) ||
        isReson8Selection(sel) ||
        isElevenLabsLiveSelection(sel) ||
        (sel.provider_preset === "openai" &&
          (sel.model_id || "").toLowerCase().includes("realtime") &&
          !settings.openai_realtime_whisper_flatten_enabled)
      );
    }
    return false;
  }

  if (selection.provider === "remote_soniox") {
    return (
      Boolean(settings.soniox_live_enabled) &&
      (selection.model_id === STT_MODEL_IDS.sonioxLive ||
        (selection.model_id || "").includes("-rt-"))
    );
  }
  if (selection.provider === "remote_deepgram") {
    return Boolean(settings.deepgram_live_enabled);
  }
  if (selection.provider === "remote_openai_compatible") {
    return (
      isGeminiLiveSelection(selection) ||
      isReson8Selection(selection) ||
      isElevenLabsLiveSelection(selection) ||
      (selection.provider_preset === "openai" &&
        (selection.model_id || "").toLowerCase().includes("realtime") &&
        !settings.openai_realtime_whisper_flatten_enabled)
    );
  }
  return false;
};

interface BenchmarkStoreState {
  clips: BenchmarkClipInfo[];
  selectedClipId: string | null;
  isRecording: boolean;
  recordingStartedAt: number | null;
  targetMode: BenchmarkTargetMode;
  selectedProfileId: string | null;
  customSelection: SttModelSelection | null;
  isRunning: boolean;
  cancelling: boolean;
  progress: BenchmarkProgress | null;
  results: BenchmarkRunResult[];
  resultsFilter: BenchmarkResultsFilter;
  lastError: string | null;

  syncState: () => Promise<void>;
  selectClip: (clipId: string) => void;
  startRecording: () => Promise<void>;
  stopRecording: () => Promise<void>;
  deleteClip: (clipId: string) => Promise<void>;
  setTargetMode: (mode: BenchmarkTargetMode) => void;
  setSelectedProfileId: (profileId: string | null) => void;
  setCustomSelection: (selection: SttModelSelection | null) => void;
  setResultsFilter: (filter: BenchmarkResultsFilter) => void;
  runCurrent: (label: string) => Promise<void>;
  runBoostSweep: (formatStepLabel: (db: number) => string) => Promise<void>;
  runNoiseSweep: (
    settings: AppSettings | null,
    formatLabels: {
      rnnoiseOff: string;
      rnnoiseOn: string;
      vadLevel: (threshold: number) => string;
    },
  ) => Promise<void>;
  cancelRun: () => Promise<void>;
  removeResult: (id: string) => Promise<void>;
  clearResults: (clipId: string | null) => Promise<void>;
}

const buildBaseRequest = (
  state: BenchmarkStoreState,
  clipId: string,
): BenchmarkRunRequest => ({
  clipId,
  profileId: state.targetMode === "profile" ? state.selectedProfileId : null,
  modelSelectionOverride:
    state.targetMode === "custom" ? state.customSelection : null,
  boostDbOverride: null,
  noiseCancellationOverride: null,
  filterSilenceOverride: null,
  vadThresholdOverride: null,
  vadBackendOverride: null,
  label: null,
});

const resolveEffectiveSelectionForLiveCheck = (
  settings: AppSettings | null,
  state: BenchmarkStoreState,
): SttModelSelection | null => {
  if (state.targetMode === "custom") {
    return state.customSelection;
  }
  if (!settings) return null;
  const profileId =
    state.targetMode === "profile"
      ? state.selectedProfileId ?? "default"
      : settings.active_profile_id ?? "default";
  if (profileId !== "default") {
    const profile = (settings.transcription_profiles ?? []).find(
      (p) => p.id === profileId,
    );
    if (profile?.stt_model_selection_override) {
      return profile.stt_model_selection_override;
    }
  }
  return null;
};

let cancelSweepRequested = false;

export const useBenchmarkStore = create<BenchmarkStoreState>((set, get) => ({
  clips: [],
  selectedClipId: null,
  isRecording: false,
  recordingStartedAt: null,
  targetMode: "active",
  selectedProfileId: null,
  customSelection: null,
  isRunning: false,
  cancelling: false,
  progress: null,
  results: [],
  resultsFilter: "selected",
  lastError: null,

  syncState: async () => {
    try {
      const snapshot =
        await invoke<BenchmarkStateSnapshot>("benchmark_get_state");
      set((prev) => {
        const clips = snapshot.clips ?? [];
        const results = snapshot.results ?? [];
        const selectedStillExists =
          prev.selectedClipId !== null &&
          clips.some((clip) => clip.id === prev.selectedClipId);
        const nextSelectedClipId = selectedStillExists
          ? prev.selectedClipId
          : (clips[0]?.id ?? null);
        const nextFilter: BenchmarkResultsFilter =
          clips.length === 0 && results.length > 0 ? "all" : prev.resultsFilter;
        return {
          clips,
          results,
          selectedClipId: nextSelectedClipId,
          resultsFilter: nextFilter,
        };
      });
    } catch (error) {
      console.error("Failed to sync benchmark state:", error);
    }
  },

  selectClip: (clipId) =>
    set((prev) =>
      prev.clips.some((clip) => clip.id === clipId)
        ? { selectedClipId: clipId }
        : prev,
    ),

  startRecording: async () => {
    if (get().isRecording || get().isRunning) return;
    set({ lastError: null });
    try {
      await invoke("benchmark_start_recording");
      set({ isRecording: true, recordingStartedAt: Date.now() });
    } catch (error) {
      set({
        isRecording: false,
        recordingStartedAt: null,
        lastError: error instanceof Error ? error.message : String(error),
      });
    }
  },

  stopRecording: async () => {
    if (!get().isRecording) return;
    try {
      const clip = await invoke<BenchmarkClipInfo>("benchmark_stop_recording");
      set((prev) => ({
        clips: [clip, ...prev.clips],
        selectedClipId: clip.id,
        isRecording: false,
        recordingStartedAt: null,
        lastError: null,
      }));
    } catch (error) {
      set({
        isRecording: false,
        recordingStartedAt: null,
        lastError: error instanceof Error ? error.message : String(error),
      });
    }
  },

  deleteClip: async (clipId) => {
    if (get().isRecording || get().isRunning) return;
    try {
      await invoke("benchmark_delete_clip", { clipId });
      set((prev) => {
        const nextClips = prev.clips.filter((clip) => clip.id !== clipId);
        const nextSelected =
          prev.selectedClipId === clipId
            ? (nextClips[0]?.id ?? null)
            : prev.selectedClipId;
        // Keep existing results accessible when the last clip is deleted.
        const nextFilter: BenchmarkResultsFilter =
          nextClips.length === 0 && prev.results.length > 0
            ? "all"
            : prev.resultsFilter;
        return {
          clips: nextClips,
          selectedClipId: nextSelected,
          resultsFilter: nextFilter,
          lastError: null,
        };
      });
    } catch (error) {
      set({
        lastError: error instanceof Error ? error.message : String(error),
      });
    }
  },

  setTargetMode: (targetMode) => set({ targetMode }),
  setSelectedProfileId: (selectedProfileId) => set({ selectedProfileId }),
  setCustomSelection: (customSelection) => set({ customSelection }),
  setResultsFilter: (resultsFilter) => set({ resultsFilter }),

  runCurrent: async (label) => {
    const state = get();
    const clipId = state.selectedClipId;
    if (!clipId || state.isRecording || state.isRunning) return;
    cancelSweepRequested = false;
    set({
      isRunning: true,
      cancelling: false,
      lastError: null,
      progress: { current: 1, total: 1, stepLabel: label },
    });
    try {
      const request: BenchmarkRunRequest = {
        ...buildBaseRequest(state, clipId),
        label,
      };
      const result = await invoke<BenchmarkRunResult>("benchmark_run_single", {
        request,
      });
      set((prev) => ({ results: [result, ...prev.results] }));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (message !== BENCHMARK_CANCELLED_MESSAGE) {
        set({ lastError: message });
      }
    } finally {
      set({ isRunning: false, cancelling: false, progress: null });
    }
  },

  runBoostSweep: async (formatStepLabel) => {
    const state = get();
    const clipId = state.selectedClipId;
    if (!clipId || state.isRecording || state.isRunning) return;
    const base = buildBaseRequest(state, clipId);
    const total = BOOST_SWEEP_DB_LEVELS.length;
    cancelSweepRequested = false;
    set({ isRunning: true, cancelling: false, lastError: null });

    try {
      for (let i = 0; i < total; i++) {
        if (cancelSweepRequested) break;
        const db = BOOST_SWEEP_DB_LEVELS[i];
        const stepLabel = formatStepLabel(db);
        set({ progress: { current: i + 1, total, stepLabel } });
        const result = await invoke<BenchmarkRunResult>(
          "benchmark_run_single",
          {
            request: {
              ...base,
              boostDbOverride: db,
              label: stepLabel,
            },
          },
        );
        set((prev) => ({ results: [result, ...prev.results] }));
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (message !== BENCHMARK_CANCELLED_MESSAGE) {
        set({ lastError: message });
      }
    } finally {
      set({ isRunning: false, cancelling: false, progress: null });
    }
  },

  runNoiseSweep: async (settings, formatLabels) => {
    const state = get();
    const clipId = state.selectedClipId;
    if (!clipId || state.isRecording || state.isRunning) return;
    const base = buildBaseRequest(state, clipId);
    const effectiveSelection = resolveEffectiveSelectionForLiveCheck(
      settings,
      state,
    );
    const isLive = isSelectionLiveForSettings(settings, effectiveSelection);

    const steps: BenchmarkRunRequest[] = isLive
      ? [
          {
            ...base,
            noiseCancellationOverride: false,
            filterSilenceOverride: false,
            label: formatLabels.rnnoiseOff,
          },
          {
            ...base,
            noiseCancellationOverride: true,
            filterSilenceOverride: false,
            label: formatLabels.rnnoiseOn,
          },
        ]
      : [
          {
            ...base,
            noiseCancellationOverride: false,
            filterSilenceOverride: false,
            label: formatLabels.rnnoiseOff,
          },
          {
            ...base,
            noiseCancellationOverride: true,
            filterSilenceOverride: false,
            label: formatLabels.rnnoiseOn,
          },
          ...VAD_SWEEP_THRESHOLDS.map((threshold) => ({
            ...base,
            filterSilenceOverride: true,
            vadThresholdOverride: threshold,
            label: formatLabels.vadLevel(threshold),
          })),
        ];

    const total = steps.length;
    cancelSweepRequested = false;
    set({ isRunning: true, cancelling: false, lastError: null });

    try {
      for (let i = 0; i < total; i++) {
        if (cancelSweepRequested) break;
        const step = steps[i];
        set({
          progress: {
            current: i + 1,
            total,
            stepLabel: step.label ?? "",
          },
        });
        const result = await invoke<BenchmarkRunResult>(
          "benchmark_run_single",
          { request: step },
        );
        set((prev) => ({ results: [result, ...prev.results] }));
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (message !== BENCHMARK_CANCELLED_MESSAGE) {
        set({ lastError: message });
      }
    } finally {
      set({ isRunning: false, cancelling: false, progress: null });
    }
  },

  cancelRun: async () => {
    if (!get().isRunning) return;
    cancelSweepRequested = true;
    set({ cancelling: true });
    try {
      await invoke("benchmark_cancel_run");
    } catch (error) {
      console.error("Failed to cancel benchmark run:", error);
    }
  },

  removeResult: async (id) => {
    try {
      await invoke("benchmark_delete_result", { resultId: id });
      set((prev) => ({
        results: prev.results.filter((item) => item.id !== id),
      }));
    } catch (error) {
      set({
        lastError: error instanceof Error ? error.message : String(error),
      });
    }
  },

  clearResults: async (clipId) => {
    try {
      await invoke("benchmark_clear_results", { clipId });
      set((prev) => ({
        results: clipId
          ? prev.results.filter((item) => item.clipId !== clipId)
          : [],
      }));
    } catch (error) {
      set({
        lastError: error instanceof Error ? error.message : String(error),
      });
    }
  },
}));
