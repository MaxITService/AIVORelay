import { type as getOsType } from "@tauri-apps/plugin-os";
import type { SettingsSearchEntry } from "../settingsSearchTypes";

const unavailableReasonFallback = "Available on Windows only.";

const isWindows = (() => {
  try {
    return getOsType() === "windows";
  } catch {
    return false;
  }
})();

export const aiReplaceSearchEntries = [
  { id: "ai-replace", section: "aiReplace", labelKey: "settingsSearch.items.aiReplace", fallbackLabel: "AI Replace", unavailableReasonKey: "settingsSearch.unavailable.windowsOnly", unavailableReasonFallback, keywords: ["replace selection", "instruction", "prompt", "замена"] },
  { id: "ai-replace-shortcut", section: "aiReplace", anchor: "shortcut-ai_replace_selection", labelKey: "settings.general.shortcut.bindings.ai_replace_selection.name", fallbackLabel: "AI Replace Selection", groupLabelKey: "settingsSearch.groups.shortcuts", groupFallbackLabel: "Shortcuts", unavailableReasonKey: "settingsSearch.unavailable.windowsOnly", unavailableReasonFallback, keywords: ["ai replace hotkey", "selected text shortcut", "replace selection", "замена выделения", "горячая клавиша"] },
  { id: "ai-replace-no-selection", section: "aiReplace", anchor: "settings-ai-replace-no-selection", labelKey: "settings.aiReplace.noSelection.title", fallbackLabel: "Without Selection Mode", unavailableReasonKey: "settingsSearch.unavailable.windowsOnly", unavailableReasonFallback: "Available on Windows only.", isAvailable: () => isWindows, keywords: ["without selection","no selection prompt","allow without selection","без выделения","промпт без выделения"] },
] as const satisfies readonly SettingsSearchEntry[];
