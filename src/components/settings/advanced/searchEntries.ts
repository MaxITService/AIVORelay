import { type as getOsType } from "@tauri-apps/plugin-os";
import type { SettingsSearchEntry } from "../settingsSearchTypes";

const isWindows = (() => {
  try {
    return getOsType() === "windows";
  } catch {
    return false;
  }
})();

export const advancedSearchEntries = [
  { id: "advanced", section: "advanced", labelKey: "settingsSearch.items.advanced", fallbackLabel: "Advanced application settings", keywords: ["advanced", "behavior", "startup", "webview", "memory", "ram", "headless", "speech only", "dictation", "расширенные", "запуск", "память", "без интерфейса", "только диктовка"] },
  { id: "speech-only-low-memory", section: "advanced", anchor: "speech-only-low-memory-settings", labelKey: "settings.advanced.neverLaunchWebview.label", fallbackLabel: "Start without WebView (speech only)", groupLabelKey: "settings.advanced.neverLaunchWebview.groupTitle", groupFallbackLabel: "Speech-only low-memory mode", keywords: ["memory", "low memory", "low-memory", "ram", "webview", "without webview", "no webview", "speech only", "speech-only", "dictation only", "headless", "minimal memory", "память", "низкое потребление памяти", "экономия памяти", "без webview", "без интерфейса", "только диктовка"] },
  { id: "paste-method-delay", section: "advanced", anchor: "advanced-paste-settings", labelKey: "settings.advanced.pasteMethod.title", fallbackLabel: "Paste method and delay", keywords: ["paste", "paste method", "paste delay", "clipboard delay", "ctrl v", "shift insert", "direct input", "вставка", "метод вставки", "задержка вставки", "буфер обмена"] },
  { id: "start-hidden", section: "advanced", anchor: "settings-start-hidden", labelKey: "settings.advanced.startHidden.label", fallbackLabel: "Start Hidden", keywords: ["minimized","start in tray","hidden startup","скрытый запуск","запуск в трее"] },
  { id: "autostart", section: "advanced", anchor: "settings-autostart", labelKey: "settings.advanced.autostart.label", fallbackLabel: "Launch on Startup", keywords: ["autostart","startup","login","sign in","автозапуск","вход в систему"] },
  { id: "admin-autostart", section: "advanced", anchor: "settings-autostart", labelKey: "settings.advanced.autostartAsAdmin.label", fallbackLabel: "Autostart with Administrator Privileges", unavailableReasonKey: "settingsSearch.unavailable.windowsOnly", unavailableReasonFallback: "Available on Windows only.", isAvailable: () => isWindows, keywords: ["administrator","elevated","uac","admin windows","администратор","повышенные права"] },
  { id: "auto-submit", section: "advanced", anchor: "settings-auto-submit", labelKey: "settings.advanced.autoSubmit.title", fallbackLabel: "Auto Submit", keywords: ["enter after paste","submit","send automatically","автоматическая отправка","нажать enter"] },
  { id: "recording-auto-stop", section: "advanced", anchor: "settings-recording-auto-stop", labelKey: "settings.advanced.autoStop.title", fallbackLabel: "Recording Auto-Stop Safety", keywords: ["auto stop","automatic stop","recording safety","автостоп","автоматическая остановка"] },
  { id: "recording-auto-stop-timeout", section: "advanced", anchor: "settings-recording-auto-stop-timeout", fallbackAnchor: "settings-recording-auto-stop", labelKey: "settings.advanced.autoStop.timeoutTitle", fallbackLabel: "Auto-Stop After", keywords: ["recording timeout","time limit","maximum recording","лимит записи","таймаут записи"] },
  { id: "recording-auto-stop-paste", section: "advanced", anchor: "settings-recording-auto-stop-paste", fallbackAnchor: "settings-recording-auto-stop", labelKey: "settings.advanced.autoStop.pasteTitle", fallbackLabel: "Paste Transcribed Text", keywords: ["auto stop paste","safety stop output","вставка после автостопа","текст после остановки"] },
] as const satisfies readonly SettingsSearchEntry[];
