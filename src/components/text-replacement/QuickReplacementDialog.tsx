import React, { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { useSettingsStore } from "@/stores/settingsStore";
import { useNavigationStore } from "@/stores/navigationStore";

interface QuickReplacementDialogProps {
  open: boolean;
  onClose: () => void;
  initialFrom?: string;
  selectionTooLong?: boolean;
}

// Literal rules still interpret these escapes in the backend.
const decodeLiteral = (value: string): string =>
  value.replace(/\\(u\{[^}]*\}|u\{[^}]*$|[nrt\\])/g, (match, escape: string) => {
    if (escape === "n") return "\n";
    if (escape === "r") return "\r";
    if (escape === "t") return "\t";
    if (escape === "\\") return "\\";
    const hex = escape.slice(2, -1);
    if (!escape.endsWith("}") || !/^[0-9a-f]{1,6}$/i.test(hex)) return match;
    const code = parseInt(hex, 16);
    return code <= 0x10ffff && !(code >= 0xd800 && code <= 0xdfff)
      ? String.fromCodePoint(code)
      : match;
  });

export const QuickReplacementDialog = ({ open, onClose, initialFrom = "", selectionTooLong = false }: QuickReplacementDialogProps) => {
  const { t } = useTranslation();
  const enabled = useSettingsStore((state) => state.settings?.text_replacements_enabled);
  const [from, setFrom] = useState(initialFrom);
  const [to, setTo] = useState("");
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const savingRef = useRef(false);
  const dialogRef = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  const initialFromRef = useRef(initialFrom);
  initialFromRef.current = initialFrom;
  const id = useId();

  useEffect(() => {
    if (!open) return;
    setFrom(initialFromRef.current);
    setTo("");
    setError("");
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const timer = window.setTimeout(() => dialogRef.current?.querySelector<HTMLInputElement>("input")?.focus(), 0);
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        if (!savingRef.current) closeRef.current();
      }
      if (event.key !== "Tab" || !dialogRef.current) return;
      const elements = Array.from(dialogRef.current.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled)'));
      const first = elements[0];
      const last = elements[elements.length - 1];
      if (!first) {
        event.preventDefault();
        dialogRef.current.focus();
      } else if (!dialogRef.current.contains(document.activeElement) || (event.shiftKey && document.activeElement === first) || (!event.shiftKey && document.activeElement === last)) {
        event.preventDefault();
        (event.shiftKey ? last : first).focus();
      }
    };
    window.addEventListener("keydown", handleKeyDown, true);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener("keydown", handleKeyDown, true);
      if (previousFocus?.isConnected) previousFocus.focus();
    };
  }, [open]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (savingRef.current) return;
    setError("");
    if (from.length === 0) {
      setError(t("textReplacement.quickAdd.emptySource"));
      return;
    }
    const store = useSettingsStore.getState();
    if (!store.settings || store.isUpdating.text_replacements) {
      setError(t("textReplacement.quickAdd.busy"));
      return;
    }
    const rules = store.settings.text_replacements ?? [];
    if (rules.some((rule) => {
      if (rule.is_regex) return false;
      const source = decodeLiteral(rule.from);
      return rule.case_sensitive ? source === from : source.toLowerCase() === from.toLowerCase();
    })) {
      setError(t("textReplacement.quickAdd.duplicate"));
      return;
    }
    savingRef.current = true;
    setSaving(true);
    try {
      await store.updateSetting("text_replacements", [...rules, {
        id: crypto.randomUUID(),
        from: from.replace(/\\/g, "\\\\"),
        to: to.replace(/\\/g, "\\\\"),
        enabled: true,
        case_sensitive: true,
        is_regex: false,
      }], { throwOnError: true });
      closeRef.current();
    } catch (failure) {
      setError(t("textReplacement.quickAdd.saveFailed", { error: failure instanceof Error ? failure.message : String(failure) }));
    } finally {
      savingRef.current = false;
      setSaving(false);
    }
  };

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm">
      <div ref={dialogRef} role="dialog" aria-modal="true" aria-labelledby={`${id}-title`} aria-describedby={`${id}-description`} aria-busy={saving} tabIndex={-1} className="w-full max-w-lg rounded-lg border border-[#3c3c3c] bg-[#151515] p-5 shadow-2xl">
        <h2 id={`${id}-title`} className="text-base font-semibold">{t("textReplacement.quickAdd.title")}</h2>
        <p id={`${id}-description`} className="mt-1 text-sm text-[#a0a0a0]">{t("textReplacement.quickAdd.description")}</p>
        {selectionTooLong && (
          <p role="status" className="mt-3 text-sm text-amber-300">
            {t("textReplacement.quickAdd.selectionTooLong")}
          </p>
        )}
        <form
          onSubmit={submit}
          onKeyDown={(event) => {
            if (event.key === "Enter" && event.nativeEvent.isComposing) {
              event.preventDefault();
            }
          }}
          className="mt-4 space-y-4"
        >
          <label className="block space-y-1 text-sm" htmlFor={`${id}-from`}>
            <span>{t("textReplacement.quickAdd.from")}</span>
            <Input id={`${id}-from`} className="w-full" value={from} disabled={saving} onChange={(event) => setFrom(event.target.value)} autoComplete="off" />
          </label>
          <label className="block space-y-1 text-sm" htmlFor={`${id}-to`}>
            <span>{t("textReplacement.quickAdd.to")}</span>
            <Input id={`${id}-to`} className="w-full" value={to} disabled={saving} onChange={(event) => setTo(event.target.value)} autoComplete="off" />
            <span className="block text-xs text-[#a0a0a0]">{t("textReplacement.quickAdd.emptyReplacementHint")}</span>
          </label>
          {enabled === false && <p className="text-sm text-amber-300">{t("textReplacement.quickAdd.disabled")} </p>}
          <button type="button" disabled={saving} className="text-sm text-[#c69cff] underline" onClick={() => { closeRef.current(); useNavigationStore.getState().setSection("textReplacement"); }}>
            {t("textReplacement.quickAdd.manage")}
          </button>
          {error && <p role="alert" className="text-sm text-red-400">{error}</p>}
          <div className="flex justify-end gap-2">
            <Button type="button" variant="secondary" disabled={saving} onClick={onClose}>{t("textReplacement.quickAdd.cancel")}</Button>
            <Button type="submit" disabled={saving}>{t(saving ? "textReplacement.quickAdd.saving" : "textReplacement.quickAdd.save")}</Button>
          </div>
        </form>
      </div>
    </div>
  );
};
