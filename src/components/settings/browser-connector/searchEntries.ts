import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const browserConnectorSearchEntries = [
  { id: "connector", section: "browserConnector", labelKey: "settingsSearch.items.connector", fallbackLabel: "Browser connector", keywords: ["chrome", "browser", "extension", "chatgpt", "claude", "браузер", "коннектор"] },
  { id: "connector-status", section: "browserConnector", anchor: "settings-connector-status", labelKey: "settings.browserConnector.status.sectionTitle", fallbackLabel: "Extension Status", groupLabelKey: "sidebar.browserConnector", groupFallbackLabel: "Connector", keywords: ["extension status","connection status","export extension","install extension","статус расширения","установка расширения"] },
  { id: "connector-dictation", section: "browserConnector", anchor: "settings-connector-dictation", labelKey: "settings.general.shortcut.bindings.send_to_extension.name", fallbackLabel: "Send Transcription Directly to Extension", groupLabelKey: "sidebar.browserConnector", groupFallbackLabel: "Connector", keywords: ["send dictation","extension shortcut","browser transcription","отправить диктовку","шорткат расширения"] },
] as const satisfies readonly SettingsSearchEntry[];
