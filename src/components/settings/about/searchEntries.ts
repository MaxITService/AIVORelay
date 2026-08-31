import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const aboutSearchEntries = [
  { id: "about", section: "about", labelKey: "sidebar.about", fallbackLabel: "About", keywords: ["about", "version", "update", "license", "credits", "о программе", "версия", "обновление", "лицензия"] },
] as const satisfies readonly SettingsSearchEntry[];
