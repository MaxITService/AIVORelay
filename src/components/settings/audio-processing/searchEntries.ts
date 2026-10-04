import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const audioProcessingSearchEntries = [
  { id: "audio-processing", section: "audioProcessing", labelKey: "settingsSearch.items.audioProcessing", fallbackLabel: "Speech and audio processing", keywords: ["noise", "gain", "vad", "audio", "processing", "шум", "обработка аудио"] },
  { id: "filter-silence", section: "audioProcessing", anchor: "settings-filter-silence", labelKey: "settings.debug.filterSilence.label", fallbackLabel: "Filter Silence", keywords: ["silence filter","voice activity detection","vad","фильтр тишины","обнаружение речи"] },
  { id: "vad-backend", section: "audioProcessing", anchor: "settings-vad-backend", labelKey: "audioProcessing.vadBackend", fallbackLabel: "Filter Silence engine", keywords: ["vad backend","vad engine","silero","earshot","движок vad","движок фильтра тишины"] },
] as const satisfies readonly SettingsSearchEntry[];
