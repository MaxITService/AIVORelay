import { type as getOsType } from "@tauri-apps/plugin-os";
import type { SettingsSearchEntry } from "../settingsSearchTypes";

const isWindows = (() => {
  try {
    return getOsType() === "windows";
  } catch {
    return false;
  }
})();

export const debugSearchEntries = [
  { id: "dictation-quick-tap", section: "debug", anchor: "settings-dictation-quick-tap-threshold", labelKey: "settings.debug.dictationQuickTap.title", fallbackLabel: "Short press threshold (ms)", keywords: ["quick tap", "short press", "empty transcription", "no text", "threshold", "короткое нажатие", "нет текста", "порог"] },
  { id: "debug", section: "debug", labelKey: "settingsSearch.items.debug", fallbackLabel: "Debug and logs", keywords: ["debug", "logs", "diagnostics", "troubleshoot", "логи", "диагностика"] },
  { id: "settings-directory", section: "debug", anchor: "settings-settings-directory", labelKey: "settings.debug.settingsDirectoryTitle", fallbackLabel: "Settings Directory", keywords: ["settings folder","configuration file","settings.json","папка настроек","файл настроек"] },
  { id: "log-directory", section: "debug", anchor: "settings-log-directory", labelKey: "settings.debug.logDirectory.title", fallbackLabel: "Log Directory", keywords: ["log folder","diagnostic files","папка логов","файлы журналов"] },
  { id: "log-level", section: "debug", anchor: "settings-log-level", labelKey: "settings.debug.logLevel.title", fallbackLabel: "Log Level", keywords: ["logging level","verbosity","debug log","trace","уровень логов","подробность журналов"] },
  { id: "transcription-text-logging", section: "debug", anchor: "settings-transcription-text-logging", labelKey: "settings.debug.transcriptionTextLogging.title", fallbackLabel: "Log transcription text", keywords: ["transcription logs","log transcript","privacy","журнал текста","текст в логах"] },
  { id: "sound-theme", section: "debug", anchor: "settings-sound-theme", labelKey: "settings.debug.soundTheme.label", fallbackLabel: "Sound Theme", keywords: ["sound pack","sound theme","beep theme","звуковая тема","набор звуков"] },
  { id: "always-on-microphone", section: "debug", anchor: "settings-always-on-microphone", labelKey: "settings.debug.alwaysOnMicrophone.label", fallbackLabel: "Always-On Microphone", keywords: ["always on mic","microphone stream","mic latency","постоянный микрофон","задержка микрофона"] },
  { id: "clamshell-microphone", section: "debug", anchor: "settings-clamshell-microphone", labelKey: "settings.debug.clamshellMicrophone.title", fallbackLabel: "Clamshell Microphone", keywords: ["laptop lid","closed lid","clamshell","закрытая крышка","микрофон ноутбука"] },
  { id: "extra-recording-buffer", section: "debug", anchor: "settings-extra-recording-buffer", labelKey: "settings.debug.recordingBuffer.title", fallbackLabel: "Extra Local Recording Buffer", keywords: ["recording tail","trailing speech","extra buffer","обрезание конца","буфер записи","последнее слово"] },
  { id: "keep-microphone-open", section: "debug", anchor: "settings-keep-microphone-open", labelKey: "settings.advanced.lazyStreamClose.label", fallbackLabel: "Keep Mic Open Between Transcriptions", unavailableReasonKey: "settingsSearch.unavailable.windowsOnly", unavailableReasonFallback: "Available on Windows only.", isAvailable: () => isWindows, keywords: ["keep mic open","lazy stream close","microphone latency","держать микрофон открытым","задержка микрофона"] },
  { id: "shortcut-engine", section: "debug", anchor: "settings-shortcut-engine", labelKey: "settings.debug.shortcutEngine.title", fallbackLabel: "Shortcut Engine", unavailableReasonKey: "settingsSearch.unavailable.windowsOnly", unavailableReasonFallback: "Available on Windows only.", isAvailable: () => isWindows, keywords: ["hotkey backend","shortcut backend","rdev","handykeys","tauri shortcuts","движок горячих клавиш","шорткаты"] },
  { id: "first-start-wizard", section: "debug", anchor: "settings-first-start-wizard", labelKey: "settings.debug.firstStartWizard.title", fallbackLabel: "First Start Wizard", keywords: ["onboarding","setup wizard","first start","мастер настройки","первый запуск"] },
  { id: "secret-logging", section: "debug", anchor: "settings-secret-logging", labelKey: "settings.debug.unsafeSecretLogging.title", fallbackLabel: "Do not redact secrets from logs — very dangerous", keywords: ["redact secrets","secret logging","api key logs","redaction","секреты в логах","маскировка ключей"] },
  { id: "dev-console-log-level", section: "debug", anchor: "settings-dev-console-log-level", labelKey: "settings.debug.devConsoleLogLevel.title", fallbackLabel: "Dev Console Log Level", isAvailable: () => import.meta.env.DEV, unavailableReasonKey: "settingsSearch.unavailable.developmentOnly", unavailableReasonFallback: "Available in development builds only.", keywords: ["dev console","console logs","frontend logs","консоль разработчика","логи интерфейса"] },
  { id: "beta-voice-commands", section: "debug", anchor: "settings-beta-voice-commands", labelKey: "settings.debug.voiceCommands.title", fallbackLabel: "Voice Commands", unavailableReasonKey: "settingsSearch.unavailable.windowsOnly", unavailableReasonFallback: "Available on Windows only.", isAvailable: () => isWindows, keywords: ["beta voice commands","enable voice commands","experimental voice commands","включить голосовые команды","бета голосовые команды"] },
  { id: "session-toast-history", section: "debug", anchor: "settings-session-toast-history", labelKey: "settingsSearch.items.session-toast-history", fallbackLabel: "Session notification history", keywords: ["toast history","notifications","session errors","session warnings","история уведомлений","ошибки сессии"] },
] as const satisfies readonly SettingsSearchEntry[];
