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
  { id: "selected-text-presets", section: "sendSelectedText", anchor: "settings-selected-text-presets", expandAnchor: "settings-selected-text-presets-tab", labelKey: "settingsSearch.items.selected-text-presets", fallbackLabel: "Send Selected Text presets", groupLabelKey: "settingsSearch.items.sendSelectedText", groupFallbackLabel: "Send selected text", unavailableReasonKey: "settingsSearch.unavailable.windowsOnly", unavailableReasonFallback: "Available on Windows only.", isAvailable: () => isWindows, keywords: ["presets","preset hotkey","filename template","output directory","save selection","markdown","json","powershell command","пресеты","шаблон имени файла","папка вывода"] },
  { id: "selected-text-history-options", section: "sendSelectedText", anchor: "settings-selected-text-history-options", expandAnchor: "settings-selected-text-presets-tab", labelKey: "settingsSearch.items.selected-text-history-options", fallbackLabel: "Send Selected Text history and error overlay", groupLabelKey: "settingsSearch.items.sendSelectedText", groupFallbackLabel: "Send selected text", unavailableReasonKey: "settingsSearch.unavailable.windowsOnly", unavailableReasonFallback: "Available on Windows only.", isAvailable: () => isWindows, keywords: ["selected text history limit","command error overlay","error overlay duration","лимит истории отправки","оверлей ошибок команд"] },
] as const satisfies readonly SettingsSearchEntry[];
