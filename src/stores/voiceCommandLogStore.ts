import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import type { VoiceCommandResultPayload } from "@/command-confirm/CommandConfirmOverlay";

interface LogEntry extends VoiceCommandResultPayload {
  id: string;
}

export const useVoiceCommandLogStore = create<{
  entries: LogEntry[];
  clear: () => void;
}>((set) => ({
  entries: [],
  clear: () => set({ entries: [] }),
}));

export const listenForVoiceCommandResults = () =>
  listen<VoiceCommandResultPayload>("voice-command-result", ({ payload }) => {
    const entry = { ...payload, id: crypto.randomUUID() };
    useVoiceCommandLogStore.setState((state) => ({
      entries: [...state.entries, entry].slice(-100),
    }));
  });
