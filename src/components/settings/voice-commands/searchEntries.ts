import { type as getOsType } from "@tauri-apps/plugin-os";
import type { SettingsSearchEntry } from "../settingsSearchTypes";

const isWindows = (() => {
  try {
    return getOsType() === "windows";
  } catch {
    return false;
  }
})();

export const voiceCommandsSearchEntries = [
  { id: "voice-commands", section: "voiceCommands", labelKey: "settingsSearch.items.voiceCommands", fallbackLabel: "Voice commands", unavailableReasonKey: "settingsSearch.unavailable.voiceCommands", unavailableReasonFallback: "Enable Beta Voice Commands on Windows to open this section.", keywords: ["voice command", "action", "голосовые команды"] },
  { id: "voice-command-enabled", section: "voiceCommands", anchor: "settings-voice-command-enabled", fallbackAnchor: "settings-voice-command-enabled", labelKey: "settingsSearch.items.voice-command-enabled", fallbackLabel: "Enable Voice Commands", isAvailable: () => isWindows, unavailableReasonKey: "settingsSearch.unavailable.voiceCommands", unavailableReasonFallback: "Enable Beta Voice Commands on Windows to open this section.", keywords: ["enable voice commands","voice commands toggle","включить голосовые команды","голосовые команды"] },
  { id: "voice-command-llm-fallback", section: "voiceCommands", anchor: "settings-voice-command-llm-fallback", fallbackAnchor: "settings-voice-command-enabled", labelKey: "settingsSearch.items.voice-command-llm-fallback", fallbackLabel: "LLM Fallback", isAvailable: () => isWindows, unavailableReasonKey: "settingsSearch.unavailable.voiceCommands", unavailableReasonFallback: "Enable Beta Voice Commands on Windows to open this section.", keywords: ["voice command llm","command suggestions","powershell suggestions","llm fallback","подсказки команд","llm для команд"] },
] as const satisfies readonly SettingsSearchEntry[];
