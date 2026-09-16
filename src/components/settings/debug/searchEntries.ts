import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const debugSearchEntries = [
  { id: "dictation-quick-tap", section: "debug", anchor: "settings-dictation-quick-tap-threshold", labelKey: "settings.debug.dictationQuickTap.title", fallbackLabel: "Short press threshold (ms)", keywords: ["quick tap", "short press", "empty transcription", "no text", "threshold", "короткое нажатие", "нет текста", "порог"] },
  { id: "debug", section: "debug", labelKey: "settingsSearch.items.debug", fallbackLabel: "Debug and logs", keywords: ["debug", "logs", "diagnostics", "troubleshoot", "логи", "диагностика"] },
] as const satisfies readonly SettingsSearchEntry[];
