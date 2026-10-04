import { type as getOsType } from "@tauri-apps/plugin-os";
import type { SettingsSearchEntry } from "../settingsSearchTypes";

const isWindows = (() => {
  try {
    return getOsType() === "windows";
  } catch {
    return false;
  }
})();

export const sendSelectedTextSearchEntries = [
  { id: "send-selected", section: "sendSelectedText", labelKey: "settingsSearch.items.sendSelectedText", fallbackLabel: "Send selected text", unavailableReasonKey: "settingsSearch.unavailable.windowsOnly", unavailableReasonFallback: "Available on Windows only.", keywords: ["selected text", "markdown", "json", "command", "выделенный текст"] },
  { id: "selected-text-history", section: "sendSelectedText", anchor: "settings-selected-text-history", expandAnchor: "settings-selected-text-history-tab", labelKey: "settingsSearch.items.selected-text-history", fallbackLabel: "Send Selected Text execution history", groupLabelKey: "settingsSearch.items.sendSelectedText", groupFallbackLabel: "Send selected text", unavailableReasonKey: "settingsSearch.unavailable.windowsOnly", unavailableReasonFallback: "Available on Windows only.", isAvailable: () => isWindows, keywords: ["selected text history","command execution history","failed command","история отправки текста","история выполнения команд"] },
] as const satisfies readonly SettingsSearchEntry[];
