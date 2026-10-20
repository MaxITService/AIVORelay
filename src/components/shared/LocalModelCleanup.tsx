import React, { useEffect, useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { HardDrive, Loader2, RefreshCw, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "../ui/Button";
import {
  cleanSelectedLocalModels, formatStorageBytes, getLocalModelStorage,
  modelStorageKey, removedModelBytes,
  type LocalModelStorageSnapshot, type ModelCleanupOutcome,
} from "@/lib/localModelCleanup";

export const LocalModelCleanup: React.FC<{ onCleaned?: () => void }> = ({ onCleaned }) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  return <>
    <Button type="button" variant="ghost" size="sm" className="inline-flex items-center gap-1.5"
      onClick={() => setOpen(true)}>
      <HardDrive className="h-3.5 w-3.5" aria-hidden="true" />
      {t("modelCleanup.open")}
    </Button>
    {open && createPortal(<LocalModelCleanupDialog onClose={() => setOpen(false)} onCleaned={onCleaned} />, document.body)}
  </>;
};

export const LocalModelCleanupDialog: React.FC<{ onClose: () => void; onCleaned?: () => void }> = ({ onClose, onCleaned }) => {
  const { t } = useTranslation();
  const titleId = useId();
  const descriptionId = useId();
  const dialogRef = useRef<HTMLDivElement>(null);
  const selectAllRef = useRef<HTMLInputElement>(null);
  const busyRef = useRef(false);
  const mounted = useRef(true);
  const [snapshot, setSnapshot] = useState<LocalModelStorageSnapshot | null>(null);
  const [after, setAfter] = useState<LocalModelStorageSnapshot | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [outcomes, setOutcomes] = useState<Record<string, ModelCleanupOutcome>>({});
  const [activeKey, setActiveKey] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  const refresh = async () => {
    setLoading(true);
    setSnapshot(null);
    setError(null);
    setConfirming(false);
    try {
      const current = await getLocalModelStorage();
      if (!mounted.current) return;
      setSnapshot(current);
      setAfter(null);
      setOutcomes({});
      setSelected(new Set(current.entries.filter(entry => !entry.blocked_reason).map(modelStorageKey)));
    } catch (failure) {
      if (mounted.current) setError(String(failure));
    } finally {
      if (mounted.current) setLoading(false);
    }
  };

  useEffect(() => {
    mounted.current = true;
    void refresh();
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const timer = window.setTimeout(() => dialogRef.current?.focus(), 0);
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        if (!busyRef.current) onCloseRef.current();
      } else if (event.key === "Tab" && dialogRef.current) {
        const focusable = Array.from(dialogRef.current.querySelectorAll<HTMLElement>("button:not(:disabled), input:not(:disabled)"));
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        if (!first) { event.preventDefault(); dialogRef.current.focus(); }
        else if (event.shiftKey && (document.activeElement === first || !focusable.includes(document.activeElement as HTMLElement))) {
          event.preventDefault(); last.focus();
        } else if (!event.shiftKey && (document.activeElement === last || !focusable.includes(document.activeElement as HTMLElement))) {
          event.preventDefault(); first.focus();
        }
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      mounted.current = false;
      window.clearTimeout(timer);
      window.removeEventListener("keydown", handleKeyDown);
      previous?.focus();
    };
  }, []);

  const eligible = snapshot?.entries.filter(entry => !entry.blocked_reason && !outcomes[modelStorageKey(entry)]) ?? [];
  const selectedEntries = eligible.filter(entry => selected.has(modelStorageKey(entry)));
  const selectedBytes = selectedEntries.reduce((sum, entry) => sum + entry.bytes, 0);
  const allSelected = eligible.length > 0 && selectedEntries.length === eligible.length;
  useEffect(() => {
    if (selectAllRef.current) selectAllRef.current.indeterminate = selectedEntries.length > 0 && !allSelected;
  }, [allSelected, selectedEntries.length]);

  const clean = async () => {
    if (busyRef.current || !snapshot || selectedEntries.length === 0) return;
    if (!confirming) { setConfirming(true); return; }
    busyRef.current = true;
    setBusy(true);
    setError(null);
    setConfirming(false);
    try {
      await cleanSelectedLocalModels(selectedEntries, undefined, (key, outcome) => {
        if (!mounted.current) return;
        setActiveKey(outcome ? null : key);
        if (outcome) setOutcomes(current => ({ ...current, [key]: outcome }));
      });
      const current = await getLocalModelStorage();
      if (mounted.current) setAfter(current);
    } catch (failure) {
      if (mounted.current) setError(String(failure));
    } finally {
      busyRef.current = false;
      if (mounted.current) { setBusy(false); setActiveKey(null); }
      onCleaned?.();
    }
  };

  const attempted = new Set(Object.keys(outcomes));
  const removed = snapshot && after ? removedModelBytes(snapshot, after, attempted) : null;
  const shared = snapshot?.entries.some(entry => entry.shared_cache_bytes > 0);
  return <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/65 p-4"
    onClick={() => { if (!busyRef.current) onClose(); }}>
    <div ref={dialogRef} role="dialog" aria-modal="true" aria-labelledby={titleId} aria-describedby={descriptionId}
      aria-busy={busy || loading} tabIndex={-1} onClick={event => event.stopPropagation()}
      className="w-full max-w-xl rounded-xl border border-mid-gray/30 bg-[#171717] text-text shadow-2xl outline-none">
      <div className="flex items-center justify-between border-b border-mid-gray/20 px-4 py-3">
        <h2 id={titleId} className="text-sm font-semibold">{t("modelCleanup.title")}</h2>
        <div className="flex items-center gap-2">
          <button type="button" disabled={busy || loading} onClick={() => void refresh()} aria-label={t("modelCleanup.refresh")}
            className="rounded p-1 text-text/60 hover:text-text disabled:opacity-40"><RefreshCw className="h-4 w-4" /></button>
          <button type="button" disabled={busy} onClick={onClose} aria-label={t("common.close", "Close")}
            className="rounded p-1 text-text/60 hover:text-text disabled:opacity-40"><X className="h-4 w-4" /></button>
        </div>
      </div>
      <p id={descriptionId} className="px-4 py-2 text-xs text-text/60">{t("modelCleanup.description")}</p>
      {loading ? <div role="status" className="flex items-center justify-center gap-2 p-6 text-xs"><Loader2 className="h-4 w-4 animate-spin" />{t("common.loading")}</div> :
        <div className="max-h-[45vh] overflow-auto border-y border-mid-gray/20">
          <table className="w-full text-xs">
            <thead className="sticky top-0 bg-[#222] text-text/60">
              <tr>
                <th className="w-8 px-3 py-1.5"><input ref={selectAllRef} type="checkbox" checked={allSelected}
                  disabled={busy || eligible.length === 0} aria-label={t("modelCleanup.selectAll")}
                  onChange={() => { setSelected(new Set(allSelected ? [] : eligible.map(modelStorageKey))); setConfirming(false); }} /></th>
                <th className="py-1.5 pr-2 text-left font-normal">{t("modelCleanup.model")}</th>
                <th className="w-12 py-1.5 text-left font-normal">{t("modelCleanup.type")}</th>
                <th className="px-3 py-1.5 text-right font-normal">{t("modelCleanup.size")}</th>
              </tr>
            </thead>
            <tbody>
              {snapshot?.entries.map(entry => {
                const key = modelStorageKey(entry);
                const outcome = outcomes[key];
                const message = outcome?.error ?? entry.blocked_reason;
                return <tr key={key} className="border-t border-mid-gray/15 hover:bg-white/[0.025]">
                  <td className="px-3 py-2 align-top"><input type="checkbox" aria-label={entry.name}
                    checked={!outcome && selected.has(key)} disabled={busy || !!entry.blocked_reason || !!outcome}
                    onChange={() => { setSelected(current => { const next = new Set(current); if (next.has(key)) next.delete(key); else next.add(key); return next; }); setConfirming(false); }} /></td>
                  <td className="py-1.5 pr-2">
                    <span className={outcome && !outcome.error ? "text-text/45 line-through" : ""}>{entry.name}</span>
                    {activeKey === key && <span role="status" className="ml-2 text-text/60">{t("modelCleanup.deleting")}</span>}
                    {outcome && !outcome.error && <span className="ml-2 text-emerald-400">{t("modelCleanup.deleted")}</span>}
                    {message && <div role={outcome?.error ? "alert" : undefined} className="mt-0.5 break-words text-[11px] text-amber-300/90">{message}</div>}
                    {entry.shared_cache_bytes > 0 && <div className="text-[10px] text-text/45">{t("modelCleanup.shared", { size: formatStorageBytes(entry.shared_cache_bytes) })}</div>}
                  </td>
                  <td className="py-2 align-top text-[10px] text-text/50">{entry.kind.toUpperCase()}</td>
                  <td className="whitespace-nowrap px-3 py-2 text-right align-top tabular-nums text-text/75">{formatStorageBytes(entry.bytes)}</td>
                </tr>;
              })}
              {snapshot?.entries.length === 0 && <tr><td colSpan={4} className="p-6 text-center text-text/60">{t("modelCleanup.empty")}</td></tr>}
            </tbody>
          </table>
        </div>}
      <div aria-live="polite" className="space-y-1 px-4 py-3 text-xs text-text/65">
        {snapshot && <div className="flex flex-wrap justify-between gap-2 tabular-nums">
          <span>{t("modelCleanup.before", { size: formatStorageBytes(snapshot.total_bytes) })}</span>
          {after ? <><span className="text-emerald-400">{t("modelCleanup.removed", { size: formatStorageBytes(removed ?? 0) })}</span><span>{t("modelCleanup.after", { size: formatStorageBytes(after.total_bytes) })}</span></> :
            <span>{t("modelCleanup.selected", { count: selectedEntries.length, size: formatStorageBytes(selectedBytes) })}</span>}
        </div>}
        {snapshot?.disk_free_bytes != null && <div className="text-[11px] text-text/45">{t("modelCleanup.diskFree", { size: formatStorageBytes(snapshot.disk_free_bytes) })}{after?.disk_free_bytes != null && ` → ${formatStorageBytes(after.disk_free_bytes)}`}</div>}
        {shared && <p className="text-[11px] text-text/45">{t("modelCleanup.sharedNote")}</p>}
        {error && <p role="alert" className="break-words text-red-400">{error}</p>}
        {confirming && <p role="alert" className="pt-1 text-amber-300">{t("modelCleanup.confirm", { count: selectedEntries.length })}</p>}
      </div>
      <div className="flex justify-end gap-2 border-t border-mid-gray/20 px-4 py-3">
        <Button type="button" variant="ghost" size="sm" disabled={busy} onClick={() => { if (confirming) setConfirming(false); else onClose(); }}>
          {confirming ? t("common.cancel") : t("common.close", "Close")}
        </Button>
        <Button type="button" variant="danger" size="sm" disabled={loading || busy || selectedEntries.length === 0} onClick={() => void clean()}>
          {busy ? t("modelCleanup.deleting") : confirming ? t("modelCleanup.confirmButton") : allSelected ? t("modelCleanup.deleteAll") : t("modelCleanup.deleteSelected")}
        </Button>
      </div>
    </div>
  </div>;
};
