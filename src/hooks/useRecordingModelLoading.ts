import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface RecordingModelLoadingState {
  recording_session_id: number | null;
  provider: string | null;
  model_id: string | null;
  loading: boolean;
  loading_elapsed_ms: number;
  preview_active: boolean;
}

const EMPTY_STATE: RecordingModelLoadingState = {
  recording_session_id: null,
  provider: null,
  model_id: null,
  loading: false,
  loading_elapsed_ms: 0,
  preview_active: false,
};

/// The backend scopes loading to the recording's captured provider/model.
/// Query after subscribing so recovered windows and cold-load retries hydrate
/// without relying on a loading_started event they may have missed.
export function subscribeRecordingModelLoading(options: {
  readState: () => Promise<RecordingModelLoadingState>;
  subscribe: (name: string, handler: (state?: RecordingModelLoadingState) => void) => Promise<() => void>;
  setState: (state: RecordingModelLoadingState) => void;
  setSlow: (slow: boolean) => void;
  schedule?: (callback: () => void, delay: number) => ReturnType<typeof setTimeout>;
  cancel?: (timer: ReturnType<typeof setTimeout> | undefined) => void;
}) {
    const { setState, setSlow } = options;
    const schedule = options.schedule ?? setTimeout;
    const cancel = options.cancel ?? clearTimeout;
    let active = true;
    let revision = 0;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const unlisteners: Array<() => void> = [];
    const apply = (payload: RecordingModelLoadingState) => {
      if (!active) return;
      cancel(timer);
      setState(payload);
      const loading = payload.loading && payload.recording_session_id !== null;
      const elapsed = Math.max(0, payload.loading_elapsed_ms);
      setSlow(loading && elapsed >= 2000);
      if (loading && elapsed < 2000) {
        timer = schedule(() => {
          if (active) setSlow(true);
        }, 2000 - elapsed);
      }
    };
    const refresh = async () => {
      if (!active) return;
      const requestedRevision = ++revision;
      try {
        const payload = await options.readState();
        if (active && requestedRevision === revision) apply(payload);
      } catch {
        // Keep the last authoritative state during transient renderer recovery.
      }
    };
    const subscribe = async (name: string, handler: () => void) => {
      const unlisten = await options.subscribe(name, handler);
      if (active) unlisteners.push(unlisten);
      else unlisten();
    };
    const setup = async () => {
      const unlisten = await options.subscribe(
        "recording-model-loading-state",
        (payload) => {
          if (!payload) return;
          ++revision;
          apply(payload);
        },
      );
      if (!active) {
        unlisten();
        return;
      }
      unlisteners.push(unlisten);
      for (const name of [
        "show-overlay",
        "soniox-live-preview-reset",
        "soniox_live_preview_reset",
        "preview-output-mode-state",
        "preview_output_mode_state",
      ]) {
        if (!active) return;
        try {
          await subscribe(name, () => void refresh());
        } catch {
          // A missing auxiliary subscription must not prevent hydration.
        }
      }
      await refresh();
    };
    void setup().catch(() => { if (active) void refresh(); });
    return () => {
      active = false;
      ++revision;
      cancel(timer);
      for (const unlisten of unlisteners) unlisten();
    };
}

export function useRecordingModelLoading() {
  const [state, setState] = useState(EMPTY_STATE);
  const [slow, setSlow] = useState(false);
  useEffect(() => subscribeRecordingModelLoading({
    readState: () => invoke<RecordingModelLoadingState>("get_recording_model_loading_state"),
    subscribe: (name, handler) => listen<RecordingModelLoadingState>(name, (event) => handler(event.payload)),
    setState,
    setSlow,
  }), []);

  return {
    loading: state.loading && state.recording_session_id !== null,
    slow,
    previewActive: state.preview_active,
    sessionId: state.recording_session_id,
  };
}
