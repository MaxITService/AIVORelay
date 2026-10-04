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
  { id: "ai-replace-quick-tap", section: "aiReplace", anchor: "settings-ai-replace-quick-tap", labelKey: "settings.aiReplace.quickTap.title", fallbackLabel: "Quick Tap Mode", unavailableReasonKey: "settingsSearch.unavailable.windowsOnly", unavailableReasonFallback: "Available on Windows only.", isAvailable: () => isWindows, keywords: ["quick tap","short press","quick tap prompt","короткое нажатие","быстрое нажатие"] },
  { id: "ai-replace-quick-tap-threshold", section: "aiReplace", anchor: "settings-ai-replace-quick-tap-threshold", fallbackAnchor: "settings-ai-replace-quick-tap", labelKey: "settings.aiReplace.quickTap.threshold.title", fallbackLabel: "Quick Tap Threshold", unavailableReasonKey: "settingsSearch.unavailable.windowsOnly", unavailableReasonFallback: "Available on Windows only.", isAvailable: () => isWindows, keywords: ["quick tap threshold","tap duration","short press milliseconds","порог короткого нажатия","время нажатия"] },
  { id: "ai-replace-selection-prompts", section: "aiReplace", anchor: "settings-ai-replace-selection-prompts", labelKey: "settings.aiReplace.withSelection.title", fallbackLabel: "With Selection Mode", unavailableReasonKey: "settingsSearch.unavailable.windowsOnly", unavailableReasonFallback: "Available on Windows only.", isAvailable: () => isWindows, keywords: ["selection prompt","system prompt","user prompt template","instruction","промпт выделения","системный промпт","шаблон промпта"] },
  { id: "ai-replace-max-characters", section: "aiReplace", anchor: "settings-ai-replace-max-characters", labelKey: "settings.aiReplace.withSelection.maxChars.title", fallbackLabel: "Max Characters", unavailableReasonKey: "settingsSearch.unavailable.windowsOnly", unavailableReasonFallback: "Available on Windows only.", isAvailable: () => isWindows, keywords: ["max characters","selection length","text limit","максимум символов","лимит текста"] },
  { id: "ai-replace-restore-on-error", section: "aiReplace", anchor: "settings-ai-replace-restore-on-error", labelKey: "settings.aiReplace.withSelection.restoreOnError.label", fallbackLabel: "Attempt Restore On Error", unavailableReasonKey: "settingsSearch.unavailable.windowsOnly", unavailableReasonFallback: "Available on Windows only.", isAvailable: () => isWindows, keywords: ["restore on error","restore selection","undo failed replacement","восстановить при ошибке","восстановление выделения"] },
] as const satisfies readonly SettingsSearchEntry[];
