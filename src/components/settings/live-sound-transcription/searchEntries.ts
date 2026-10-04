import type { SettingsSearchEntry } from "../settingsSearchTypes";
import {
  legacyLiveSttSelection,
  sttModelCapabilities,
  sttSupports,
} from "../../../lib/sttModelSelection";

const liveSelectionSupportsDiarization = (
  settings: Parameters<NonNullable<SettingsSearchEntry["isAvailable"]>>[0],
): boolean => {
  if (!settings) return true;

  const storedSelection = settings.live_sound_model_selection;
  const selection =
    storedSelection && sttModelCapabilities(storedSelection).workflows.includes("live")
      ? storedSelection
      : legacyLiveSttSelection(settings);

  return selection ? sttSupports(selection, "diarization", "live") : false;
};

export const liveSoundSearchEntries = [
  { id: "live-model", section: "liveSoundTranscription", anchor: "live-monitor-session-settings", expandAnchor: "live-monitor-session-settings", labelKey: "settingsSearch.items.liveModel", fallbackLabel: "Live Monitor model and session settings", groupLabelKey: "settingsSearch.groups.session", groupFallbackLabel: "Session", keywords: ["live monitor", "computer audio", "model", "session", "живой монитор"] },
  { id: "live-diarization", section: "liveSoundTranscription", anchor: "live-monitor-diarization", expandAnchor: "live-monitor-session-settings", labelKey: "settingsSearch.items.liveDiarization", fallbackLabel: "Live Monitor speaker diarization", groupLabelKey: "settingsSearch.groups.session", groupFallbackLabel: "Session", unavailableReasonKey: "settingsSearch.unavailable.diarization", unavailableReasonFallback: "Select a model or provider that supports speaker diarization.", isAvailable: liveSelectionSupportsDiarization, keywords: ["diarization", "speaker", "live", "диаризация", "спикер"] },
  { id: "live-session", section: "liveSoundTranscription", anchor: "settings-live-session", labelKey: "settings.liveSoundTranscription.title", fallbackLabel: "Live Monitor", keywords: ["live monitor session","start monitoring","stop monitoring","live session","начать мониторинг","остановить мониторинг"] },
  { id: "live-transcript", section: "liveSoundTranscription", anchor: "settings-live-transcript", labelKey: "settings.liveSoundTranscription.transcript.title", fallbackLabel: "Transcript", keywords: ["live transcript","copy transcript","save transcript","clear transcript","живой транскрипт","сохранить транскрипт"] },
  { id: "live-endpoint-settings", section: "liveSoundTranscription", anchor: "settings-live-endpoint-settings", labelKey: "settings.liveSoundTranscription.sessionOverrides.title", fallbackLabel: "Session Settings", isAvailable: liveSelectionSupportsDiarization, unavailableReasonKey: "settingsSearch.unavailable.diarization", unavailableReasonFallback: "Select a model or provider that supports speaker diarization.", keywords: ["endpoint detection","endpoint delay","endpointing","live endpoint","определение конца речи","задержка окончания"] },
  { id: "live-speaker-names", section: "liveSoundTranscription", anchor: "settings-live-speaker-names", labelKey: "settings.liveSoundTranscription.speakerNames.title", fallbackLabel: "Speaker Names", isAvailable: liveSelectionSupportsDiarization, unavailableReasonKey: "settingsSearch.unavailable.diarization", unavailableReasonFallback: "Select a model or provider that supports speaker diarization.", keywords: ["speaker names","speaker name sets","rename speakers","speaker profiles","имена спикеров","набор имен","переименовать спикеров"] },
] as const satisfies readonly SettingsSearchEntry[];
