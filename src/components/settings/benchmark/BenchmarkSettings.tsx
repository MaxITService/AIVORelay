import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { useTranslation } from "react-i18next";
import {
  Mic,
  Square,
  Play,
  SlidersHorizontal,
  Trash2,
  Volume2,
  Wind,
  X,
  AlertTriangle,
  Sparkles,
  FileText,
  FileAudio,
  Clock3,
} from "lucide-react";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { SettingContainer } from "@/components/ui/SettingContainer";
import { Button } from "@/components/ui/Button";
import { Dropdown, type DropdownOption } from "@/components/ui/Dropdown";
import { AudioPlayer, AudioPlayerGroup } from "@/components/ui/AudioPlayer";
import { CopyFeedbackIcon } from "@/components/ui/CopyFeedbackIcon";
import { SttModelSelector } from "@/components/settings/SttModelSelector";
import { useSettings } from "@/hooks/useSettings";
import { useModels } from "@/hooks/useModels";
import { useNavigationStore } from "@/stores/navigationStore";
import {
  BOOST_SWEEP_DB_LEVELS,
  VAD_SWEEP_THRESHOLDS,
  useBenchmarkStore,
  type BenchmarkRunResult,
} from "@/stores/benchmarkStore";
import { globalSttSelection } from "@/lib/sttModelSelection";
import { sessionToast as toast } from "@/lib/sessionToast";
import { formatDateTime } from "@/utils/dateFormat";

const formatElapsedSeconds = (seconds: number): string => {
  const total = Math.max(0, Math.floor(seconds));
  const mins = Math.floor(total / 60);
  const secs = total % 60;
  return `${mins}:${secs.toString().padStart(2, "0")}`;
};

const formatDurationMs = (durationMs: number): string =>
  `${(durationMs / 1000).toFixed(2)} s`;

interface ResultRowProps {
  item: BenchmarkRunResult;
  clipExists: boolean;
  onSelectClip: (clipId: string) => void;
  onRemove: (id: string) => void;
}

const BenchmarkResultRow: React.FC<ResultRowProps> = ({
  item,
  clipExists,
  onSelectClip,
  onRemove,
}) => {
  const { t, i18n } = useTranslation();
  const [showSettings, setShowSettings] = useState(false);
  const [showRaw, setShowRaw] = useState(false);
  const [copied, setCopied] = useState(false);
  const [copyBurst, setCopyBurst] = useState(0);
  const copyTimerRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (copyTimerRef.current !== null) {
        window.clearTimeout(copyTimerRef.current);
      }
    };
  }, []);

  const hasRawDiff =
    item.rawText !== null &&
    item.rawText.trim().length > 0 &&
    item.rawText !== item.text;
  const displayText = hasRawDiff && showRaw ? (item.rawText ?? item.text) : item.text;
  const hasText = displayText.trim().length > 0;
  const formattedTime = formatDateTime(
    String(Math.floor(item.timestampMs / 1000)),
    i18n.language,
  );
  const clipBadgeLabel = item.clipName || t("benchmark.results.unknownClip", "Clip");

  const handleCopy = async () => {
    if (!hasText) return;
    try {
      await writeText(displayText);
      if (copyTimerRef.current !== null) {
        window.clearTimeout(copyTimerRef.current);
      }
      setCopied(true);
      setCopyBurst((n) => n + 1);
      copyTimerRef.current = window.setTimeout(() => {
        setCopied(false);
        copyTimerRef.current = null;
      }, 2000);
    } catch (error) {
      console.error("Failed to copy benchmark text:", error);
      toast.error(t("settings.history.copyError", "Failed to copy text"));
    }
  };

  return (
    <div className="px-4 py-3 flex flex-col gap-2.5">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-wrap items-center gap-2 min-w-0">
          {clipExists ? (
            <button
              type="button"
              onClick={() => onSelectClip(item.clipId)}
              title={t("benchmark.results.selectClipTooltip", "Select this test clip")}
              className="inline-flex items-center gap-1 text-xs rounded border border-[#9b5de5]/35 bg-[#9b5de5]/15 px-2 py-0.5 text-[#d7b8ff] hover:bg-[#9b5de5]/25 transition-colors cursor-pointer"
            >
              <FileAudio className="w-3 h-3 shrink-0" />
              <span>
                {clipBadgeLabel} · {item.clipDurationSeconds.toFixed(1)} s
              </span>
            </button>
          ) : (
            <span
              className="inline-flex items-center gap-1 text-xs rounded border border-white/[0.08] bg-white/[0.04] px-2 py-0.5 text-[#888888]"
              title={t("benchmark.results.deletedClipTooltip", "The source audio clip was deleted")}
            >
              <FileAudio className="w-3 h-3 shrink-0" />
              <span>
                {t("benchmark.results.deletedClipBadge", {
                  defaultValue: "{{name}} (deleted)",
                  name: clipBadgeLabel,
                })}
              </span>
            </span>
          )}

          <span className="text-sm font-medium text-[#f5f5f5]">
            {item.label}
          </span>

          <span className="text-xs rounded bg-white/[0.06] px-2 py-0.5 text-[#c8c8c8]">
            {item.providerLabel}
            {item.modelLabel ? ` · ${item.modelLabel}` : ""}
          </span>

          {item.llmPostProcessed && (
            <span
              className="inline-flex items-center gap-1 text-xs rounded border border-[#ff4d8d]/35 bg-[#ff4d8d]/15 px-2 py-0.5 text-[#ff8ebb] font-medium"
              title={t(
                "benchmark.results.llmBadgeTooltip",
                "Processed with the profile's LLM post-processing pipeline",
              )}
            >
              <Sparkles className="w-3 h-3 shrink-0" />
              <span>LLM</span>
            </span>
          )}

          <span className="text-xs font-mono text-[#9a9a9a]">
            {formatDurationMs(item.durationMs)}
          </span>
          <span className="text-xs text-[#707070]">{formattedTime}</span>
        </div>

        <div className="flex items-center gap-1.5 shrink-0">
          {hasRawDiff && (
            <button
              type="button"
              onClick={() => setShowRaw((raw) => !raw)}
              title={
                showRaw
                  ? t("settings.history.showProcessedText", "Show processed text")
                  : t("settings.history.showRawTranscript", "Show raw transcript")
              }
              className={`p-1.5 rounded transition-colors cursor-pointer ${
                showRaw
                  ? "text-[#ff8ebb] bg-[#ff4d8d]/15"
                  : "text-[#b8b8b8] hover:text-[#f5f5f5] hover:bg-white/[0.06]"
              }`}
            >
              <FileText className="w-4 h-4" />
            </button>
          )}
          <Button
            type="button"
            variant={showSettings ? "primary" : "secondary"}
            size="sm"
            onClick={() => setShowSettings((open) => !open)}
            className="flex items-center gap-1.5"
          >
            <SlidersHorizontal className="w-3.5 h-3.5" />
            <span>
              {showSettings
                ? t("benchmark.results.hideSettings", "Hide settings")
                : t("benchmark.results.showSettings", "Show settings")}
            </span>
          </Button>
          <button
            type="button"
            onClick={() => void handleCopy()}
            disabled={!hasText}
            title={t("settings.history.copyToClipboard", "Copy to clipboard")}
            className="p-1.5 rounded text-[#b8b8b8] hover:text-[#f5f5f5] hover:bg-white/[0.06] disabled:opacity-35 disabled:cursor-not-allowed transition-colors cursor-pointer"
          >
            <CopyFeedbackIcon copied={copied} burstKey={copyBurst} />
          </button>
          <button
            type="button"
            onClick={() => onRemove(item.id)}
            title={t("settings.history.delete", "Delete")}
            className="p-1.5 rounded text-[#b8b8b8] hover:text-red-400 hover:bg-white/[0.06] transition-colors cursor-pointer"
          >
            <Trash2 className="w-4 h-4" />
          </button>
        </div>
      </div>

      {item.error ? (
        <div className="flex items-start gap-2 rounded-md border border-red-500/25 bg-red-500/10 px-3 py-2 text-xs text-red-200">
          <AlertTriangle className="w-4 h-4 shrink-0 text-red-400 mt-0.5" />
          <span className="break-words">{item.error}</span>
        </div>
      ) : hasText ? (
        <p className="italic text-sm text-[#e8e8e8] select-text cursor-text whitespace-pre-wrap break-words">
          {displayText}
        </p>
      ) : (
        <p className="italic text-sm text-[#787878]">
          {t("benchmark.results.empty", "(No speech detected)")}
        </p>
      )}

      {showSettings && (
        <pre className="rounded-md border border-white/[0.08] bg-[#101010]/85 px-3 py-2.5 text-xs font-mono text-[#d0d0d0] whitespace-pre-wrap break-words select-text overflow-x-auto">
          {item.settingsText}
        </pre>
      )}
    </div>
  );
};

export const BenchmarkSettings: React.FC = () => {
  const { t, i18n } = useTranslation();
  const { settings } = useSettings();
  const { models } = useModels();
  const setSection = useNavigationStore((state) => state.setSection);

  const {
    clips,
    selectedClipId,
    isRecording,
    recordingStarting,
    recordingStartedAt,
    targetMode,
    selectedProfileId,
    customSelection,
    isRunning,
    cancelling,
    progress,
    results,
    resultsFilter,
    lastError,
    syncState,
    selectClip,
    startRecording,
    stopRecording,
    deleteClip,
    setTargetMode,
    setSelectedProfileId,
    setCustomSelection,
    setResultsFilter,
    runCurrent,
    runBoostSweep,
    runNoiseSweep,
    cancelRun,
    removeResult,
    clearResults,
  } = useBenchmarkStore();

  const [recordingElapsed, setRecordingElapsed] = useState(0);

  useEffect(() => {
    void syncState();
  }, [syncState]);

  useEffect(() => {
    if (!isRecording || !recordingStartedAt) {
      setRecordingElapsed(0);
      return;
    }
    setRecordingElapsed((Date.now() - recordingStartedAt) / 1000);
    const id = window.setInterval(() => {
      setRecordingElapsed((Date.now() - recordingStartedAt) / 1000);
    }, 200);
    return () => window.clearInterval(id);
  }, [isRecording, recordingStartedAt]);

  useEffect(() => {
    if (targetMode === "custom" && !customSelection && settings) {
      setCustomSelection(globalSttSelection(settings));
    }
  }, [customSelection, setCustomSelection, settings, targetMode]);

  const selectedClip = useMemo(
    () => clips.find((clip) => clip.id === selectedClipId) ?? null,
    [clips, selectedClipId],
  );

  const clipIdsSet = useMemo(
    () => new Set(clips.map((clip) => clip.id)),
    [clips],
  );

  const resultCountByClip = useMemo(() => {
    const counts = new Map<string, number>();
    for (const item of results) {
      counts.set(item.clipId, (counts.get(item.clipId) ?? 0) + 1);
    }
    return counts;
  }, [results]);

  const selectedClipResultCount = selectedClip
    ? (resultCountByClip.get(selectedClip.id) ?? 0)
    : 0;

  const visibleResults = useMemo(() => {
    if (resultsFilter === "all" || !selectedClip) {
      return results;
    }
    return results.filter((item) => item.clipId === selectedClip.id);
  }, [results, resultsFilter, selectedClip]);

  const activeProfileId = settings?.active_profile_id || "default";
  const profiles = settings?.transcription_profiles ?? [];
  const activeProfileName = useMemo(() => {
    if (activeProfileId === "default") {
      return t("benchmark.target.defaultProfile", "Default");
    }
    return (
      profiles.find((p) => p.id === activeProfileId)?.name || activeProfileId
    );
  }, [activeProfileId, profiles, t]);

  const targetDropdownOptions = useMemo<DropdownOption[]>(() => {
    return [
      {
        value: "active",
        label: t("benchmark.target.currentProfile", {
          defaultValue: "Current profile ({{name}})",
          name: activeProfileName,
        }),
      },
      {
        value: "profile:default",
        label: t("benchmark.target.profileOption", {
          defaultValue: "Profile: {{name}}",
          name: t("benchmark.target.defaultProfile", "Default"),
        }),
      },
      ...profiles.map((profile) => ({
        value: `profile:${profile.id}`,
        label: t("benchmark.target.profileOption", {
          defaultValue: "Profile: {{name}}",
          name: profile.name,
        }),
      })),
      {
        value: "custom",
        label: t("benchmark.target.customProvider", "Other provider / model…"),
      },
    ];
  }, [activeProfileName, profiles, t]);

  const selectedDropdownValue =
    targetMode === "active"
      ? "active"
      : targetMode === "custom"
        ? "custom"
        : `profile:${selectedProfileId ?? "default"}`;

  const handleTargetSelect = (value: string) => {
    if (value === "active") {
      setTargetMode("active");
      setSelectedProfileId(null);
    } else if (value === "custom") {
      setTargetMode("custom");
      if (!customSelection && settings) {
        setCustomSelection(globalSttSelection(settings));
      }
    } else if (value.startsWith("profile:")) {
      setTargetMode("profile");
      setSelectedProfileId(value.slice("profile:".length));
    }
  };

  const loadSelectedClipAudioUrl = useCallback(async (): Promise<string | null> => {
    if (!selectedClip) return null;
    try {
      const bytes = await invoke<number[]>("benchmark_get_clip_wav", {
        clipId: selectedClip.id,
      });
      const blob = new Blob([new Uint8Array(bytes)], { type: "audio/wav" });
      return URL.createObjectURL(blob);
    } catch (error) {
      console.error("Failed to load benchmark audio:", error);
      return null;
    }
  }, [selectedClip]);

  const currentMicLabel =
    settings?.selected_microphone && settings.selected_microphone !== "default"
      ? settings.selected_microphone
      : t("benchmark.recording.defaultMic", "Default");

  const currentBoostDb = useMemo(() => {
    const boostMap = ((settings as any)?.microphone_input_boost_db_by_device ??
      {}) as Record<string, number>;
    const deviceKey =
      selectedClip?.deviceName && selectedClip.deviceName !== "Default"
        ? selectedClip.deviceName
        : settings?.selected_microphone &&
            settings.selected_microphone !== "default"
          ? settings.selected_microphone
          : "__default__";
    return (
      boostMap[deviceKey] ??
      ((settings as any)?.microphone_input_boost_db as number | undefined) ??
      0
    );
  }, [selectedClip?.deviceName, settings]);

  const currentNoiseCancellation = Boolean(
    settings?.microphone_noise_cancellation_enabled,
  );
  const currentFilterSilence = settings?.filter_silence ?? true;
  const currentVadThreshold = settings?.vad_threshold ?? 0.3;

  const effectiveLlmEnabled = useMemo(() => {
    if (!settings) return false;
    const effectiveProfileId =
      targetMode === "profile"
        ? (selectedProfileId ?? "default")
        : activeProfileId;
    if (effectiveProfileId !== "default") {
      const profile = profiles.find((p) => p.id === effectiveProfileId);
      if (profile) {
        return Boolean(profile.llm_post_process_enabled);
      }
    }
    return Boolean(settings.post_process_enabled);
  }, [activeProfileId, profiles, selectedProfileId, settings, targetMode]);

  const recordingBusy = isRecording || recordingStarting;
  const isBusy = recordingBusy || isRunning;
  const canRun = Boolean(selectedClip) && !isBusy;

  const handleRunCurrent = () => {
    const label =
      targetMode === "active"
        ? t("benchmark.runLabels.currentProfile", {
            defaultValue: "Current profile ({{name}})",
            name: activeProfileName,
          })
        : targetMode === "profile"
          ? t("benchmark.runLabels.profile", {
              defaultValue: "Profile: {{name}}",
              name:
                selectedProfileId === "default" || !selectedProfileId
                  ? t("benchmark.target.defaultProfile", "Default")
                  : profiles.find((p) => p.id === selectedProfileId)?.name ||
                    selectedProfileId,
            })
          : t("benchmark.runLabels.custom", "Custom provider");
    void runCurrent(label);
  };

  const handleRunBoostSweep = () => {
    void runBoostSweep((db) =>
      db <= 0
        ? t("benchmark.runLabels.boostOff", "Boost: Off (0 dB)")
        : t("benchmark.runLabels.boostDb", {
            defaultValue: "Boost: +{{db}} dB",
            db,
          }),
    );
  };

  const handleRunNoiseSweep = () => {
    void runNoiseSweep(settings, {
      rnnoiseOff: t(
        "benchmark.runLabels.rnnoiseOff",
        "Noise: RNNoise Off · Filter Silence Off",
      ),
      rnnoiseOn: t(
        "benchmark.runLabels.rnnoiseOn",
        "Noise: RNNoise On · Filter Silence Off",
      ),
      vadLevel: (threshold) =>
        t("benchmark.runLabels.vadLevel", {
          defaultValue: "Noise: VAD {{threshold}}",
          threshold: threshold.toFixed(1),
        }),
    });
  };

  const handleClearVisibleResults = () => {
    if (resultsFilter === "selected" && selectedClip) {
      void clearResults(selectedClip.id);
    } else {
      void clearResults(null);
    }
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6 pb-12">
      <SettingsGroup
        id="settings-benchmark-recording"
        title={t("benchmark.recording.title", "1. Test Recordings")}
        description={t(
          "benchmark.recording.description",
          "Record raw microphone clips (without boost, noise cancellation, or silence filtering). Clips are saved on disk until you delete them.",
        )}
      >
        <div className="px-4 py-4 space-y-4">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="flex flex-wrap items-center gap-2">
              {isRecording ? (
                <Button
                  type="button"
                  variant="danger"
                  size="md"
                  onClick={() => void stopRecording()}
                  className="flex items-center gap-2"
                >
                  <Square className="w-4 h-4 fill-current" />
                  <span>
                    {t("benchmark.recording.stop", "Stop Recording")} (
                    {formatElapsedSeconds(recordingElapsed)})
                  </span>
                </Button>
              ) : (
                <Button
                  type="button"
                  variant="primary"
                  size="md"
                  disabled={isRunning || recordingStarting}
                  onClick={() => void startRecording()}
                  className="flex items-center gap-2"
                >
                  <Mic className="w-4 h-4" />
                  <span>
                    {t("benchmark.recording.start", "Record New Clip")}
                  </span>
                </Button>
              )}
            </div>

            <div className="text-xs text-[#9a9a9a]">
              {t("benchmark.recording.microphoneLabel", "Microphone:")}{" "}
              <button
                type="button"
                onClick={() => setSection("general")}
                className="text-[#ff8ebb] hover:underline cursor-pointer"
              >
                {currentMicLabel}
              </button>
            </div>
          </div>

          {clips.length > 0 && (
            <div className="space-y-2">
              <div className="text-xs text-[#9a9a9a]">
                {t("benchmark.recording.savedClips", {
                  defaultValue: "Saved clips ({{count}}):",
                  count: clips.length,
                })}
              </div>
              <div className="space-y-1.5 max-h-56 overflow-y-auto pr-1">
                {clips.map((clip) => {
                  const isSelected = clip.id === selectedClipId;
                  const runCount = resultCountByClip.get(clip.id) ?? 0;
                  const clipTime = formatDateTime(
                    String(Math.floor(clip.recordedAtMs / 1000)),
                    i18n.language,
                  );
                  return (
                    <div
                      key={clip.id}
                      role="button"
                      tabIndex={0}
                      onClick={() => selectClip(clip.id)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter" || e.key === " ") {
                          e.preventDefault();
                          selectClip(clip.id);
                        }
                      }}
                      className={`flex items-center justify-between gap-2 rounded-lg border px-3 py-2 transition-colors cursor-pointer ${
                        isSelected
                          ? "border-[#9b5de5]/70 bg-[#9b5de5]/15"
                          : "border-white/[0.06] bg-white/[0.02] hover:bg-white/[0.04]"
                      }`}
                    >
                      <div className="flex flex-wrap items-center gap-x-2.5 gap-y-0.5 min-w-0">
                        <FileAudio
                          className={`w-4 h-4 shrink-0 ${
                            isSelected ? "text-[#d7b8ff]" : "text-[#8a8a8a]"
                          }`}
                        />
                        <span className="text-sm font-medium text-[#f5f5f5]">
                          {clip.name}
                        </span>
                        <span className="text-xs font-mono text-[#c8c8c8]">
                          {clip.durationSeconds.toFixed(1)} s
                        </span>
                        <span className="text-xs text-[#8a8a8a] truncate">
                          {clip.deviceName}
                        </span>
                        <span className="text-xs text-[#707070]">
                          {clipTime}
                        </span>
                        {runCount > 0 && (
                          <span className="text-[11px] rounded bg-white/[0.07] px-1.5 py-0.5 text-[#c8c8c8]">
                            {t("benchmark.recording.runsBadge", {
                              defaultValue: "{{count}} runs",
                              count: runCount,
                            })}
                          </span>
                        )}
                      </div>

                      <button
                        type="button"
                        disabled={isBusy}
                        onClick={(e) => {
                          e.stopPropagation();
                          void deleteClip(clip.id);
                        }}
                        title={t(
                          "benchmark.recording.deleteClip",
                          "Delete clip (results remain in list)",
                        )}
                        className="p-1.5 rounded text-[#9a9a9a] hover:text-red-400 hover:bg-white/[0.06] disabled:opacity-35 disabled:cursor-not-allowed transition-colors cursor-pointer shrink-0"
                      >
                        <Trash2 className="w-4 h-4" />
                      </button>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {selectedClip && (
            <div className="space-y-2 pt-1 border-t border-white/[0.06]">
              <div className="text-xs text-[#a0a0a0]">
                {t("benchmark.recording.selectedClipSummary", {
                  defaultValue: "Selected: {{name}} · {{seconds}} s from {{device}}",
                  name: selectedClip.name,
                  seconds: selectedClip.durationSeconds.toFixed(1),
                  device: selectedClip.deviceName,
                })}
              </div>
              <AudioPlayerGroup>
                <AudioPlayer
                  key={selectedClip.id}
                  onLoadRequest={loadSelectedClipAudioUrl}
                  initialDurationSeconds={selectedClip.durationSeconds}
                  className="w-full"
                />
              </AudioPlayerGroup>
            </div>
          )}
        </div>
      </SettingsGroup>

      <SettingsGroup
        id="settings-benchmark-run"
        title={t("benchmark.run.title", "2. Run Benchmark")}
        description={t(
          "benchmark.run.description",
          "Run the selected clip through the standard transcription pipeline (including profile LLM post-processing if enabled), or sweep all boost and noise-removal levels.",
        )}
      >
        <SettingContainer
          title={t("benchmark.target.title", "Profile / Provider")}
          description={t(
            "benchmark.target.description",
            "Choose the current active profile, another saved profile, or any configured provider and model.",
          )}
          descriptionMode="inline"
          layout="stacked"
          grouped={true}
        >
          <div className="space-y-3">
            <Dropdown
              options={targetDropdownOptions}
              selectedValue={selectedDropdownValue}
              onSelect={handleTargetSelect}
              disabled={recordingBusy}
              className="w-full"
            />
            {targetMode === "custom" && (
              <SttModelSelector
                workflow="dictation"
                selection={customSelection}
                localModels={models}
                onChange={(next) => setCustomSelection(next)}
                disabled={recordingBusy}
              />
            )}
            {isRunning && (
              <p className="text-xs text-[#b8b8b8]">
                {t("benchmark.run.backgroundNote")}
              </p>
            )}
          </div>
        </SettingContainer>

        <div className="px-4 py-4 space-y-3">
          <div className="text-xs text-[#9a9a9a] flex flex-wrap items-center gap-x-2 gap-y-1">
            <span>
              {t("benchmark.run.currentAudioSummary", "Current settings:")}
            </span>
            <button
              type="button"
              onClick={() => setSection("general")}
              className="text-[#ff8ebb] hover:underline cursor-pointer"
            >
              {currentBoostDb <= 0
                ? t("benchmark.run.boostOffChip", "Boost Off")
                : t("benchmark.run.boostOnChip", {
                    defaultValue: "Boost +{{db}} dB",
                    db: currentBoostDb.toFixed(0),
                  })}
              {" · "}
              {currentNoiseCancellation
                ? t("benchmark.run.rnnoiseOnChip", "RNNoise On")
                : t("benchmark.run.rnnoiseOffChip", "RNNoise Off")}
            </button>
            <span>·</span>
            <button
              type="button"
              onClick={() => setSection("audioProcessing")}
              className="text-[#ff8ebb] hover:underline cursor-pointer"
            >
              {currentFilterSilence
                ? t("benchmark.run.vadOnChip", {
                    defaultValue: "VAD {{threshold}}",
                    threshold: currentVadThreshold.toFixed(1),
                  })
                : t("benchmark.run.vadOffChip", "Filter Silence Off")}
            </button>
            <span>·</span>
            <button
              type="button"
              onClick={() => setSection("postprocessing")}
              className="text-[#ff8ebb] hover:underline cursor-pointer"
            >
              {effectiveLlmEnabled
                ? t("benchmark.run.llmOnChip", "LLM Post-Processing On")
                : t("benchmark.run.llmOffChip", "LLM Post-Processing Off")}
            </button>
          </div>

          <p className="flex items-start gap-2 text-xs text-[#b8b8b8]">
            <Clock3 className="mt-0.5 h-3.5 w-3.5 shrink-0" aria-hidden="true" />
            <span>{t("benchmark.run.liveDurationNote")}</span>
          </p>

          <div className="flex flex-wrap items-center gap-2">
            <Button
              type="button"
              variant="primary"
              size="md"
              disabled={!canRun}
              onClick={handleRunCurrent}
              className="flex items-center gap-2"
            >
              <Play className="w-4 h-4" />
              <span>
                {t("benchmark.run.runCurrent", "Benchmark Current Settings")}
              </span>
            </Button>

            <Button
              type="button"
              variant="secondary"
              size="md"
              disabled={!canRun}
              onClick={handleRunBoostSweep}
              className="flex items-center gap-2"
              title={t("benchmark.run.boostSweepTooltip", {
                defaultValue:
                  "Run all {{count}} microphone boost levels (0 to +17 dB, step 1 dB)",
                count: BOOST_SWEEP_DB_LEVELS.length,
              })}
            >
              <Volume2 className="w-4 h-4" />
              <span>
                {t("benchmark.run.boostSweep", "Auto: All Boost Levels (0–17 dB)")}
              </span>
            </Button>

            <Button
              type="button"
              variant="secondary"
              size="md"
              disabled={!canRun}
              onClick={handleRunNoiseSweep}
              className="flex items-center gap-2"
              title={t("benchmark.run.noiseSweepTooltip", {
                defaultValue:
                  "Run RNNoise Off/On and all voice detection sensitivity levels ({{min}}–{{max}})",
                min: VAD_SWEEP_THRESHOLDS[0].toFixed(1),
                max: VAD_SWEEP_THRESHOLDS[VAD_SWEEP_THRESHOLDS.length - 1].toFixed(1),
              })}
            >
              <Wind className="w-4 h-4" />
              <span>
                {t("benchmark.run.noiseSweep", "Auto: All Noise Removal Levels")}
              </span>
            </Button>
          </div>

          {isRunning && progress && (
            <div className="rounded-md border border-white/[0.08] bg-white/[0.02] px-3 py-2.5 space-y-2">
              <div className="flex items-center justify-between gap-3">
                <span className="text-xs text-[#e0e0e0]">
                  {t("benchmark.run.progress", {
                    defaultValue: "Running {{current}} / {{total}} — {{step}}",
                    current: progress.current,
                    total: progress.total,
                    step: progress.stepLabel,
                  })}
                </span>
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  disabled={cancelling}
                  onClick={() => void cancelRun()}
                  className="flex items-center gap-1"
                >
                  <X className="w-3.5 h-3.5" />
                  <span>
                    {cancelling
                      ? t("benchmark.run.cancelling", "Cancelling…")
                      : t("common.cancel", "Cancel")}
                  </span>
                </Button>
              </div>
              <progress
                className="w-full h-1.5 accent-[#9b5de5]"
                value={progress.current}
                max={progress.total}
              />
            </div>
          )}

          {lastError && (
            <div className="flex items-start gap-2 rounded-md border border-red-500/25 bg-red-500/10 px-3 py-2 text-xs text-red-200">
              <AlertTriangle className="w-4 h-4 shrink-0 text-red-400 mt-0.5" />
              <span className="break-words">{lastError}</span>
            </div>
          )}
        </div>
      </SettingsGroup>

      <SettingsGroup
        id="settings-benchmark-results"
        title={t("benchmark.results.title", "Results")}
        description={t(
          "benchmark.results.description",
          "Compare recognized text across runs. Click Show settings on any entry to view the exact parameters used.",
        )}
      >
        <div className="px-4 py-3 flex flex-wrap items-center justify-between gap-2">
          <div className="flex flex-wrap items-center gap-1.5">
            {selectedClip && (
              <Button
                type="button"
                variant={resultsFilter === "selected" ? "primary" : "secondary"}
                size="sm"
                onClick={() => setResultsFilter("selected")}
              >
                {t("benchmark.results.filterSelected", {
                  defaultValue: "{{name}} ({{count}})",
                  name: selectedClip.name,
                  count: selectedClipResultCount,
                })}
              </Button>
            )}
            <Button
              type="button"
              variant={
                resultsFilter === "all" || !selectedClip
                  ? "primary"
                  : "secondary"
              }
              size="sm"
              onClick={() => setResultsFilter("all")}
            >
              {t("benchmark.results.filterAll", {
                defaultValue: "All ({{count}})",
                count: results.length,
              })}
            </Button>
          </div>

          <Button
            type="button"
            variant="danger"
            size="sm"
            disabled={visibleResults.length === 0 || isRunning}
            onClick={handleClearVisibleResults}
            className="flex items-center gap-1.5"
          >
            <Trash2 className="w-3.5 h-3.5" />
            <span>
              {resultsFilter === "selected" && selectedClip
                ? t("benchmark.results.clearSelected", "Clear Clip Results")
                : t("benchmark.results.clearAll", "Clear All Results")}
            </span>
          </Button>
        </div>

        {visibleResults.length === 0 ? (
          <div className="px-4 py-8 text-center text-sm text-[#787878]">
            {selectedClip && resultsFilter === "selected" && results.length > 0
              ? t(
                  "benchmark.results.noResultsForClip",
                  "No benchmark results for this clip yet.",
                )
              : t(
                  "benchmark.results.noResults",
                  "No benchmark results yet. Record a clip above and run a benchmark.",
                )}
          </div>
        ) : (
          visibleResults.map((item) => (
            <BenchmarkResultRow
              key={item.id}
              item={item}
              clipExists={clipIdsSet.has(item.clipId)}
              onSelectClip={(clipId) => {
                selectClip(clipId);
              }}
              onRemove={(id) => void removeResult(id)}
            />
          ))
        )}
      </SettingsGroup>
    </div>
  );
};

export default BenchmarkSettings;
