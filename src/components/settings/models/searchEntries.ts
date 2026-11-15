import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const modelsSearchEntries = [
  { id: "models", section: "models", anchor: "settings-models", labelKey: "settingsSearch.items.models", fallbackLabel: "Transcription models", groupLabelKey: "settingsSearch.groups.models", groupFallbackLabel: "Models", keywords: ["stt", "local", "cloud", "provider", "model", "sort models", "sort by name", "alphabetical", "elevenlabs", "scribe", "scribe v3", "scribe v3 live", "scribe_v3", "scribe_v3_live", "модель", "провайдер"] },
  { id: "api-keys", section: "models", anchor: "settings-api-keys", labelKey: "settingsSearch.items.apiKeys", fallbackLabel: "API keys and remote providers", groupLabelKey: "settingsSearch.groups.providers", groupFallbackLabel: "Providers", keywords: ["api key", "credential", "endpoint", "openai", "gemini", "soniox", "deepgram", "vercel", "reson8", "rumble-2", "elevenlabs", "scribe", "scribe v3", "scribe v3 live", "scribe_v3", "scribe_v3_live", "xi-api-key", "ключ", "эндпоинт"] },
] as const satisfies readonly SettingsSearchEntry[];
