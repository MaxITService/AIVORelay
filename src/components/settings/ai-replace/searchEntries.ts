import type { SettingsSearchEntry } from "../settingsSearchTypes";

const unavailableReasonFallback = "Available on Windows only.";

export const aiReplaceSearchEntries = [
  { id: "ai-replace", section: "aiReplace", labelKey: "settingsSearch.items.aiReplace", fallbackLabel: "AI Replace", unavailableReasonKey: "settingsSearch.unavailable.windowsOnly", unavailableReasonFallback, keywords: ["replace selection", "instruction", "prompt", "замена"] },
  { id: "ai-replace-shortcut", section: "aiReplace", anchor: "shortcut-ai_replace_selection", labelKey: "settings.general.shortcut.bindings.ai_replace_selection.name", fallbackLabel: "AI Replace Selection", groupLabelKey: "settingsSearch.groups.shortcuts", groupFallbackLabel: "Shortcuts", unavailableReasonKey: "settingsSearch.unavailable.windowsOnly", unavailableReasonFallback, keywords: ["ai replace hotkey", "selected text shortcut", "replace selection", "замена выделения", "горячая клавиша"] },
] as const satisfies readonly SettingsSearchEntry[];
