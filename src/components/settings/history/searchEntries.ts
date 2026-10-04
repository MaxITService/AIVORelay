import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const historySearchEntries = [
  { id: "history", section: "history", labelKey: "settingsSearch.items.history", fallbackLabel: "History and recordings", keywords: ["history", "recording", "audio", "folder", "история", "записи"] },
  { id: "repaste-shortcut", section: "history", anchor: "shortcut-repaste_last", labelKey: "settings.general.shortcut.bindings.repaste_last.name", fallbackLabel: "Repaste Last", groupLabelKey: "settingsSearch.groups.shortcuts", groupFallbackLabel: "Shortcuts", keywords: ["repaste", "retry transcription", "last result", "paste again", "повторить вставку", "последний текст", "повтор транскрибации"] },
  { id: "history-limit", section: "history", anchor: "settings-history-limit", labelKey: "settings.history.historyLimit.title", fallbackLabel: "History Limit", keywords: ["history size","maximum entries","retention count","лимит истории","количество записей"] },
  { id: "recording-retention", section: "history", anchor: "settings-recording-retention", labelKey: "settings.history.recordingRetention.title", fallbackLabel: "Recording Retention", keywords: ["keep recordings","delete recordings","retention period","audio cleanup","хранение записей","удаление аудио"] },
] as const satisfies readonly SettingsSearchEntry[];
