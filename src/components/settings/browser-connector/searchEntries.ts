import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const browserConnectorSearchEntries = [
  { id: "connector", section: "browserConnector", labelKey: "settingsSearch.items.connector", fallbackLabel: "Browser connector", keywords: ["chrome", "browser", "extension", "chatgpt", "claude", "браузер", "коннектор"] },
  { id: "connector-status", section: "browserConnector", anchor: "settings-connector-status", labelKey: "settings.browserConnector.status.sectionTitle", fallbackLabel: "Extension Status", groupLabelKey: "sidebar.browserConnector", groupFallbackLabel: "Connector", keywords: ["extension status","connection status","export extension","install extension","статус расширения","установка расширения"] },
] as const satisfies readonly SettingsSearchEntry[];
