import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const sendSelectedTextSearchEntries = [
  { id: "send-selected", section: "sendSelectedText", labelKey: "settingsSearch.items.sendSelectedText", fallbackLabel: "Send selected text", unavailableReasonKey: "settingsSearch.unavailable.windowsOnly", unavailableReasonFallback: "Available on Windows only.", keywords: ["selected text", "markdown", "json", "command", "выделенный текст"] },
] as const satisfies readonly SettingsSearchEntry[];
