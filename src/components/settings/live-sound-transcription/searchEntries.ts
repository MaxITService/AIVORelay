import type { SettingsSearchEntry } from "../settingsSearchTypes";
import {
  legacyLiveSttSelection,
  sttModelCapabilities,
  sttSupports,
  type SttModelSelection,
} from "../../../lib/sttModelSelection";

const liveSelectionSupportsDiarization = (
  settings: Parameters<NonNullable<SettingsSearchEntry["isAvailable"]>>[0],
): boolean => {
  if (!settings) return true;

  const storedSelection = (settings as any)
    .live_sound_model_selection as SttModelSelection | null | undefined;
  const selection =
    storedSelection && sttModelCapabilities(storedSelection).workflows.includes("live")
      ? storedSelection
      : legacyLiveSttSelection(settings);

  return sttSupports(selection, "diarization", "live");
};

export const liveSoundSearchEntries = [
  { id: "live-model", section: "liveSoundTranscription", anchor: "live-monitor-session-settings", expandAnchor: "live-monitor-session-settings", labelKey: "settingsSearch.items.liveModel", fallbackLabel: "Live Monitor model and session settings", groupLabelKey: "settingsSearch.groups.session", groupFallbackLabel: "Session", keywords: ["live monitor", "computer audio", "model", "session", "живой монитор"] },
  { id: "live-diarization", section: "liveSoundTranscription", anchor: "live-monitor-diarization", expandAnchor: "live-monitor-session-settings", labelKey: "settingsSearch.items.liveDiarization", fallbackLabel: "Live Monitor speaker diarization", groupLabelKey: "settingsSearch.groups.session", groupFallbackLabel: "Session", unavailableReasonKey: "settingsSearch.unavailable.diarization", unavailableReasonFallback: "Select a model or provider that supports speaker diarization.", isAvailable: liveSelectionSupportsDiarization, keywords: ["diarization", "speaker", "live", "диаризация", "спикер"] },
] as const satisfies readonly SettingsSearchEntry[];
