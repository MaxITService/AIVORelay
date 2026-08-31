import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const historySearchEntries = [
  { id: "history", section: "history", labelKey: "settingsSearch.items.history", fallbackLabel: "History and recordings", keywords: ["history", "recording", "audio", "folder", "история", "записи"] },
  { id: "repaste-shortcut", section: "history", anchor: "shortcut-repaste_last", labelKey: "settings.general.shortcut.bindings.repaste_last.name", fallbackLabel: "Repaste Last", groupLabelKey: "settingsSearch.groups.shortcuts", groupFallbackLabel: "Shortcuts", keywords: ["repaste", "retry transcription", "last result", "paste again", "повторить вставку", "последний текст", "повтор транскрибации"] },
] as const satisfies readonly SettingsSearchEntry[];
