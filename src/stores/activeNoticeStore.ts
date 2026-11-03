import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { MouseEvent } from "react";

export interface ActiveNotice {
  id: number;
  source: string;
  level: "error" | "warning";
  title: string;
  title_key: string | null;
  detail: string | null;
  target: string | null;
  generation: number;
}
export interface NoticeSnapshot { revision: number; notices: ActiveNotice[]; raised: ActiveNotice | null; open_requested?: boolean }
export interface NoticeAction { label: string; onClick: (event: MouseEvent<HTMLButtonElement>) => void }
interface State {
  revision: number;
  notices: ActiveNotice[];
  open: boolean;
  actions: Record<string, NoticeAction | undefined>;
  apply: (snapshot: NoticeSnapshot) => boolean;
  setOpen: (open: boolean) => void;
  setAction: (source: string, action?: NoticeAction) => void;
  dismiss: (id: number) => Promise<void>;
}
export const useActiveNoticeStore = create<State>((set, get) => ({
  revision: -1, notices: [], open: false, actions: {},
  apply: snapshot => {
    if (snapshot.revision <= get().revision) return false;
    set({ revision: snapshot.revision, notices: snapshot.notices });
    return true;
  },
  setOpen: open => set({ open }),
  setAction: (source, action) => set(state => ({ actions: Object.fromEntries(
    [...Object.entries(state.actions).filter(([key]) => key !== source), [source, action] as const].slice(-100)
  ) })),
  dismiss: async id => {
    await invoke("dismiss_active_notice", { id });
    // Also reconcile when the main window missed the event during recreation.
    get().apply(await invoke<NoticeSnapshot>("get_active_notices"));
  },
}));
