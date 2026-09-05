import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const advancedSearchEntries = [
  { id: "advanced", section: "advanced", labelKey: "settingsSearch.items.advanced", fallbackLabel: "Advanced application settings", keywords: ["advanced", "behavior", "startup", "webview", "memory", "ram", "headless", "speech only", "dictation", "расширенные", "запуск", "память", "без интерфейса", "только диктовка"] },
  { id: "speech-only-low-memory", section: "advanced", anchor: "speech-only-low-memory-settings", labelKey: "settings.advanced.neverLaunchWebview.label", fallbackLabel: "Start without WebView (speech only)", groupLabelKey: "settings.advanced.neverLaunchWebview.groupTitle", groupFallbackLabel: "Speech-only low-memory mode", keywords: ["memory", "low memory", "low-memory", "ram", "webview", "without webview", "no webview", "speech only", "speech-only", "dictation only", "headless", "minimal memory", "память", "низкое потребление памяти", "экономия памяти", "без webview", "без интерфейса", "только диктовка"] },
] as const satisfies readonly SettingsSearchEntry[];
