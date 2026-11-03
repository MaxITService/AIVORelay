import React, { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { MODEL_CAPABILITY_LANGUAGES, recognitionLanguage } from "@/lib/constants/languages";
import { LanguageDetectorDownload } from "../shared/LanguageDetectorDownload";
import { getLocalizedLanguageLabel } from "./models/ModelMetadataPanel";
import { SpokenLanguagePicker } from "./SpokenLanguagePicker";

export type LanguageDetectionConfig = { spoken_languages: string[]; mode: "when_needed" | "always"; window_secs: number };

export const LanguageDetectionSettings: React.FC<{
  config: LanguageDetectionConfig;
  availableLanguages: readonly string[];
  supportsSelection: boolean;
  usesPreferences: boolean;
  unavailableReason?: string;
  disabled?: boolean;
  onChange: (config: LanguageDetectionConfig) => Promise<void>;
}> = ({ config, availableLanguages, supportsSelection, usesPreferences, unavailableReason, disabled = false, onChange }) => {
  const { t, i18n } = useTranslation();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const saving = useRef(false);

  const save = async (next: LanguageDetectionConfig) => {
    if (saving.current) return;
    saving.current = true;
    setBusy(true);
    setError("");
    try {
      await onChange(next);
    } catch (reason) { setError(String(reason)); }
    finally { saving.current = false; setBusy(false); }
  };
  const selected = config?.spoken_languages ?? [];
  const compatible = selected.filter((code) => availableLanguages.some((value) => recognitionLanguage(value) === recognitionLanguage(code)));
  const unavailable = supportsSelection ? selected.filter((code) => !compatible.includes(code)) : [];
  const languageLabel = (code: string) => getLocalizedLanguageLabel(code, i18n.resolvedLanguage ?? i18n.language);
  const controlsDisabled = disabled || busy;
  const seen = new Set<string>();
  const languageOptions = MODEL_CAPABILITY_LANGUAGES.filter(language => {
    const identity = recognitionLanguage(language.value);
    if (seen.has(identity) || !availableLanguages.some(code => recognitionLanguage(code) === identity)) return false;
    seen.add(identity);
    return true;
  }).map(language => {
    const label = languageLabel(language.value);
    const nativeLabel = getLocalizedLanguageLabel(language.value, recognitionLanguage(language.value));
    return { value: language.value, label, nativeLabel, searchText: `${label} ${nativeLabel} ${language.label} ${language.value}` };
  }).sort((a, b) => a.label.localeCompare(b.label, i18n.resolvedLanguage ?? i18n.language));

  return (
      <div className="space-y-3 rounded-lg border border-amber-500/20 bg-amber-500/5 p-3 text-xs text-text">
        <p className="text-text/70">{t("settings.spokenLanguage.profileDescription")}</p>
        <p className="text-text/70">{t("settings.spokenLanguage.streaming", "Live streaming uses the model's own detection when available, otherwise the first compatible language. Unsupported languages and short or noisy speech can make detection unreliable.")}</p>
        {!supportsSelection && <p className="text-amber-400">{unavailableReason ?? t("settings.spokenLanguage.unsupportedEngine")}</p>}
        {unavailable.length > 0 && <p className="text-amber-400">{t("settings.spokenLanguage.incompatible", { languages: unavailable.map(languageLabel).join(", ") })}</p>}
        {usesPreferences && selected.length > 0 && compatible.length === 0 && <p className="text-amber-400">{t("settings.spokenLanguage.noCompatible")}</p>}
        {usesPreferences && compatible.length === 1 && <p className="text-amber-400">{t("settings.spokenLanguage.singleCompatible", { language: languageLabel(compatible[0]) })}</p>}
        {/* Keep search focus during this control's save; selection changes remain blocked by saving. */}
        <SpokenLanguagePicker options={languageOptions} selected={selected} languageLabel={languageLabel}
          unavailable={unavailable} disabled={disabled && !busy} selectionDisabled={!supportsSelection} saving={busy}
          onChange={spoken_languages => void save({ ...config, spoken_languages })} />
        <label className="flex flex-col gap-1">
          {t("settings.spokenLanguage.window", "Maximum speech used for detection")}
          <select className="rounded border border-white/20 bg-mid-gray p-2" disabled={controlsDisabled || !supportsSelection} value={config.window_secs} onChange={(event) => void save({ ...config, window_secs: Number(event.target.value) })}>
            {Array.from({ length: 29 }, (_, index) => index + 2).map((seconds) => <option key={seconds} value={seconds}>{seconds} s</option>)}
          </select>
        </label>
        <LanguageDetectorDownload disabled={disabled} />
        {error && <p role="alert" className="text-red-400">{error}</p>}
      </div>
  );
};
