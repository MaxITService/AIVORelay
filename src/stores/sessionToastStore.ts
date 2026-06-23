import { create } from "zustand";

export type SessionToastLevel = "error" | "warning";

export interface SessionToastRecord {
  id: number;
  level: SessionToastLevel;
  shownAt: number;
  message: string;
  description?: string;
  actionLabel?: string;
}

type NewSessionToast = Omit<SessionToastRecord, "id" | "shownAt">;

interface SessionToastStore {
  toasts: SessionToastRecord[];
  showErrors: boolean;
  showWarnings: boolean;
  addToast: (toast: NewSessionToast) => void;
  setShowErrors: (showErrors: boolean) => void;
  setShowWarnings: (showWarnings: boolean) => void;
}

let nextToastId = 1;

// Intentionally in-memory only. This store must reset whenever the app restarts.
export const useSessionToastStore = create<SessionToastStore>((set) => ({
  toasts: [],
  showErrors: true,
  showWarnings: false,
  addToast: (toast) =>
    set((state) => ({
      toasts: [
        ...state.toasts,
        {
          ...toast,
          id: nextToastId++,
          shownAt: Date.now(),
        },
      ],
    })),
  setShowErrors: (showErrors) => set({ showErrors }),
  setShowWarnings: (showWarnings) => set({ showWarnings }),
}));
