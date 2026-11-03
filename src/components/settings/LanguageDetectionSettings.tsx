import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { MODEL_CAPABILITY_LANGUAGES } from "@/lib/constants/languages";
import { languageIdentity } from "@handy-computer/models";
import { SettingsGroup } from "../ui/SettingsGroup";
import { Button } from "../ui/Button";
import { useSettingsStore } from "@/stores/settingsStore";

type Config = { spoken_languages: string[]; mode: "when_needed" | "always" };
type DownloadProgress = { model_id: string; percentage: number };
type LanguageStatus =
  | { kind: "auto" }
  | { kind: "selected"; language: string }
  | { kind: "detector_missing"; fallback: string }
  | { kind: "auto_unavailable"; language: string };

export const LanguageDetectionSettings: React.FC = () => {
  const { t } = useTranslation();
  const selectedModel = useSettingsStore((state) => state.settings?.selected_model);
  const selectedLanguage = useSettingsStore((state) => state.settings?.selected_language);
  const [modelRevision, setModelRevision] = useState(0);
  const [config, setConfig] = useState<Config | null>(null);
  const [downloaded, setDownloaded] = useState(false);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<number | null>(null);
  const [error, setError] = useState("");
  const [status, setStatus] = useState<LanguageStatus | null>(null);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    let unlistenComplete: (() => void) | undefined;
    let unlistenModels: (() => void) | undefined;
    let unlistenModelState: (() => void) | undefined;
    void Promise.all([
      invoke<Config>("get_language_detection_config"),
      invoke<boolean>("language_detector_downloaded"),
    ]).then(([value, ready]) => {
      if (active) { setConfig(value); setDownloaded(ready); }
    }).catch((reason) => { if (active) setError(String(reason)); });
    void listen<DownloadProgress>("model-download-progress", ({ payload }) => {
      if (payload.model_id === "language-detector") setProgress(payload.percentage);
    }).then((stop) => { if (active) unlisten = stop; else stop(); });
    void listen<string>("model-download-complete", ({ payload }) => {
      if (payload === "language-detector") { setDownloaded(true); setProgress(null); }
    }).then((stop) => { if (active) unlistenComplete = stop; else stop(); });
    void listen("models-updated", () => setModelRevision((value) => value + 1))
      .then((stop) => { if (active) unlistenModels = stop; else stop(); });
    void listen("model-state-changed", () => setModelRevision((value) => value + 1))
      .then((stop) => { if (active) unlistenModelState = stop; else stop(); });
    return () => { active = false; unlisten?.(); unlistenComplete?.(); unlistenModels?.(); unlistenModelState?.(); };
  }, []);

  useEffect(() => {
    if (!config) return;
    let active = true;
    void invoke<LanguageStatus | null>("get_local_language_detection_status")
      .then((value) => { if (active) setStatus(value); })
      .catch((reason) => { if (active) setError(String(reason)); });
    return () => { active = false; };
  }, [config, downloaded, selectedModel, selectedLanguage, modelRevision]);

  const save = async (next: Config) => {
    setBusy(true);
    setError("");
    try {
      await invoke("set_language_detection_config", { spokenLanguages: next.spoken_languages, mode: next.mode });
      setConfig(next);
    } catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  };
  const download = async () => {
    setBusy(true);
    setProgress(0);
    setError("");
    try {
      await invoke("download_language_detector");
      setDownloaded(await invoke<boolean>("language_detector_downloaded"));
    } catch (reason) { setError(String(reason)); }
    finally { setBusy(false); setProgress(null); }
  };
  const selected = config?.spoken_languages ?? [];

  return (
    <SettingsGroup title={t("settings.spokenLanguage.title", "Spoken language detection")}>
      <div className="space-y-3 p-4 text-sm text-text">
        <p className="text-text/70">{t("settings.spokenLanguage.description", "Choose the languages you speak. For Auto, a local model can select between them before transcription. The first compatible language is used if detection is unavailable. Leave the list empty to keep the existing behavior.")}</p>
        <p className="text-text/70">{t("settings.spokenLanguage.streaming", "Live streaming uses the model's own detection when available, otherwise the first compatible language. Unsupported languages and short or noisy speech can make detection unreliable.")}</p>
        <p className="text-text/70">{t("settings.spokenLanguage.unsupportedEngine", "Models that cannot accept a language selection use their own detection, even in Always mode.")}</p>
        <select className="w-full rounded border border-white/20 bg-mid-gray p-2" disabled={!config || busy} value="" onChange={(event) => {
          const value = event.target.value;
          if (config && value) void save({ ...config, spoken_languages: [...selected, value] });
        }} aria-label={t("settings.spokenLanguage.add", "Add spoken language")}>
          <option value="">{t("settings.spokenLanguage.add", "Add spoken language…")}</option>
          {MODEL_CAPABILITY_LANGUAGES.filter((language) => !selected.some((code) => languageIdentity(code) === languageIdentity(language.value))).map((language) => (
            <option key={language.value} value={language.value}>{language.label}</option>
          ))}
        </select>
        <div className="flex flex-wrap gap-2">
          {selected.map((code) => (
            <button type="button" key={code} disabled={busy} className="rounded border border-white/20 px-2 py-1" onClick={() => config && void save({ ...config, spoken_languages: selected.filter((item) => item !== code) })} aria-label={t("settings.spokenLanguage.remove", { defaultValue: "Remove {{language}}", language: code })}>
              {MODEL_CAPABILITY_LANGUAGES.find((language) => languageIdentity(language.value) === languageIdentity(code))?.label ?? code} ×
            </button>
          ))}
        </div>
        <label className="flex flex-col gap-1">
          {t("settings.spokenLanguage.mode", "Use detector")}
          <select className="rounded border border-white/20 bg-mid-gray p-2" disabled={!config || busy} value={config?.mode ?? "when_needed"} onChange={(event) => config && void save({ ...config, mode: event.target.value as Config["mode"] })}>
            <option value="when_needed">{t("settings.spokenLanguage.whenNeeded", "When the model cannot detect language")}</option>
            <option value="always">{t("settings.spokenLanguage.always", "Also for models with built-in detection")}</option>
          </select>
        </label>
        <p className="text-text/70">{downloaded ? t("settings.spokenLanguage.downloaded", "Language detector downloaded") : t("settings.spokenLanguage.missing", "Language detector is not downloaded (about 45 MB).")}</p>
        {status && <p className="text-text/70">{status.kind === "auto"
          ? t("settings.spokenLanguage.statusAuto", "Default local model for completed audio: automatic language selection")
          : status.kind === "detector_missing"
            ? t("settings.spokenLanguage.statusMissing", { defaultValue: "Default local model for completed audio: {{language}} until the detector is downloaded", language: status.fallback })
            : t("settings.spokenLanguage.statusFixed", { defaultValue: "Default local model for completed audio: {{language}}", language: status.language })}</p>}
        {!downloaded && <Button variant="secondary" size="sm" disabled={busy} onClick={() => void download()}>{t("settings.spokenLanguage.download", "Download detector")}</Button>}
        {busy && progress !== null && <div className="flex items-center gap-3"><span>{Math.round(progress)}%</span><Button variant="secondary" size="sm" onClick={() => void invoke("cancel_language_detector_download")}>{t("common.cancel", "Cancel")}</Button></div>}
        {error && <p role="alert" className="text-red-400">{error}</p>}
      </div>
    </SettingsGroup>
  );
};
