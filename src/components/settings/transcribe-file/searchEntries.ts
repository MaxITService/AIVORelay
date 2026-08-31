import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const transcribeFileSearchEntries = [
  { id: "file-model", section: "transcribeFile", anchor: "transcribe-file-model-settings", labelKey: "settingsSearch.items.fileModel", fallbackLabel: "File transcription model and settings", groupLabelKey: "settingsSearch.groups.modelAndLanguage", groupFallbackLabel: "Model & language", keywords: ["transcribe file", "audio file", "video file", "model", "chunking", "language hints", "транскрибация файла"] },
  { id: "file-diarization", section: "transcribeFile", anchor: "transcribe-file-diarization", expandAnchor: "transcribe-file-model-settings", labelKey: "settingsSearch.items.fileDiarization", fallbackLabel: "File transcription speaker diarization", groupLabelKey: "settingsSearch.groups.modelAndLanguage", groupFallbackLabel: "Model & language", keywords: ["diarization", "speaker", "file", "диаризация", "спикер", "файл"] },
] as const satisfies readonly SettingsSearchEntry[];
