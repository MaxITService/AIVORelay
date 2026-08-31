import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const liveSoundSearchEntries = [
  { id: "live-model", section: "liveSoundTranscription", anchor: "live-monitor-session-settings", expandAnchor: "live-monitor-session-settings", labelKey: "settingsSearch.items.liveModel", fallbackLabel: "Live Monitor model and session settings", groupLabelKey: "settingsSearch.groups.session", groupFallbackLabel: "Session", keywords: ["live monitor", "computer audio", "model", "session", "живой монитор"] },
  { id: "live-diarization", section: "liveSoundTranscription", anchor: "live-monitor-diarization", expandAnchor: "live-monitor-session-settings", labelKey: "settingsSearch.items.liveDiarization", fallbackLabel: "Live Monitor speaker diarization", groupLabelKey: "settingsSearch.groups.session", groupFallbackLabel: "Session", keywords: ["diarization", "speaker", "live", "диаризация", "спикер"] },
] as const satisfies readonly SettingsSearchEntry[];
