import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const advancedSearchEntries = [
  { id: "advanced", section: "advanced", labelKey: "settingsSearch.items.advanced", fallbackLabel: "Advanced application settings", keywords: ["advanced", "behavior", "startup", "webview", "memory", "ram", "headless", "speech only", "dictation", "расширенные", "запуск", "память", "без интерфейса", "только диктовка"] },
] as const satisfies readonly SettingsSearchEntry[];
