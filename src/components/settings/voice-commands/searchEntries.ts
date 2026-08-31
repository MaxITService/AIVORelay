import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const voiceCommandsSearchEntries = [
  { id: "voice-commands", section: "voiceCommands", labelKey: "settingsSearch.items.voiceCommands", fallbackLabel: "Voice commands", unavailableReasonKey: "settingsSearch.unavailable.voiceCommands", unavailableReasonFallback: "Enable Beta Voice Commands on Windows to open this section.", keywords: ["voice command", "action", "голосовые команды"] },
] as const satisfies readonly SettingsSearchEntry[];
