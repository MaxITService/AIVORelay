import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { useSettings } from "../../../hooks/useSettings";
import { sessionToast as toast } from "@/lib/sessionToast";
import { SettingContainer } from "../../ui/SettingContainer";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { Input } from "../../ui/Input";

export function GeminiEarlyFinalizationSettings() {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();
  const preferences = settings as {
    gemini_live_early_finalization_enabled?: boolean;
    gemini_live_early_finalization_delay_ms?: number;
  } | null;
  const enabled = preferences?.gemini_live_early_finalization_enabled ?? true;
  const delay = preferences?.gemini_live_early_finalization_delay_ms ?? 500;
  const [draft, setDraft] = useState(String(delay));
  const [busy, setBusy] = useState(false);
  const saving = useRef(false);
  useEffect(() => setDraft(String(delay)), [delay]);

  const save = async (change: { enabled?: boolean; delayMs?: number }) => {
    if (saving.current) return;
    saving.current = true;
    setBusy(true);
    try {
      await invoke("change_gemini_early_finalization_setting", {
        enabled: change.enabled ?? null,
        delayMs: change.delayMs ?? null,
      });
      await refreshSettings();
    } catch (error) {
      setDraft(String(delay));
      toast.error(String(error));
    } finally {
      saving.current = false;
      setBusy(false);
    }
  };

  return (
    <div className="mx-4 rounded-lg border border-white/10">
      <SettingContainer
        title={<span>{t("geminiEarlyFinalization.title", "Early finalization")} <span className="ml-2 text-xs text-green-400">{t("geminiEarlyFinalization.recommended", "Recommended")}</span></span>}
        description={t("geminiEarlyFinalization.help", "Gemini can send its final completion signal several seconds after the last words, causing a late space to replace newly selected text. After Stop, wait only for the chosen delay, finish insertion with a space if needed, and close the stream. Late words may be lost, including from History. A selection made during this delay can still be replaced. Applies to dictation into other apps; Live Monitor and Output to Preview keep full finalization. This option controls the final separator independently of the trailing-space setting.")}
        descriptionMode="inline"
        grouped
      >
        <ToggleSwitch checked={enabled} onChange={value => void save({ enabled: value })} isUpdating={busy} ariaLabel={t("geminiEarlyFinalization.title", "Early finalization")} />
      </SettingContainer>
      <SettingContainer
        title={t("geminiEarlyFinalization.delay", "Delay after Stop (ms)")}
        description={t("geminiEarlyFinalization.delayHelp", "Default: 500 ms. Range: 100–5000 ms. Increase this if final words are missing. Actual insertion can be later if the UI or paste queue is busy.")}
        descriptionMode="inline"
        grouped
      >
        <Input
          type="number" min={100} max={5000} step={50}
          aria-label={t("geminiEarlyFinalization.delay", "Delay after Stop (ms)")}
          value={draft} disabled={busy || !enabled}
          onChange={event => setDraft(event.target.value)}
          onBlur={() => {
            const value = Number(draft);
            if (!draft.trim() || !Number.isInteger(value) || value < 100 || value > 5000) {
              setDraft(String(delay));
              return;
            }
            if (value !== delay) void save({ delayMs: value });
          }}
          onKeyDown={event => { if (event.key === "Enter") event.currentTarget.blur(); }}
          className="w-28"
        />
      </SettingContainer>
    </div>
  );
}
