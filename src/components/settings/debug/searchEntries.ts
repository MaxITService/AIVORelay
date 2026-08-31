import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const debugSearchEntries = [
  { id: "debug", section: "debug", labelKey: "settingsSearch.items.debug", fallbackLabel: "Debug and logs", keywords: ["debug", "logs", "diagnostics", "troubleshoot", "логи", "диагностика"] },
] as const satisfies readonly SettingsSearchEntry[];
