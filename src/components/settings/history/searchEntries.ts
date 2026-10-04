import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const historySearchEntries = [
  { id: "history", section: "history", labelKey: "settingsSearch.items.history", fallbackLabel: "History and recordings", keywords: ["history", "recording", "audio", "folder", "история", "записи"] },
  { id: "repaste-shortcut", section: "history", anchor: "shortcut-repaste_last", labelKey: "settings.general.shortcut.bindings.repaste_last.name", fallbackLabel: "Repaste Last", groupLabelKey: "settingsSearch.groups.shortcuts", groupFallbackLabel: "Shortcuts", keywords: ["repaste", "retry transcription", "last result", "paste again", "повторить вставку", "последний текст", "повтор транскрибации"] },
  { id: "history-limit", section: "history", anchor: "settings-history-limit", labelKey: "settings.history.historyLimit.title", fallbackLabel: "History Limit", keywords: ["history size","maximum entries","retention count","лимит истории","количество записей"] },
] as const satisfies readonly SettingsSearchEntry[];
