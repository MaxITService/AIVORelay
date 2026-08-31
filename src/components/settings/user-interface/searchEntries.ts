import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const userInterfaceSearchEntries = [
  { id: "overlay", section: "userInterface", anchor: "recording-overlay-settings", labelKey: "settingsSearch.items.overlay", fallbackLabel: "Recording overlay", keywords: ["overlay", "finalizing", "recording indicator", "appearance", "оверлей", "финализация"] },
  { id: "live-preview", section: "userInterface", anchor: "live-preview-settings", labelKey: "settingsSearch.items.livePreview", fallbackLabel: "Live Preview", keywords: ["preview", "staging", "window", "превью"] },
  { id: "interface", section: "userInterface", labelKey: "settingsSearch.items.interface", fallbackLabel: "Interface appearance and behaviour", keywords: ["interface", "appearance", "window", "tray", "sidebar", "theme", "ui", "интерфейс", "вид", "трей"] },
] as const satisfies readonly SettingsSearchEntry[];
