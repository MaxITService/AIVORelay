import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const audioProcessingSearchEntries = [
  { id: "audio-processing", section: "audioProcessing", labelKey: "settingsSearch.items.audioProcessing", fallbackLabel: "Speech and audio processing", keywords: ["noise", "gain", "vad", "audio", "processing", "шум", "обработка аудио"] },
] as const satisfies readonly SettingsSearchEntry[];
