import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const audioProcessingSearchEntries = [
  { id: "audio-processing", section: "audioProcessing", labelKey: "settingsSearch.items.audioProcessing", fallbackLabel: "Speech and audio processing", keywords: ["noise", "gain", "vad", "audio", "processing", "шум", "обработка аудио"] },
  { id: "filter-silence", section: "audioProcessing", anchor: "settings-filter-silence", labelKey: "settings.debug.filterSilence.label", fallbackLabel: "Filter Silence", keywords: ["silence filter","voice activity detection","vad","фильтр тишины","обнаружение речи"] },
  { id: "vad-backend", section: "audioProcessing", anchor: "settings-vad-backend", labelKey: "audioProcessing.vadBackend", fallbackLabel: "Filter Silence engine", keywords: ["vad backend","vad engine","silero","earshot","движок vad","движок фильтра тишины"] },
  { id: "vad-sensitivity", section: "audioProcessing", anchor: "settings-vad-sensitivity", labelKey: "audioProcessing.vadThreshold", fallbackLabel: "Voice Detection Sensitivity", keywords: ["vad threshold","sensitivity","quiet speech","clipped speech","чувствительность","порог vad","тихая речь"] },
] as const satisfies readonly SettingsSearchEntry[];
