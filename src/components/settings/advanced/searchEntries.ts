import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const advancedSearchEntries = [
  { id: "advanced", section: "advanced", labelKey: "settingsSearch.items.advanced", fallbackLabel: "Advanced application settings", keywords: ["advanced", "behavior", "startup", "расширенные", "запуск"] },
] as const satisfies readonly SettingsSearchEntry[];
