import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const browserConnectorSearchEntries = [
  { id: "connector", section: "browserConnector", labelKey: "settingsSearch.items.connector", fallbackLabel: "Browser connector", keywords: ["chrome", "browser", "extension", "chatgpt", "claude", "браузер", "коннектор"] },
] as const satisfies readonly SettingsSearchEntry[];
