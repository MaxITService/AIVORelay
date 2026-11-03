import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import i18n from "@/i18n";
import { useActiveNoticeStore, type NoticeSnapshot } from "@/stores/activeNoticeStore";
import { useSessionToastStore } from "@/stores/sessionToastStore";

// Listen before fetching; revisions prevent an older fetch from replacing a new event.
export async function connectActiveNotices(): Promise<UnlistenFn> {
  const stops: UnlistenFn[] = [];
  try {
    stops.push(await listen<NoticeSnapshot>("active-notices-changed", ({ payload }) => {
      const accepted = useActiveNoticeStore.getState().apply(payload);
      const notice = payload.raised;
      if (accepted && notice && !notice.source.startsWith("ui:")) {
        useSessionToastStore.getState().addToast({ level: notice.level,
          message: notice.title_key ? i18n.t(notice.title_key, { defaultValue: notice.title }) : notice.title,
          description: notice.detail ?? undefined });
      }
    }));
    const refresh = async () => {
      const snapshot = await invoke<NoticeSnapshot>("get_active_notices");
      useActiveNoticeStore.getState().apply(snapshot);
      if (snapshot.open_requested) useActiveNoticeStore.getState().setOpen(true);
    };
    stops.push(await listen("open-active-notices", () => {
      useActiveNoticeStore.getState().setOpen(true);
      void refresh().catch(error => console.warn("Failed to refresh active notices:", error));
    }));
    await refresh();
    return () => stops.forEach(stop => stop());
  } catch (error) {
    stops.forEach(stop => stop());
    throw error;
  }
}

// Serialize frontend publications: a slow command cannot resurrect an older
// update of the same toast after a newer one was shown.
let publications = Promise.resolve();
export function publishUiNotice(source: string, level: "error" | "warning", title: string, detail?: string) {
  publications = publications.then(async () => {
    await invoke("publish_ui_notice", { source, level, title, detail: detail ?? null });
  }).catch(error => console.warn("Failed to retain active notice:", error));
}
