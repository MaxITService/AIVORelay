import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import i18n from "@/i18n";
import { toast } from "sonner";
import { useNavigationStore } from "@/stores/navigationStore";
import { useActiveNoticeStore, type NoticeSnapshot, type ActiveNotice } from "@/stores/activeNoticeStore";
import { useSessionToastStore } from "@/stores/sessionToastStore";

// Startup repairs can happen before the webview subscribes. Show their retained
// notices once whether they arrive through the initial snapshot or a live event.
const shownShortcutRepairs = new Set<number>();
function showShortcutRepair(notice: ActiveNotice): boolean {
  if (!notice.source.startsWith("shortcut-migration:") || shownShortcutRepairs.has(notice.id)) return false;
  shownShortcutRepairs.add(notice.id);
  const message = i18n.t("notices.duplicateShortcutRemoved");
  toast.warning(message, { description: notice.detail ?? undefined, duration: 15000,
    action: { label: i18n.t("notices.openSettings"), onClick: () => useNavigationStore.getState().setSection("general") } });
  useSessionToastStore.getState().addToast({ level: "warning", message, description: notice.detail ?? undefined });
  return true;
}

// Listen before fetching; revisions prevent an older fetch from replacing a new event.
export async function connectActiveNotices(): Promise<UnlistenFn> {
  const stops: UnlistenFn[] = [];
  try {
    stops.push(await listen<NoticeSnapshot>("active-notices-changed", ({ payload }) => {
      const accepted = useActiveNoticeStore.getState().apply(payload);
      const notice = payload.raised;
      if (accepted && notice && !notice.source.startsWith("ui:") && !showShortcutRepair(notice)) {
        useSessionToastStore.getState().addToast({ level: notice.level,
          message: notice.title_key ? i18n.t(notice.title_key, { defaultValue: notice.title }) : notice.title,
          description: notice.detail ?? undefined });
      }
    }));
    const refresh = async () => {
      const snapshot = await invoke<NoticeSnapshot>("get_active_notices");
      useActiveNoticeStore.getState().apply(snapshot);
      snapshot.notices.forEach(showShortcutRepair);
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
