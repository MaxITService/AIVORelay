import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const debugSearchEntries = [
  { id: "dictation-quick-tap", section: "debug", anchor: "settings-dictation-quick-tap-threshold", labelKey: "settings.debug.dictationQuickTap.title", fallbackLabel: "Short press threshold (ms)", keywords: ["quick tap", "short press", "empty transcription", "no text", "threshold", "короткое нажатие", "нет текста", "порог"] },
  { id: "debug", section: "debug", labelKey: "settingsSearch.items.debug", fallbackLabel: "Debug and logs", keywords: ["debug", "logs", "diagnostics", "troubleshoot", "логи", "диагностика"] },
  { id: "settings-directory", section: "debug", anchor: "settings-settings-directory", labelKey: "settings.debug.settingsDirectoryTitle", fallbackLabel: "Settings Directory", keywords: ["settings folder","configuration file","settings.json","папка настроек","файл настроек"] },
  { id: "log-directory", section: "debug", anchor: "settings-log-directory", labelKey: "settings.debug.logDirectory.title", fallbackLabel: "Log Directory", keywords: ["log folder","diagnostic files","папка логов","файлы журналов"] },
  { id: "log-level", section: "debug", anchor: "settings-log-level", labelKey: "settings.debug.logLevel.title", fallbackLabel: "Log Level", keywords: ["logging level","verbosity","debug log","trace","уровень логов","подробность журналов"] },
  { id: "transcription-text-logging", section: "debug", anchor: "settings-transcription-text-logging", labelKey: "settings.debug.transcriptionTextLogging.title", fallbackLabel: "Log transcription text", keywords: ["transcription logs","log transcript","privacy","журнал текста","текст в логах"] },
  { id: "sound-theme", section: "debug", anchor: "settings-sound-theme", labelKey: "settings.debug.soundTheme.label", fallbackLabel: "Sound Theme", keywords: ["sound pack","sound theme","beep theme","звуковая тема","набор звуков"] },
  { id: "always-on-microphone", section: "debug", anchor: "settings-always-on-microphone", labelKey: "settings.debug.alwaysOnMicrophone.label", fallbackLabel: "Always-On Microphone", keywords: ["always on mic","microphone stream","mic latency","постоянный микрофон","задержка микрофона"] },
] as const satisfies readonly SettingsSearchEntry[];
