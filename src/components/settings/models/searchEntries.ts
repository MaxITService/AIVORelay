import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const modelsSearchEntries = [
  { id: "models", section: "models", anchor: "settings-models", labelKey: "settingsSearch.items.models", fallbackLabel: "Transcription models", groupLabelKey: "settingsSearch.groups.models", groupFallbackLabel: "Models", keywords: ["stt", "local", "cloud", "provider", "model", "модель", "провайдер"] },
  { id: "api-keys", section: "models", anchor: "settings-api-keys", labelKey: "settingsSearch.items.apiKeys", fallbackLabel: "API keys and remote providers", groupLabelKey: "settingsSearch.groups.providers", groupFallbackLabel: "Providers", keywords: ["api key", "credential", "endpoint", "openai", "gemini", "soniox", "deepgram", "vercel", "ключ", "эндпоинт"] },
] as const satisfies readonly SettingsSearchEntry[];
