import React, { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { AlertTriangle, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { connectActiveNotices } from "@/lib/activeNotices";
import { useActiveNoticeStore } from "@/stores/activeNoticeStore";
import { navigateToSettingsAnchor } from "@/lib/anchorNavigation";
import { useNavigationStore } from "@/stores/navigationStore";
import { useSessionToastStore } from "@/stores/sessionToastStore";

export const ActiveNoticeConnection: React.FC = () => {
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    void connectActiveNotices().then(unlisten => {
      if (disposed) unlisten(); else stop = unlisten;
    }).catch(error => console.warn("Failed to connect active notices:", error));
    return () => { disposed = true; stop?.(); };
  }, []);
  return null;
};

export const SessionNoticeChip: React.FC = () => {
  const { t } = useTranslation();
  const { notices, open, setOpen, dismiss, actions } = useActiveNoticeStore();
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const close = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!open) return;
    close.current?.focus();
    const key = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.isComposing || event.defaultPrevented) return;
      event.preventDefault(); event.stopPropagation(); setOpen(false); trigger.current?.focus();
    };
    const outside = (event: MouseEvent) => {
      if (!root.current?.contains(event.target as Node) && !trigger.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("keydown", key);
    document.addEventListener("mousedown", outside);
    return () => { document.removeEventListener("keydown", key); document.removeEventListener("mousedown", outside); };
  }, [open, setOpen]);
  const remove = (id: number) => { void dismiss(id).catch(error => console.warn("Failed to dismiss notice:", error)); };
  const history = () => {
    const state = useSessionToastStore.getState();
    state.setShowErrors(true); state.setShowWarnings(true); setOpen(false);
    navigateToSettingsAnchor({ activateSection: () => useNavigationStore.getState().setSection("debug"), targetId: "settings-session-toast-history" });
  };
  const error = notices.some(notice => notice.level === "error");
  if (!notices.length && !open) return null;
  return <>
    <span className="sr-only" role="status" aria-live="polite" aria-atomic="true">{notices.length ? t("notices.active", { count: notices.length }) : ""}</span>
    <button ref={trigger} type="button" aria-expanded={open} aria-controls="active-notices-panel"
      onClick={() => setOpen(!open)} title={t("notices.active", { count: notices.length })}
      className={`inline-flex shrink-0 items-center gap-1 rounded border px-2 py-1 text-[10px] ${error ? "border-red-500/30 text-red-400" : notices.length ? "border-amber-500/30 text-amber-300" : "border-mid-gray/30 text-mid-gray"}`}>
      <AlertTriangle className="h-3 w-3" aria-hidden="true" />{t("notices.active", { count: notices.length })}
    </button>
    {open && createPortal(<div ref={root} id="active-notices-panel" role="region" aria-label={t("notices.title")}
      className="fixed bottom-14 right-4 z-[80] max-h-[65vh] w-[min(420px,calc(100vw-32px))] overflow-y-auto rounded-lg border border-mid-gray/30 bg-[#1a1a1a] p-3 text-sm text-text shadow-xl">
      <div className="mb-2 flex items-center justify-between gap-2"><strong>{t("notices.title")}</strong>
        <button ref={close} type="button" onClick={() => { setOpen(false); trigger.current?.focus(); }} aria-label={t("common.close")}><X className="h-4 w-4" /></button></div>
      {!notices.length && <p className="text-mid-gray">{t("notices.empty")}</p>}
      {notices.map(notice => <div key={notice.id} className="flex gap-2 border-t border-mid-gray/20 py-3">
        <AlertTriangle className={`mt-1 h-4 w-4 shrink-0 ${notice.level === "error" ? "text-red-400" : "text-amber-300"}`} aria-hidden="true" />
        <div className="min-w-0 flex-1 select-text break-words"><p className="font-medium">{notice.title_key ? t(notice.title_key, { defaultValue: notice.title }) : notice.title}</p>
          {notice.detail && <p className="mt-1 whitespace-pre-wrap text-xs text-mid-gray">{notice.detail}</p>}
          {actions[notice.source] && <button type="button" className="mt-2 underline" onClick={event => {
            if (!useActiveNoticeStore.getState().notices.some(n => n.id === notice.id)) return;
            actions[notice.source]?.onClick(event); remove(notice.id);
          }}>{actions[notice.source]?.label}</button>}
          {notice.target && <button type="button" className="mt-2 underline" onClick={() => {
            const section = notice.target === "models" ? "models" : notice.target === "transcribe" ? "transcribeFile" : "general";
            useNavigationStore.getState().setSection(section); setOpen(false);
          }}>{t("notices.openSettings")}</button>}
        </div>
        <button type="button" className="self-start p-1" aria-label={t("notices.dismiss")} onClick={() => remove(notice.id)}><X className="h-4 w-4" /></button>
      </div>)}
      <button type="button" className="mt-2 text-xs underline" onClick={history}>{t("notices.history")}</button>
      <p className="mt-2 text-xs text-mid-gray">{t("notices.dismissHelp")}</p>
    </div>, document.body)}
  </>;
};
