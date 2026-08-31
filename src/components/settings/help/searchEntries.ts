import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const helpSearchEntries = [
  { id: "help", section: "help", labelKey: "sidebar.help", fallbackLabel: "Help", keywords: ["help", "documentation", "guide", "how to", "справка", "документация", "помощь"] },
  { id: "whats-new", section: "help", anchor: "help-whats-new-title", labelKey: "help.whatsNew.title", fallbackLabel: "What's New", keywords: ["new features", "release highlights", "changes", "новые функции", "что нового", "изменения"] },
] as const satisfies readonly SettingsSearchEntry[];
