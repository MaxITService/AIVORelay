import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import {
  AlertCircle,
  AlertTriangle,
  Download,
  History,
  Loader2,
  Pause,
  Play,
  RefreshCw,
  RotateCcw,
  Trash2,
  Volume2,
} from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/Button";
import { ConfirmationModal } from "@/components/ui/ConfirmationModal";
import { Input } from "@/components/ui/Input";
import { SettingContainer } from "@/components/ui/SettingContainer";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { ToggleSwitch } from "@/components/ui/ToggleSwitch";
import { AIVORELAY_TTS_GUIDE_URL } from "@/lib/tts/ttsProviderMetadata";
import type { TtsLlmPreprocessingSettings } from "./TtsAiCleanup";
import { TtsHelpDisclosure } from "./TtsHelpDisclosure";
import { applyPlaybackRate } from "@/lib/utils/playbackRate";
import {
  prepareTtsPlaybackSource,
  type TtsPlaybackEffect,
} from "@/lib/utils/ttsPlaybackEffects";

type TtsProvider =
  | "soniox"
  | "deepgram"
  | "openai"
  | "edge"
  | "local_qwen"
  | "local_kokoro"
  | "windows";
type TtsOutputFormat = "mp3" | "wav";
export type TtsHistoryScope = "interactive" | "file";

export type TtsHistorySettingsSnapshot = {
  interactive_history_enabled?: boolean;
  interactive_history_max_entries?: number;
  interactive_history_max_storage_mb?: number;
  file_history_enabled?: boolean;
  file_history_max_entries?: number;
  file_history_max_storage_mb?: number;
  provider?: TtsProvider;
  soniox_model?: string;
  soniox_voice?: string;
  deepgram_model?: string;
  openai_model?: string;
  openai_voice?: string;
  edge_voice?: string;
  edge_voice_language?: string;
  local_qwen_voice?: string;
  local_qwen_language?: string;
  local_kokoro_voice?: string;
  local_kokoro_language?: string;
  windows_voice_id?: string;
  windows_voice_language?: string;
  openai_instructions?: string;
  selected_prompt_id?: string;
  output_format?: TtsOutputFormat;
  mp3_bitrate_kbps?: number;
  playback_pitch?: number;
  playback_effect?: TtsPlaybackEffect;
  prompt_presets?: Array<{
    id: string;
    name: string;
    instructions: string;
  }>;
  llm_preprocessing?: TtsLlmPreprocessingSettings;
};

export type TtsHistoryEntry = {
  id: number;
  timestamp: number;
  scope: TtsHistoryScope;
  group_id: string;
  source_text: string;
  source_kind?: "text" | "markdown";
  provider: TtsProvider;
  model: string;
  voice: string;
  language: string;
  output_format: TtsOutputFormat;
  managed_audio_filename: string;
  external_output_path?: string | null;
  prompt_preset_id?: string | null;
  prompt_preset_name?: string | null;
  resolved_instructions?: string | null;
  llm_cleanup_config?: string | null;
};

type TtsHistoryProps = {
  scope: TtsHistoryScope;
  tts: TtsHistorySettingsSnapshot;
  savingField: string | null;
  onAvailabilityChange?: (hasEntries: boolean) => void;
  updateTts: (
    patch: Partial<TtsHistorySettingsSnapshot>,
    field: string,
  ) => Promise<void>;
  flushPendingSettingsWrites: () => Promise<void>;
};

type Confirmation =
  | { kind: "delete"; entry: TtsHistoryEntry }
  | { kind: "regenerate"; entry: TtsHistoryEntry }
  | null;

type ActionMessage = {
  kind: "success" | "error";
  text: string;
} | null;

type HistoryGroup = {
  id: string;
  sourceText: string;
  timestamp: number;
  entries: TtsHistoryEntry[];
};

type RegenerateTtsHistoryResponse = {
  sourceEntryId: number;
  newEntry: TtsHistoryEntry;
  outputPath?: string | null;
  operationId: number;
  chunkCount: number;
  resumedChunks: number;
  processedCharacterCount: number;
};

type TtsHistoryDeleteOutcome = {
  id: number;
  record_deleted: boolean;
  managed_audio_status: "deleted" | "missing" | "failed";
  managed_audio_error?: string | null;
};

const asErrorMessage = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

const llmCleanupSummary = (entry: TtsHistoryEntry): string | null => {
  if (!entry.llm_cleanup_config) return null;
  try {
    const config = JSON.parse(entry.llm_cleanup_config) as Record<
      string,
      unknown
    >;
    const provider =
      typeof config.provider_id === "string" ? config.provider_id : "";
    const model = typeof config.model === "string" ? config.model : "";
    const prompt =
      typeof config.prompt_name === "string" ? config.prompt_name : "";
    return [provider, model, prompt].filter(Boolean).join(" · ") || null;
  } catch {
    return null;
  }
};

const formatPlaybackTime = (seconds: number) => {
  const safeSeconds =
    Number.isFinite(seconds) && seconds > 0 ? Math.floor(seconds) : 0;
  const minutes = Math.floor(safeSeconds / 60);
  return `${minutes}:${String(safeSeconds % 60).padStart(2, "0")}`;
};

const asBoundedInteger = (
  value: string,
  minimum: number,
  maximum: number,
  fallback: number,
) => {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.min(maximum, Math.max(minimum, Math.round(parsed)));
};

const providerLabel = (provider: TtsProvider) => {
  switch (provider) {
    case "soniox":
      return "Soniox";
    case "deepgram":
      return "Deepgram";
    case "openai":
      return "OpenAI";
    case "edge":
      return "Microsoft Read Aloud (unofficial)";
    case "local_qwen":
      return "Qwen3-TTS (Local)";
    case "local_kokoro":
      return "Kokoro 82M (Local)";
    case "windows":
      return "Windows voices";
  }
};

const providerUsesPaidApi = (provider: TtsProvider | undefined) =>
  provider === "soniox" || provider === "deepgram" || provider === "openai";

const modelForProvider = (
  tts: TtsHistorySettingsSnapshot,
  provider: TtsProvider,
) => {
  switch (provider) {
    case "soniox":
      return tts.soniox_model;
    case "deepgram":
      return tts.deepgram_model;
    case "openai":
      return tts.openai_model;
    case "edge":
      return "microsoft-edge-read-aloud";
    case "local_qwen":
      return "Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice";
    case "local_kokoro":
      return "k2-fsa/sherpa-onnx/kokoro-int8-multi-lang-v1_1";
    case "windows":
      return "windows.media.speechsynthesis";
  }
};

const voiceForProvider = (
  tts: TtsHistorySettingsSnapshot,
  provider: TtsProvider,
) => {
  switch (provider) {
    case "soniox":
      return tts.soniox_voice;
    case "deepgram":
      return tts.deepgram_model;
    case "openai":
      return tts.openai_voice;
    case "edge":
      return tts.edge_voice;
    case "local_qwen":
      return tts.local_qwen_voice;
    case "local_kokoro":
      return tts.local_kokoro_voice;
    case "windows":
      return tts.windows_voice_id;
  }
};

const normalizedTimestamp = (timestamp: number) =>
  timestamp > 0 && timestamp < 10_000_000_000 ? timestamp * 1_000 : timestamp;

const safeFilePart = (value: string) => {
  const normalized = value
    .normalize("NFKD")
    .replace(/[<>:"/\\|?*\u0000-\u001f]/g, "-")
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^[.\s-]+|[.\s-]+$/g, "");
  return normalized.slice(0, 60) || "tts";
};

const groupHistoryEntries = (entries: TtsHistoryEntry[]): HistoryGroup[] => {
  const byGroup = new Map<string, TtsHistoryEntry[]>();
  for (const entry of entries) {
    const groupId = entry.group_id.trim() || `legacy-${entry.id}`;
    const groupEntries = byGroup.get(groupId);
    if (groupEntries) groupEntries.push(entry);
    else byGroup.set(groupId, [entry]);
  }

  return Array.from(byGroup, ([id, variants]) => {
    variants.sort((left, right) => right.timestamp - left.timestamp);
    const latest = variants[0];
    return {
      id,
      sourceText: latest.source_text,
      timestamp: latest.timestamp,
      entries: variants,
    };
  }).sort((left, right) => right.timestamp - left.timestamp);
};

export const TtsHistory: React.FC<TtsHistoryProps> = ({
  scope,
  tts,
  savingField,
  onAvailabilityChange,
  updateTts,
  flushPendingSettingsWrites,
}) => {
  const { t, i18n } = useTranslation();
  const [entries, setEntries] = useState<TtsHistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [busyEntryId, setBusyEntryId] = useState<number | null>(null);
  const [activeEntryId, setActiveEntryId] = useState<number | null>(null);
  const [loadedEntryId, setLoadedEntryId] = useState<number | null>(null);
  const [playbackTime, setPlaybackTime] = useState(0);
  const [playbackDuration, setPlaybackDuration] = useState(0);
  const [confirmation, setConfirmation] = useState<Confirmation>(null);
  const [actionMessage, setActionMessage] = useState<ActionMessage>(null);
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const playbackAbortRef = useRef<AbortController | null>(null);
  const playbackObjectUrlRef = useRef<string | null>(null);
  const playbackPitchCompensationRef = useRef(1);
  const loadedEntryIdRef = useRef<number | null>(null);
  const playbackGenerationRef = useRef(0);

  const historyEnabled =
    scope === "interactive"
      ? Boolean(tts.interactive_history_enabled)
      : Boolean(tts.file_history_enabled);
  const historyMaxEntries = Number(
    scope === "interactive"
      ? (tts.interactive_history_max_entries ?? 100)
      : (tts.file_history_max_entries ?? 100),
  );
  const historyMaxStorageMb = Number(
    scope === "interactive"
      ? (tts.interactive_history_max_storage_mb ?? 1024)
      : (tts.file_history_max_storage_mb ?? 1024),
  );
  const enabledField =
    scope === "interactive"
      ? ("interactive_history_enabled" as const)
      : ("file_history_enabled" as const);
  const maxEntriesField =
    scope === "interactive"
      ? ("interactive_history_max_entries" as const)
      : ("file_history_max_entries" as const);
  const maxStorageField =
    scope === "interactive"
      ? ("interactive_history_max_storage_mb" as const)
      : ("file_history_max_storage_mb" as const);
  const groups = useMemo(() => groupHistoryEntries(entries), [entries]);
  const dateFormatter = useMemo(
    () =>
      new Intl.DateTimeFormat(i18n.language, {
        dateStyle: "medium",
        timeStyle: "short",
      }),
    [i18n.language],
  );

  const stopAudio = useCallback(() => {
    playbackGenerationRef.current += 1;
    playbackAbortRef.current?.abort();
    playbackAbortRef.current = null;
    const audio = audioRef.current;
    if (audio) {
      audio.pause();
      audio.removeAttribute("src");
      audio.load();
    }
    audioRef.current = null;
    if (playbackObjectUrlRef.current) {
      URL.revokeObjectURL(playbackObjectUrlRef.current);
      playbackObjectUrlRef.current = null;
    }
    playbackPitchCompensationRef.current = 1;
    loadedEntryIdRef.current = null;
    setActiveEntryId(null);
    setLoadedEntryId(null);
    setPlaybackTime(0);
    setPlaybackDuration(0);
  }, []);

  const refreshHistory = useCallback(async () => {
    setLoadError(null);
    try {
      const nextEntries = await invoke<TtsHistoryEntry[]>(
        "get_tts_history_entries",
        { scope },
      );
      setEntries(nextEntries);
      onAvailabilityChange?.(nextEntries.length > 0);
    } catch (error) {
      setLoadError(asErrorMessage(error));
    } finally {
      setLoading(false);
    }
  }, [onAvailabilityChange, scope]);

  useEffect(() => {
    void refreshHistory();
  }, [refreshHistory]);

  useEffect(() => {
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    void listen("tts-history-changed", () => {
      if (!disposed) void refreshHistory();
    }).then((nextUnlisten) => {
      if (disposed) nextUnlisten();
      else unlisteners.push(nextUnlisten);
    });
    void listen<string>("tts-history-error", (event) => {
      if (!disposed) {
        setActionMessage({
          kind: "error",
          text: t("textToSpeech.history.errors.capture", {
            error: event.payload,
          }),
        });
      }
    }).then((nextUnlisten) => {
      if (disposed) nextUnlisten();
      else unlisteners.push(nextUnlisten);
    });
    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [refreshHistory, t]);

  useEffect(() => stopAudio, [stopAudio]);

  const showActionError = useCallback(
    (translationKey: string, error: unknown) => {
      setActionMessage({
        kind: "error",
        text: t(translationKey, { error: asErrorMessage(error) }),
      });
    },
    [t],
  );

  const togglePlayback = async (entry: TtsHistoryEntry) => {
    setActionMessage(null);
    const currentAudio = audioRef.current;
    if (currentAudio && loadedEntryIdRef.current === entry.id) {
      if (currentAudio.paused) {
        const playbackGeneration = playbackGenerationRef.current;
        try {
          await currentAudio.play();
          if (
            playbackGenerationRef.current !== playbackGeneration ||
            audioRef.current !== currentAudio
          ) {
            currentAudio.pause();
            return;
          }
          setActiveEntryId(entry.id);
        } catch (error) {
          if (
            playbackGenerationRef.current === playbackGeneration &&
            audioRef.current === currentAudio
          ) {
            showActionError("textToSpeech.history.errors.playback", error);
          }
        }
      } else {
        playbackGenerationRef.current += 1;
        currentAudio.pause();
        setActiveEntryId(null);
      }
      return;
    }

    stopAudio();
    const playbackGeneration = playbackGenerationRef.current;
    setBusyEntryId(entry.id);
    try {
      const audioPath = await invoke<string | null>(
        "get_tts_history_audio_path",
        {
          id: entry.id,
        },
      );
      if (playbackGenerationRef.current !== playbackGeneration) {
        return;
      }
      if (!audioPath) {
        throw new Error(t("textToSpeech.history.errors.notFound"));
      }
      const sourceUrl = convertFileSrc(audioPath, "asset");
      const playbackAbort = new AbortController();
      playbackAbortRef.current = playbackAbort;
      let prepared = {
        url: sourceUrl,
        objectUrl: null as string | null,
        pitchCompensation: 1,
      };
      try {
        prepared = await prepareTtsPlaybackSource(
          sourceUrl,
          tts.playback_pitch ?? 1,
          tts.playback_effect ?? "none",
          playbackAbort.signal,
        );
      } catch (error) {
        if (playbackAbort.signal.aborted) return;
        console.warn(
          "Unable to apply optional TTS History playback processing; using the original audio:",
          error,
        );
      }
      if (playbackGenerationRef.current !== playbackGeneration) {
        if (prepared.objectUrl) URL.revokeObjectURL(prepared.objectUrl);
        return;
      }
      playbackAbortRef.current = null;
      playbackObjectUrlRef.current = prepared.objectUrl;
      playbackPitchCompensationRef.current = prepared.pitchCompensation;
      const audio = new Audio(prepared.url);
      applyPlaybackRate(audio, 1 / prepared.pitchCompensation);
      audioRef.current = audio;
      loadedEntryIdRef.current = entry.id;
      setLoadedEntryId(entry.id);
      setPlaybackTime(0);
      setPlaybackDuration(0);
      const updateTimeline = () => {
        if (audioRef.current !== audio) return;
        setPlaybackTime(
          Number.isFinite(audio.currentTime)
            ? audio.currentTime * playbackPitchCompensationRef.current
            : 0,
        );
        setPlaybackDuration(
          Number.isFinite(audio.duration)
            ? audio.duration * playbackPitchCompensationRef.current
            : 0,
        );
      };
      audio.onloadedmetadata = updateTimeline;
      audio.ondurationchange = updateTimeline;
      audio.ontimeupdate = updateTimeline;
      audio.onended = () => {
        if (audioRef.current === audio) {
          updateTimeline();
          setActiveEntryId(null);
        }
      };
      audio.onerror = () => {
        if (audioRef.current === audio) {
          setActiveEntryId(null);
          setActionMessage({
            kind: "error",
            text: t("textToSpeech.history.errors.playbackMedia"),
          });
        }
      };
      await audio.play();
      if (
        playbackGenerationRef.current !== playbackGeneration ||
        audioRef.current !== audio
      ) {
        audio.pause();
        audio.removeAttribute("src");
        audio.load();
        return;
      }
      setActiveEntryId(entry.id);
    } catch (error) {
      if (playbackGenerationRef.current === playbackGeneration) {
        stopAudio();
        showActionError("textToSpeech.history.errors.playback", error);
      }
    } finally {
      if (playbackGenerationRef.current === playbackGeneration) {
        setBusyEntryId(null);
      }
    }
  };

  const seekPlayback = (entryId: number, requestedTime: number) => {
    const audio = audioRef.current;
    if (
      !audio ||
      loadedEntryIdRef.current !== entryId ||
      !Number.isFinite(requestedTime) ||
      playbackDuration <= 0
    ) {
      return;
    }
    const boundedTime = Math.min(playbackDuration, Math.max(0, requestedTime));
    audio.currentTime = boundedTime / playbackPitchCompensationRef.current;
    setPlaybackTime(boundedTime);
  };

  const exportEntry = async (entry: TtsHistoryEntry) => {
    setActionMessage(null);
    const date = new Date(normalizedTimestamp(entry.timestamp));
    const datePart = Number.isNaN(date.getTime())
      ? String(entry.id)
      : date.toISOString().slice(0, 19).replace(/[T:]/g, "-");
    const destination = await save({
      defaultPath: `${safeFilePart(`${datePart}-${entry.voice}`)}.${entry.output_format}`,
      filters: [
        {
          name:
            entry.output_format === "mp3"
              ? t("textToSpeech.history.audio.mp3")
              : t("textToSpeech.history.audio.wav"),
          extensions: [entry.output_format],
        },
      ],
    });
    if (!destination) return;

    setBusyEntryId(entry.id);
    try {
      await invoke<string>("export_tts_history_audio", {
        id: entry.id,
        destination,
      });
      setActionMessage({
        kind: "success",
        text: t("textToSpeech.history.exportSuccess"),
      });
      await refreshHistory();
    } catch (error) {
      showActionError("textToSpeech.history.errors.export", error);
    } finally {
      setBusyEntryId(null);
    }
  };

  const deleteEntry = async (entry: TtsHistoryEntry) => {
    setActionMessage(null);
    setBusyEntryId(entry.id);
    try {
      const outcome = await invoke<TtsHistoryDeleteOutcome | null>(
        "delete_tts_history_entry_detailed",
        {
          id: entry.id,
        },
      );
      if (!outcome) {
        throw new Error(t("textToSpeech.history.errors.notFound"));
      }
      if (!outcome.record_deleted) {
        throw new Error(
          t("textToSpeech.history.deletePartial", {
            error:
              outcome.managed_audio_error ??
              t("textToSpeech.history.errors.notFound"),
          }),
        );
      }
      if (loadedEntryIdRef.current === entry.id) stopAudio();
      setActionMessage(
        outcome.managed_audio_status === "deleted"
          ? {
              kind: "success",
              text: t("textToSpeech.history.deleteSuccess"),
            }
          : {
              kind: "error",
              text: t("textToSpeech.history.deletePartial", {
                error:
                  outcome.managed_audio_error ??
                  t("textToSpeech.history.errors.notFound"),
              }),
            },
      );
      await refreshHistory();
    } catch (error) {
      showActionError("textToSpeech.history.errors.delete", error);
    } finally {
      setBusyEntryId(null);
    }
  };

  const regenerateEntry = async (entry: TtsHistoryEntry) => {
    setActionMessage(null);
    setBusyEntryId(entry.id);
    try {
      await flushPendingSettingsWrites();
      const provider = tts.provider ?? entry.provider;
      const model = modelForProvider(tts, provider);
      const voice = voiceForProvider(tts, provider);
      const normalizedModel = model?.trim() ?? "";
      const promptSupported =
        provider === "openai" &&
        (!normalizedModel || normalizedModel.startsWith("gpt-4o-mini-tts"));
      const response = await invoke<RegenerateTtsHistoryResponse>(
        "regenerate_tts_history_entry",
        {
          request: {
            id: entry.id,
            outputPath: null,
            provider,
            model: model || null,
            voice: voice || null,
            promptPresetId:
              promptSupported && tts.selected_prompt_id
                ? tts.selected_prompt_id
                : null,
            promptPresetName: null,
            instructions:
              promptSupported && !tts.selected_prompt_id
                ? (tts.openai_instructions ?? "")
                : null,
            outputFormat: tts.output_format ?? entry.output_format,
            mp3BitrateKbps:
              (tts.output_format ?? entry.output_format) === "mp3"
                ? (tts.mp3_bitrate_kbps ?? null)
                : null,
            confirmedApiCharge:
              providerUsesPaidApi(provider) ||
              (scope === "interactive"
                ? Boolean(tts.llm_preprocessing?.interactive_enabled)
                : Boolean(tts.llm_preprocessing?.file_enabled)),
          },
        },
      );
      setActionMessage({
        kind: "success",
        text:
          response.resumedChunks > 0
            ? t("textToSpeech.history.regenerateSuccessResumed", {
                count: response.resumedChunks,
              })
            : t("textToSpeech.history.regenerateSuccess"),
      });
      await refreshHistory();
    } catch (error) {
      showActionError("textToSpeech.history.errors.regenerate", error);
    } finally {
      setBusyEntryId(null);
    }
  };

  const confirmAction = () => {
    const pending = confirmation;
    if (!pending) return;
    if (pending.kind === "delete") void deleteEntry(pending.entry);
    else void regenerateEntry(pending.entry);
  };

  const currentModel = tts.provider
    ? modelForProvider(tts, tts.provider)
    : undefined;
  const normalizedCurrentModel = currentModel?.trim() ?? "";
  const currentPromptSupported =
    tts.provider === "openai" &&
    (!normalizedCurrentModel ||
      normalizedCurrentModel.startsWith("gpt-4o-mini-tts"));
  const selectedPrompt = currentPromptSupported
    ? (tts.prompt_presets?.find(
        (preset) => preset.id === tts.selected_prompt_id,
      )?.name ??
      (tts.openai_instructions?.trim()
        ? t("textToSpeech.history.customInstructions")
        : t("textToSpeech.history.none")))
    : t("textToSpeech.history.none");
  const currentVoice = tts.provider
    ? voiceForProvider(tts, tts.provider)
    : undefined;
  const currentLlmCleanupEnabled =
    scope === "interactive"
      ? Boolean(tts.llm_preprocessing?.interactive_enabled)
      : Boolean(tts.llm_preprocessing?.file_enabled);
  const regenerationUsesPaidApi =
    providerUsesPaidApi(tts.provider) || currentLlmCleanupEnabled;
  const regenerationUsesNoKeyNetwork =
    tts.provider === "edge" && !currentLlmCleanupEnabled;

  return (
    <>
      <SettingsGroup
        title={t(`textToSpeech.history.scopes.${scope}.title`)}
        description={t(`textToSpeech.history.scopes.${scope}.description`)}
        help={
          <TtsHelpDisclosure
            summary={t("textToSpeech.help.historySummary")}
            items={[
              {
                term: t("textToSpeech.help.retention"),
                description: t("textToSpeech.help.retentionDescription"),
              },
              {
                term: t("textToSpeech.help.regeneration"),
                description: t("textToSpeech.help.regenerationDescription"),
              },
              {
                term: t("textToSpeech.help.export"),
                description: t("textToSpeech.help.exportDescription"),
              },
            ]}
            links={[
              {
                label: t("textToSpeech.help.aivoRelayGuide"),
                href: AIVORELAY_TTS_GUIDE_URL,
              },
            ]}
          />
        }
      >
        <ToggleSwitch
          grouped
          checked={historyEnabled}
          onChange={(enabled) => {
            void updateTts({ [enabledField]: enabled }, enabledField).then(
              refreshHistory,
            );
          }}
          isUpdating={savingField === enabledField}
          label={t("textToSpeech.history.enable")}
          description={t(
            `textToSpeech.history.scopes.${scope}.enableDescription`,
          )}
          descriptionMode="inline"
        />

        <SettingContainer
          grouped
          title={t("textToSpeech.history.maxEntries")}
          description={t("textToSpeech.history.maxEntriesDescription")}
          descriptionMode="inline"
        >
          <Input
            className="w-28"
            type="number"
            min={1}
            max={100000}
            step={1}
            value={historyMaxEntries}
            onChange={(event) =>
              void updateTts(
                {
                  [maxEntriesField]: asBoundedInteger(
                    event.target.value,
                    1,
                    100000,
                    100,
                  ),
                },
                maxEntriesField,
              )
            }
          />
        </SettingContainer>

        <SettingContainer
          grouped
          title={t("textToSpeech.history.maxStorage")}
          description={t("textToSpeech.history.maxStorageDescription")}
          descriptionMode="inline"
        >
          <div className="flex items-center gap-2">
            <Input
              className="w-28"
              type="number"
              min={1}
              max={1048576}
              step={128}
              value={historyMaxStorageMb}
              onChange={(event) =>
                void updateTts(
                  {
                    [maxStorageField]: asBoundedInteger(
                      event.target.value,
                      1,
                      1048576,
                      1024,
                    ),
                  },
                  maxStorageField,
                )
              }
            />
            <span className="text-xs text-[#808080]">
              {t("textToSpeech.units.megabytes")}
            </span>
          </div>
        </SettingContainer>

        <div className="px-6 py-4">
          <div
            role="alert"
            className="flex items-start gap-3 rounded-lg border border-amber-400/50 bg-amber-400/10 px-4 py-3"
          >
            <AlertTriangle
              className="mt-0.5 h-5 w-5 shrink-0 text-amber-300"
              aria-hidden="true"
            />
            <div>
              <p className="text-sm font-semibold text-amber-100">
                {t("textToSpeech.history.storageWarningTitle")}
              </p>
              <p className="mt-1 text-xs leading-relaxed text-amber-100/75">
                {t("textToSpeech.history.storageWarning")}
              </p>
            </div>
          </div>
          {!historyEnabled && entries.length > 0 && (
            <p className="mt-3 text-xs leading-relaxed text-[#a0a0a0]">
              {t("textToSpeech.history.disabledExisting")}
            </p>
          )}
        </div>

        <div className="px-6 py-4">
          <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
            <div className="flex items-center gap-2">
              <History className="h-4 w-4 text-[#ff8ebb]" aria-hidden="true" />
              <h3 className="text-sm font-semibold text-[#f5f5f5]">
                {t("textToSpeech.history.results")}
              </h3>
              {!loading && (
                <span className="rounded-full bg-white/[0.06] px-2 py-0.5 text-[11px] text-[#a0a0a0]">
                  {t("textToSpeech.history.resultCount", {
                    count: entries.length,
                  })}
                </span>
              )}
            </div>
            <Button
              variant="ghost"
              size="sm"
              disabled={loading}
              onClick={() => void refreshHistory()}
              title={t("textToSpeech.history.refresh")}
            >
              <RefreshCw
                className={`mr-1.5 inline h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`}
                aria-hidden="true"
              />
              {t("textToSpeech.history.refresh")}
            </Button>
          </div>

          {actionMessage && (
            <div
              role={actionMessage.kind === "error" ? "alert" : "status"}
              className={`mb-4 flex items-start gap-2 rounded-lg border px-3 py-2 text-xs ${
                actionMessage.kind === "error"
                  ? "border-red-400/40 bg-red-400/10 text-red-100"
                  : "border-emerald-400/40 bg-emerald-400/10 text-emerald-100"
              }`}
            >
              {actionMessage.kind === "error" && (
                <AlertCircle
                  className="mt-0.5 h-4 w-4 shrink-0"
                  aria-hidden="true"
                />
              )}
              <span className="break-words">{actionMessage.text}</span>
            </div>
          )}

          {loadError && (
            <div
              role="alert"
              className="flex items-start justify-between gap-3 rounded-lg border border-red-400/40 bg-red-400/10 px-4 py-3"
            >
              <div className="flex min-w-0 items-start gap-2 text-sm text-red-100">
                <AlertCircle
                  className="mt-0.5 h-4 w-4 shrink-0"
                  aria-hidden="true"
                />
                <span className="break-words">
                  {t("textToSpeech.history.errors.load", { error: loadError })}
                </span>
              </div>
              <Button
                variant="secondary"
                size="sm"
                onClick={() => void refreshHistory()}
              >
                {t("textToSpeech.history.tryAgain")}
              </Button>
            </div>
          )}

          {loading && entries.length === 0 && (
            <div
              role="status"
              className="flex items-center justify-center gap-2 rounded-lg border border-white/[0.05] bg-black/10 px-4 py-10 text-sm text-[#a0a0a0]"
            >
              <Loader2 className="h-4 w-4 animate-spin" aria-hidden="true" />
              {t("textToSpeech.history.loading")}
            </div>
          )}

          {!loading && !loadError && entries.length === 0 && (
            <div className="rounded-lg border border-dashed border-white/10 bg-black/10 px-5 py-10 text-center">
              <Volume2
                className="mx-auto h-7 w-7 text-[#707070]"
                aria-hidden="true"
              />
              <p className="mt-3 text-sm font-medium text-[#d0d0d0]">
                {t("textToSpeech.history.empty")}
              </p>
              <p className="mx-auto mt-1 max-w-lg text-xs leading-relaxed text-[#808080]">
                {historyEnabled
                  ? t(`textToSpeech.history.scopes.${scope}.emptyEnabled`)
                  : t(`textToSpeech.history.scopes.${scope}.emptyDisabled`)}
              </p>
            </div>
          )}

          {groups.length > 0 && (
            <div className="space-y-4">
              {groups.map((group) => (
                <section
                  key={group.id}
                  className="rounded-xl border border-white/[0.07] bg-black/15 p-4"
                >
                  <div className="mb-4">
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-[#ff8ebb]">
                        {t("textToSpeech.history.source")}
                      </p>
                      <span className="text-[11px] text-[#808080]">
                        {t("textToSpeech.history.variantCount", {
                          count: group.entries.length,
                        })}
                      </span>
                    </div>
                    <p className="mt-2 line-clamp-4 whitespace-pre-wrap break-words text-sm leading-relaxed text-[#d8d8d8]">
                      {group.sourceText}
                    </p>
                  </div>

                  <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
                    {group.entries.map((entry) => {
                      const isPlaying = activeEntryId === entry.id;
                      const isLoaded = loadedEntryId === entry.id;
                      const isBusy = busyEntryId === entry.id;
                      const timestamp = new Date(
                        normalizedTimestamp(entry.timestamp),
                      );
                      const formattedTimestamp = Number.isNaN(
                        timestamp.getTime(),
                      )
                        ? t("textToSpeech.history.unknownDate")
                        : dateFormatter.format(timestamp);
                      return (
                        <article
                          key={entry.id}
                          className={`rounded-lg border p-3 transition-colors ${
                            isPlaying
                              ? "border-[#ff4d8d]/70 bg-[#ff4d8d]/10 shadow-[0_0_0_1px_rgba(255,77,141,0.12)]"
                              : "border-white/[0.07] bg-white/[0.025]"
                          }`}
                        >
                          <div className="flex items-start justify-between gap-3">
                            <div className="min-w-0">
                              <p className="truncate text-sm font-semibold text-[#f5f5f5]">
                                {providerLabel(entry.provider)}
                              </p>
                              <time
                                className="mt-0.5 block text-[11px] text-[#808080]"
                                dateTime={
                                  Number.isNaN(timestamp.getTime())
                                    ? undefined
                                    : timestamp.toISOString()
                                }
                              >
                                {formattedTimestamp}
                              </time>
                            </div>
                            {isPlaying && (
                              <span className="flex shrink-0 items-center gap-1 rounded-full bg-[#ff4d8d]/20 px-2 py-1 text-[10px] font-semibold text-[#ff9bc0]">
                                <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-[#ff4d8d]" />
                                {t("textToSpeech.history.playing")}
                              </span>
                            )}
                          </div>

                          <dl className="mt-3 grid grid-cols-[auto,minmax(0,1fr)] gap-x-3 gap-y-1 text-xs">
                            <dt className="text-[#707070]">
                              {t("textToSpeech.history.metadata.model")}
                            </dt>
                            <dd className="truncate text-right text-[#c8c8c8]">
                              {entry.model || t("textToSpeech.history.none")}
                            </dd>
                            <dt className="text-[#707070]">
                              {t("textToSpeech.history.metadata.voice")}
                            </dt>
                            <dd className="truncate text-right text-[#c8c8c8]">
                              {entry.voice ||
                                (entry.provider === "windows"
                                  ? t("textToSpeech.windows.defaultVoice")
                                  : t("textToSpeech.history.none"))}
                            </dd>
                            <dt className="text-[#707070]">
                              {t("textToSpeech.history.metadata.language")}
                            </dt>
                            <dd className="truncate text-right text-[#c8c8c8]">
                              {entry.language || t("textToSpeech.history.none")}
                            </dd>
                            <dt className="text-[#707070]">
                              {t("textToSpeech.history.metadata.prompt")}
                            </dt>
                            <dd className="truncate text-right text-[#c8c8c8]">
                              {entry.prompt_preset_name ||
                                (entry.resolved_instructions?.trim()
                                  ? t("textToSpeech.history.customInstructions")
                                  : t("textToSpeech.history.none"))}
                            </dd>
                            <dt className="text-[#707070]">
                              {t(
                                "textToSpeech.history.metadata.aiCleanup",
                                "AI cleanup",
                              )}
                            </dt>
                            <dd
                              className="truncate text-right text-[#c8c8c8]"
                              title={llmCleanupSummary(entry) ?? undefined}
                            >
                              {llmCleanupSummary(entry) ||
                                t("textToSpeech.history.none")}
                            </dd>
                            <dt className="text-[#707070]">
                              {t("textToSpeech.history.metadata.format")}
                            </dt>
                            <dd className="text-right font-mono uppercase text-[#c8c8c8]">
                              {entry.output_format}
                            </dd>
                          </dl>

                          <div className="mt-4 flex items-center gap-2">
                            <span className="min-w-9 text-right font-mono text-[11px] tabular-nums text-[#808080]">
                              {formatPlaybackTime(isLoaded ? playbackTime : 0)}
                            </span>
                            <input
                              type="range"
                              min={0}
                              max={isLoaded ? playbackDuration : 0}
                              step={0.01}
                              value={isLoaded ? playbackTime : 0}
                              disabled={!isLoaded || playbackDuration <= 0}
                              onChange={(event) =>
                                seekPlayback(
                                  entry.id,
                                  Number(event.target.value),
                                )
                              }
                              aria-label={t("textToSpeech.history.seek")}
                              className="h-1 min-w-0 flex-1 cursor-pointer appearance-none rounded-lg bg-white/10 accent-[#ff4d8d] disabled:cursor-not-allowed disabled:opacity-40"
                            />
                            <span className="min-w-9 font-mono text-[11px] tabular-nums text-[#808080]">
                              {formatPlaybackTime(
                                isLoaded ? playbackDuration : 0,
                              )}
                            </span>
                          </div>

                          <div className="mt-4 flex flex-wrap gap-2">
                            <Button
                              variant={isPlaying ? "primary" : "secondary"}
                              size="sm"
                              disabled={isBusy}
                              onClick={() => void togglePlayback(entry)}
                              title={
                                isPlaying
                                  ? t("textToSpeech.history.pause")
                                  : t("textToSpeech.history.play")
                              }
                            >
                              {isBusy ? (
                                <Loader2
                                  className="mr-1.5 inline h-3.5 w-3.5 animate-spin"
                                  aria-hidden="true"
                                />
                              ) : isPlaying ? (
                                <Pause
                                  className="mr-1.5 inline h-3.5 w-3.5"
                                  aria-hidden="true"
                                />
                              ) : (
                                <Play
                                  className="mr-1.5 inline h-3.5 w-3.5"
                                  aria-hidden="true"
                                />
                              )}
                              {isPlaying
                                ? t("textToSpeech.history.pause")
                                : t("textToSpeech.history.play")}
                            </Button>
                            <Button
                              variant="secondary"
                              size="sm"
                              disabled={isBusy}
                              onClick={() => void exportEntry(entry)}
                              title={t("textToSpeech.history.export")}
                            >
                              <Download
                                className="mr-1.5 inline h-3.5 w-3.5"
                                aria-hidden="true"
                              />
                              {t("textToSpeech.history.export")}
                            </Button>
                            <Button
                              variant="secondary"
                              size="sm"
                              disabled={isBusy}
                              onClick={() =>
                                setConfirmation({
                                  kind: "regenerate",
                                  entry,
                                })
                              }
                              title={t("textToSpeech.history.regenerate")}
                            >
                              <RotateCcw
                                className="mr-1.5 inline h-3.5 w-3.5"
                                aria-hidden="true"
                              />
                              {t("textToSpeech.history.regenerate")}
                            </Button>
                            <Button
                              variant="ghost"
                              size="sm"
                              disabled={isBusy}
                              onClick={() =>
                                setConfirmation({ kind: "delete", entry })
                              }
                              className="ml-auto text-red-300 hover:text-red-200"
                              title={t("textToSpeech.history.delete")}
                            >
                              <Trash2
                                className="h-3.5 w-3.5"
                                aria-hidden="true"
                              />
                              <span className="sr-only">
                                {t("textToSpeech.history.delete")}
                              </span>
                            </Button>
                          </div>
                        </article>
                      );
                    })}
                  </div>
                </section>
              ))}
            </div>
          )}
        </div>
      </SettingsGroup>

      <ConfirmationModal
        isOpen={confirmation?.kind === "delete"}
        onClose={() => setConfirmation(null)}
        onConfirm={confirmAction}
        title={t("textToSpeech.history.confirmDeleteTitle")}
        message={t("textToSpeech.history.confirmDelete")}
        confirmText={t("textToSpeech.history.delete")}
        cancelText={t("common.cancel")}
        variant="danger"
      />

      <ConfirmationModal
        isOpen={confirmation?.kind === "regenerate"}
        onClose={() => setConfirmation(null)}
        onConfirm={confirmAction}
        title={t("textToSpeech.history.confirmRegenerateTitle")}
        message={t(
          regenerationUsesPaidApi
            ? "textToSpeech.history.confirmRegenerate"
            : regenerationUsesNoKeyNetwork
              ? "textToSpeech.history.confirmRegenerateNoCredits"
              : "textToSpeech.history.confirmRegenerateOffline",
          {
            provider: tts.provider
              ? providerLabel(tts.provider)
              : t("textToSpeech.history.none"),
            voice:
              currentVoice ||
              (tts.provider === "windows"
                ? t("textToSpeech.windows.defaultVoice")
                : t("textToSpeech.history.none")),
            prompt: selectedPrompt,
          },
        )}
        confirmText={t(
          regenerationUsesPaidApi
            ? "textToSpeech.history.regenerateAndUseCredits"
            : regenerationUsesNoKeyNetwork
              ? "textToSpeech.history.regenerateNoCredits"
              : "textToSpeech.history.regenerateOffline",
        )}
        cancelText={t("common.cancel")}
        variant="warning"
      />
    </>
  );
};

export default TtsHistory;
