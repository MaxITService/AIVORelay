import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { type as getOsType } from "@tauri-apps/plugin-os";
import { useTranslation } from "react-i18next";
import {
  AlertCircle,
  CheckCircle2,
  ExternalLink,
  FileAudio,
  FileText,
  Loader2,
  Plus,
  Save as SaveIcon,
  Trash2,
} from "lucide-react";

import { useSettings } from "@/hooks/useSettings";
import { ApiKeyEditor, StoredApiKeyDisplay } from "../ApiKeyControls";
import { HandyShortcut } from "../HandyShortcut";
import { TtsFolderAutomation } from "./TtsFolderAutomation";
import { TtsHistory } from "./TtsHistory";
import { Button } from "@/components/ui/Button";
import { Dropdown, type DropdownOption } from "@/components/ui/Dropdown";
import { HotkeyCapture } from "@/components/ui/HotkeyCapture";
import { Input } from "@/components/ui/Input";
import { SettingContainer } from "@/components/ui/SettingContainer";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { Textarea } from "@/components/ui/Textarea";
import { ToggleSwitch } from "@/components/ui/ToggleSwitch";
import type { OSType } from "@/lib/utils/keyboard";

type TtsProvider = "soniox" | "deepgram" | "openai";
type TtsKeySource = "shared" | "separate";
type TtsOutputFormat = "mp3" | "wav";

type TtsReplacementRule = {
  id: string;
  from: string;
  to: string;
  enabled: boolean;
  case_sensitive: boolean;
  is_regex: boolean;
};

type TtsPromptPreset = {
  id: string;
  name: string;
  instructions: string;
};

type TtsSettings = {
  enabled: boolean;
  provider: TtsProvider;
  soniox_key_source: TtsKeySource;
  deepgram_key_source: TtsKeySource;
  openai_key_source: TtsKeySource;
  soniox_model: string;
  soniox_language: string;
  soniox_voice: string;
  deepgram_model: string;
  openai_model: string;
  openai_voice: string;
  speed: number;
  openai_instructions: string;
  prompt_presets: TtsPromptPreset[];
  selected_prompt_id: string;
  play_pause_hotkey: string;
  stop_hotkey: string;
  preprocessing_enabled: boolean;
  preprocessing_rules: TtsReplacementRule[];
  retry_count: number;
  retry_base_delay_ms: number;
  interactive_target_chars: number;
  file_target_chars: number;
  inter_chunk_pause_ms: number;
  paragraph_pause_ms: number;
  autoplay: boolean;
  output_format: TtsOutputFormat;
  mp3_bitrate_kbps: number;
  watch_folder_enabled: boolean;
  watch_recursive: boolean;
  watch_input_directory: string;
  watch_output_directory: string;
  watch_settle_delay_ms: number;
  disk_reserve_mb: number;
  history_enabled: boolean;
  history_max_entries: number;
  history_max_storage_mb: number;
};

type FileInspection = {
  source_characters?: number;
  sourceCharacters?: number;
  source_character_count?: number;
  sourceCharacterCount?: number;
  processed_characters?: number;
  processedCharacters?: number;
  processed_character_count?: number;
  processedCharacterCount?: number;
  chunk_count?: number;
  chunkCount?: number;
};

type ConversionProgress = {
  operation_id?: string;
  operationId?: string;
  kind?: string | null;
  phase?: string;
  completed_chunks?: number;
  completedChunks?: number;
  total_chunks?: number;
  totalChunks?: number;
  current_chunk?: number;
  currentChunk?: number;
  current_attempt?: number;
  currentAttempt?: number;
  attempt?: number;
  status?: string;
  message?: string | null;
  output_path?: string | null;
  outputPath?: string | null;
};

const DEFAULT_TTS_SETTINGS: TtsSettings = {
  enabled: true,
  provider: "soniox",
  soniox_key_source: "shared",
  deepgram_key_source: "shared",
  openai_key_source: "shared",
  soniox_model: "tts-rt-v1",
  soniox_language: "en",
  soniox_voice: "Maya",
  deepgram_model: "aura-2-thalia-en",
  openai_model: "gpt-4o-mini-tts",
  openai_voice: "marin",
  speed: 1,
  openai_instructions: "",
  prompt_presets: [],
  selected_prompt_id: "",
  play_pause_hotkey: "space",
  stop_hotkey: "escape",
  preprocessing_enabled: true,
  preprocessing_rules: [],
  retry_count: 3,
  retry_base_delay_ms: 750,
  interactive_target_chars: 350,
  file_target_chars: 1800,
  inter_chunk_pause_ms: 120,
  paragraph_pause_ms: 350,
  autoplay: true,
  output_format: "mp3",
  mp3_bitrate_kbps: 256,
  watch_folder_enabled: false,
  watch_recursive: false,
  watch_input_directory: "",
  watch_output_directory: "",
  watch_settle_delay_ms: 1500,
  disk_reserve_mb: 512,
  history_enabled: false,
  history_max_entries: 100,
  history_max_storage_mb: 1024,
};

const PROVIDERS: DropdownOption[] = [
  { value: "soniox", label: "Soniox" },
  { value: "deepgram", label: "Deepgram" },
  { value: "openai", label: "OpenAI" },
];

const PROVIDER_INPUT_LIMITS: Record<TtsProvider, number> = {
  soniox: 5000,
  deepgram: 2000,
  openai: 4096,
};

const PROVIDER_VOICE_RESOURCES: Record<
  TtsProvider,
  { voices: string; playground?: string }
> = {
  soniox: {
    voices: "https://soniox.com/docs/tts/concepts/voices",
  },
  deepgram: {
    voices: "https://developers.deepgram.com/docs/tts-models",
    playground: "https://playground.deepgram.com/",
  },
  openai: {
    voices: "https://developers.openai.com/api/docs/guides/text-to-speech",
    playground: "https://www.openai.fm/",
  },
};

const BITRATES = [64, 96, 128, 192, 256, 320];
const SETTINGS_EDIT_DEBOUNCE_MS = 250;
const COALESCED_TTS_FIELDS = new Set([
  "soniox_model",
  "soniox_language",
  "soniox_voice",
  "deepgram_model",
  "openai_model",
  "openai_voice",
  "speed",
  "openai_instructions",
  "retry_count",
  "retry_base_delay_ms",
  "interactive_target_chars",
  "file_target_chars",
  "inter_chunk_pause_ms",
  "paragraph_pause_ms",
  "watch_settle_delay_ms",
  "disk_reserve_mb",
  "history_max_entries",
  "history_max_storage_mb",
]);

const asErrorMessage = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

const clampNumber = (
  value: string,
  min: number,
  max: number,
  fallback: number,
) => {
  const parsed = Number(value);
  return Number.isFinite(parsed)
    ? Math.min(max, Math.max(min, parsed))
    : fallback;
};

const characterCount = (
  inspection: FileInspection | null,
  kind: "source" | "processed",
) =>
  kind === "source"
    ? (inspection?.source_characters ??
      inspection?.sourceCharacters ??
      inspection?.source_character_count ??
      inspection?.sourceCharacterCount ??
      null)
    : (inspection?.processed_characters ??
      inspection?.processedCharacters ??
      inspection?.processed_character_count ??
      inspection?.processedCharacterCount ??
      null);

const chunkCount = (inspection: FileInspection | null) =>
  inspection?.chunk_count ?? inspection?.chunkCount ?? null;

export const TextToSpeechSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();
  const storedTts = (settings as any)?.tts as Partial<TtsSettings> | undefined;
  const storedTtsSnapshot = useMemo<TtsSettings>(
    () => ({
      ...DEFAULT_TTS_SETTINGS,
      ...storedTts,
      preprocessing_rules: storedTts?.preprocessing_rules ?? [],
      prompt_presets: storedTts?.prompt_presets ?? [],
    }),
    [storedTts],
  );
  const [tts, setTts] = useState(storedTtsSnapshot);
  const ttsRef = useRef(storedTtsSnapshot);
  const settingsWriteQueueRef = useRef<Promise<void>>(Promise.resolve());
  const pendingSettingsWritesRef = useRef(0);
  const settingsWriteGenerationRef = useRef(0);

  useEffect(() => {
    if (pendingSettingsWritesRef.current === 0) {
      ttsRef.current = storedTtsSnapshot;
      setTts(storedTtsSnapshot);
    }
  }, [storedTtsSnapshot]);

  const osKind = getOsType();
  const hotkeyOsType: OSType =
    osKind === "windows" || osKind === "macos" || osKind === "linux"
      ? osKind
      : "unknown";

  const [savingField, setSavingField] = useState<string | null>(null);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [hasSeparateKey, setHasSeparateKey] = useState(false);
  const [keyDraft, setKeyDraft] = useState("");
  const [editingKey, setEditingKey] = useState(false);
  const [keyBusy, setKeyBusy] = useState(false);
  const [capturingHotkey, setCapturingHotkey] = useState<
    "play_pause" | "stop" | null
  >(null);
  const [ruleFrom, setRuleFrom] = useState("");
  const [ruleTo, setRuleTo] = useState("");
  const [ruleCaseSensitive, setRuleCaseSensitive] = useState(true);
  const [ruleIsRegex, setRuleIsRegex] = useState(false);
  const [promptPresetName, setPromptPresetName] = useState("");

  const [inputPath, setInputPath] = useState("");
  const [outputPath, setOutputPath] = useState("");
  const [inspection, setInspection] = useState<FileInspection | null>(null);
  const [inspecting, setInspecting] = useState(false);
  const [outputFormat, setOutputFormat] = useState<TtsOutputFormat>("mp3");
  const [mp3Bitrate, setMp3Bitrate] = useState(256);
  const [conversionBusy, setConversionBusy] = useState(false);
  const [conversionProgress, setConversionProgress] =
    useState<ConversionProgress | null>(null);
  const [conversionError, setConversionError] = useState<string | null>(null);
  const [completedPath, setCompletedPath] = useState("");
  const [operationId, setOperationId] = useState<string | null>(null);
  const conversionBusyRef = useRef(false);
  const conversionOperationIdRef = useRef<string | null>(null);
  const keySourceField = `${tts.provider}_key_source` as
    | "soniox_key_source"
    | "deepgram_key_source"
    | "openai_key_source";
  const keySource = tts[keySourceField];
  const voiceValue =
    tts.provider === "soniox"
      ? tts.soniox_voice
      : tts.provider === "deepgram"
        ? tts.deepgram_model
        : tts.openai_voice;
  const modelValue =
    tts.provider === "soniox"
      ? tts.soniox_model
      : tts.provider === "deepgram"
        ? tts.deepgram_model
        : tts.openai_model;
  const speedMinimum = tts.provider === "openai" ? 0.25 : 0.7;
  const speedMaximum =
    tts.provider === "soniox" ? 1.3 : tts.provider === "deepgram" ? 1.5 : 4;
  const openAiInstructionsSupported =
    !tts.openai_model.trim() ||
    tts.openai_model.trim().startsWith("gpt-4o-mini-tts");
  const voiceResources = PROVIDER_VOICE_RESOURCES[tts.provider];

  const updateTts = useCallback(
    async (patch: Partial<TtsSettings>, field: string) => {
      const writeGeneration = ++settingsWriteGenerationRef.current;
      const nextSettings = { ...ttsRef.current, ...patch };
      ttsRef.current = nextSettings;
      setTts(nextSettings);
      if (
        Object.keys(patch).some((key) =>
          [
            "provider",
            "preprocessing_enabled",
            "preprocessing_rules",
            "file_target_chars",
          ].includes(key),
        )
      ) {
        setInspection(null);
      }
      pendingSettingsWritesRef.current += 1;
      setSavingField(field);
      setSettingsError(null);
      if (COALESCED_TTS_FIELDS.has(field)) {
        await new Promise((resolve) =>
          window.setTimeout(resolve, SETTINGS_EDIT_DEBOUNCE_MS),
        );
        if (settingsWriteGenerationRef.current !== writeGeneration) {
          pendingSettingsWritesRef.current = Math.max(
            0,
            pendingSettingsWritesRef.current - 1,
          );
          if (pendingSettingsWritesRef.current === 0) {
            setSavingField(null);
            await refreshSettings();
          }
          return;
        }
      }
      const write = settingsWriteQueueRef.current
        .catch(() => undefined)
        .then(async () => {
          try {
            const normalized = await invoke<TtsSettings>(
              "update_tts_settings",
              {
                settings: nextSettings,
              },
            );
            if (pendingSettingsWritesRef.current === 1) {
              ttsRef.current = normalized;
              setTts(normalized);
            }
          } finally {
            pendingSettingsWritesRef.current = Math.max(
              0,
              pendingSettingsWritesRef.current - 1,
            );
            if (pendingSettingsWritesRef.current === 0) {
              setSavingField(null);
              await refreshSettings();
            }
          }
        });
      settingsWriteQueueRef.current = write.catch(() => undefined);
      try {
        await write;
      } catch (error) {
        if (settingsWriteGenerationRef.current === writeGeneration) {
          setSettingsError(asErrorMessage(error));
        }
      }
    },
    [refreshSettings],
  );

  const refreshKeyStatus = useCallback(async () => {
    setKeyBusy(true);
    try {
      const exists = await invoke<boolean>("tts_has_api_key", {
        provider: tts.provider,
      });
      setHasSeparateKey(exists);
    } catch (error) {
      setSettingsError(asErrorMessage(error));
    } finally {
      setKeyBusy(false);
    }
  }, [tts.provider]);

  useEffect(() => {
    void refreshKeyStatus();
    setEditingKey(false);
    setKeyDraft("");
  }, [refreshKeyStatus]);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    let disposed = false;

    const subscribe = async () => {
      for (const eventName of [
        "tts-conversion-progress",
        "tts_conversion_progress",
        "tts://progress",
        "tts://state",
      ]) {
        const unlisten = await listen<ConversionProgress>(
          eventName,
          (event) => {
            const progress = event.payload;
            if (!conversionBusyRef.current) return;
            if (
              eventName === "tts://state" &&
              progress.kind !== "file_conversion"
            ) {
              return;
            }
            const rawEventOperationId =
              progress.operation_id ?? progress.operationId;
            const eventOperationId =
              rawEventOperationId === undefined
                ? null
                : String(rawEventOperationId);
            if (
              conversionOperationIdRef.current &&
              eventOperationId &&
              eventOperationId !== conversionOperationIdRef.current
            )
              return;
            if (!conversionOperationIdRef.current && eventOperationId) {
              conversionOperationIdRef.current = eventOperationId;
              setOperationId(eventOperationId);
            }
            const normalizedProgress: ConversionProgress = {
              ...progress,
              status: progress.status ?? progress.phase,
              attempt:
                progress.attempt ??
                progress.current_attempt ??
                progress.currentAttempt,
            };
            setConversionProgress((previous) => ({
              ...previous,
              ...normalizedProgress,
            }));
            const finalPath = progress.output_path ?? progress.outputPath;
            if (finalPath) setCompletedPath(finalPath);
          },
        );
        if (disposed) unlisten();
        else unlisteners.push(unlisten);
      }
    };

    void subscribe();
    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<ConversionProgress>("tts://state", (event) => {
      const state = event.payload;
      const rawOperationId = state.operation_id ?? state.operationId;
      if (
        disposed ||
        state.kind !== "file_conversion" ||
        state.phase !== "error" ||
        Number(rawOperationId) !== 0
      ) {
        return;
      }
      const message = state.message || t("textToSpeech.folder.backgroundError");
      setSettingsError(message);
      console.error("Automatic TTS folder conversion failed:", message);
    }).then((dispose) => {
      if (disposed) dispose();
      else unlisten = dispose;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [t]);

  const chooseProvider = (provider: TtsProvider) =>
    updateTts({ provider }, "provider");

  const saveKey = async () => {
    if (!keyDraft.trim()) return;
    setKeyBusy(true);
    setSettingsError(null);
    try {
      await invoke("tts_set_api_key", {
        provider: tts.provider,
        apiKey: keyDraft.trim(),
      });
      setHasSeparateKey(true);
      setEditingKey(false);
      setKeyDraft("");
    } catch (error) {
      setSettingsError(asErrorMessage(error));
    } finally {
      setKeyBusy(false);
    }
  };

  const clearKey = async () => {
    setKeyBusy(true);
    setSettingsError(null);
    try {
      await invoke("tts_clear_api_key", { provider: tts.provider });
      setHasSeparateKey(false);
      setEditingKey(false);
      setKeyDraft("");
    } catch (error) {
      setSettingsError(asErrorMessage(error));
    } finally {
      setKeyBusy(false);
    }
  };

  const addRule = async () => {
    if (!ruleFrom) return;
    const rule: TtsReplacementRule = {
      id: `tts_rule_${Date.now()}`,
      from: ruleFrom,
      to: ruleTo,
      enabled: true,
      case_sensitive: ruleCaseSensitive,
      is_regex: ruleIsRegex,
    };
    await updateTts(
      { preprocessing_rules: [...tts.preprocessing_rules, rule] },
      "preprocessing_rules",
    );
    setRuleFrom("");
    setRuleTo("");
  };

  const patchRule = (id: string, patch: Partial<TtsReplacementRule>) =>
    updateTts(
      {
        preprocessing_rules: tts.preprocessing_rules.map((rule) =>
          rule.id === id ? { ...rule, ...patch } : rule,
        ),
      },
      "preprocessing_rules",
    );

  const removeRule = (id: string) =>
    updateTts(
      {
        preprocessing_rules: tts.preprocessing_rules.filter(
          (rule) => rule.id !== id,
        ),
      },
      "preprocessing_rules",
    );

  const selectPromptPreset = (id: string) => {
    const preset = tts.prompt_presets.find((item) => item.id === id);
    return updateTts(
      {
        selected_prompt_id: preset?.id ?? "",
        openai_instructions: preset?.instructions ?? tts.openai_instructions,
      },
      "selected_prompt_id",
    );
  };

  const savePromptPreset = async () => {
    const name = promptPresetName.trim();
    if (!name || !tts.openai_instructions.trim()) return;
    const existing = tts.prompt_presets.find(
      (item) => item.name.toLocaleLowerCase() === name.toLocaleLowerCase(),
    );
    const preset: TtsPromptPreset = {
      id: existing?.id ?? `tts_prompt_${Date.now()}`,
      name,
      instructions: tts.openai_instructions,
    };
    const promptPresets = existing
      ? tts.prompt_presets.map((item) =>
          item.id === existing.id ? preset : item,
        )
      : [...tts.prompt_presets, preset];
    await updateTts(
      {
        prompt_presets: promptPresets,
        selected_prompt_id: preset.id,
      },
      "prompt_presets",
    );
    setPromptPresetName("");
  };

  const deleteSelectedPromptPreset = () => {
    if (!tts.selected_prompt_id) return Promise.resolve();
    return updateTts(
      {
        prompt_presets: tts.prompt_presets.filter(
          (item) => item.id !== tts.selected_prompt_id,
        ),
        selected_prompt_id: "",
      },
      "prompt_presets",
    );
  };

  const chooseInputFile = async () => {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [
        {
          name: t("textToSpeech.conversion.textMarkdownFiles"),
          extensions: ["txt", "md"],
        },
      ],
    });
    if (typeof selected !== "string") return;
    setInputPath(selected);
    setOutputPath("");
    setInspection(null);
    setCompletedPath("");
    setConversionError(null);
  };

  const inspectFile = async () => {
    if (!inputPath) return;
    setInspecting(true);
    setConversionError(null);
    try {
      const result = await invoke<FileInspection>("inspect_tts_text_file", {
        path: inputPath,
      });
      setInspection(result);
    } catch (error) {
      setInspection(null);
      setConversionError(asErrorMessage(error));
    } finally {
      setInspecting(false);
    }
  };

  const chooseOutputFile = async () => {
    const selected = await save({
      defaultPath: outputPath || undefined,
      filters: [
        {
          name:
            outputFormat === "mp3"
              ? t("textToSpeech.conversion.mp3Audio")
              : t("textToSpeech.conversion.wavAudio"),
          extensions: [outputFormat],
        },
      ],
    });
    if (selected) setOutputPath(selected);
  };

  const convertFile = async () => {
    if (!inputPath || !outputPath || !inspection) return;
    setConversionBusy(true);
    setConversionError(null);
    setCompletedPath("");
    setConversionProgress(null);
    setOperationId(null);
    conversionOperationIdRef.current = null;
    conversionBusyRef.current = true;
    try {
      const result = await invoke<
        | string
        | {
            operation_id?: string;
            operationId?: string;
            output_path?: string;
            outputPath?: string;
            resumed_chunks?: number;
            resumedChunks?: number;
          }
      >("convert_tts_text_file", {
        request: {
          inputPath,
          outputPath,
          outputFormat,
          mp3Bitrate,
        },
      });
      if (typeof result === "string") {
        setCompletedPath(result);
      } else {
        const returnedOperationId = result.operation_id ?? result.operationId;
        setOperationId(
          returnedOperationId === undefined
            ? null
            : String(returnedOperationId),
        );
        conversionOperationIdRef.current =
          returnedOperationId === undefined
            ? null
            : String(returnedOperationId);
        const finalPath = result.output_path ?? result.outputPath;
        if (finalPath) setCompletedPath(finalPath);
        const resumedChunks =
          result.resumed_chunks ?? result.resumedChunks ?? 0;
        if (resumedChunks > 0) {
          setConversionProgress((previous) => ({
            ...previous,
            message: t("textToSpeech.conversion.resumeRecovered", {
              count: resumedChunks,
            }),
          }));
        }
      }
    } catch (error) {
      setConversionError(asErrorMessage(error));
      console.error("TTS file conversion failed:", error);
    } finally {
      conversionBusyRef.current = false;
      setConversionBusy(false);
    }
  };

  const cancelConversion = async () => {
    try {
      await invoke("cancel_tts_operation", {
        operationId,
      });
    } catch (error) {
      setConversionError(asErrorMessage(error));
    }
  };

  const completedChunks =
    conversionProgress?.completed_chunks ??
    conversionProgress?.completedChunks ??
    0;
  const totalChunks =
    conversionProgress?.total_chunks ??
    conversionProgress?.totalChunks ??
    chunkCount(inspection) ??
    0;
  const progressPercent =
    totalChunks > 0
      ? Math.min(100, Math.round((completedChunks / totalChunks) * 100))
      : 0;
  const conversionStatus =
    conversionProgress?.status === "retrying"
      ? t("textToSpeech.overlayPlayer.retryingAttempt", {
          attempt: conversionProgress.attempt ?? 1,
        })
      : conversionProgress?.status === "preparing"
        ? t("textToSpeech.overlayPlayer.preparing")
        : t("textToSpeech.conversion.converting");
  const keySourceLabel =
    tts.provider === "openai"
      ? t("textToSpeech.api.sameAsOpenAi")
      : t("textToSpeech.api.sameAsStt", {
          provider: tts.provider === "soniox" ? "Soniox" : "Deepgram",
        });

  useEffect(() => {
    setOutputFormat(tts.output_format);
    setMp3Bitrate(tts.mp3_bitrate_kbps);
  }, [tts.mp3_bitrate_kbps, tts.output_format]);

  return (
    <div className="mx-auto w-full max-w-3xl space-y-6 pb-12">
      {settingsError && (
        <div
          role="alert"
          className="flex items-start gap-2 rounded-lg border border-red-500/35 bg-red-500/10 px-4 py-3 text-sm text-red-200"
        >
          <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" />
          <span>{settingsError}</span>
        </div>
      )}

      <SettingsGroup
        title={t("textToSpeech.title")}
        description={t("textToSpeech.description")}
      >
        <ToggleSwitch
          grouped
          checked={tts.enabled}
          onChange={(enabled) => void updateTts({ enabled }, "enabled")}
          isUpdating={savingField === "enabled"}
          label={t("textToSpeech.enable.label")}
          description={t("textToSpeech.enable.description")}
          descriptionMode="inline"
        />
        <SettingContainer
          grouped
          title={t("textToSpeech.provider.title")}
          description={t("textToSpeech.provider.description")}
        >
          <Dropdown
            selectedValue={tts.provider}
            options={PROVIDERS}
            onSelect={(value) => void chooseProvider(value as TtsProvider)}
            disabled={savingField !== null}
            dropUp={false}
          />
        </SettingContainer>
      </SettingsGroup>

      <SettingsGroup
        title={t("textToSpeech.api.title")}
        description={t("textToSpeech.api.description")}
      >
        <SettingContainer
          grouped
          layout="stacked"
          title={t("textToSpeech.api.sourceTitle")}
          description={t("textToSpeech.api.sourceDescription")}
          descriptionMode="inline"
        >
          <Dropdown
            className="max-w-md"
            selectedValue={keySource}
            options={[
              { value: "shared", label: keySourceLabel },
              {
                value: "separate",
                label: t("textToSpeech.api.separateKey"),
              },
            ]}
            onSelect={(value) =>
              void updateTts(
                { [keySourceField]: value as TtsKeySource },
                keySourceField,
              )
            }
            disabled={savingField !== null}
            dropUp={false}
          />
        </SettingContainer>
        {keySource === "separate" && (
          <SettingContainer
            grouped
            layout="stacked"
            title={t("textToSpeech.api.providerKeyTitle", {
              provider: PROVIDERS.find((item) => item.value === tts.provider)
                ?.label,
            })}
            description={t("textToSpeech.api.keyStorageDescription")}
            descriptionMode="inline"
          >
            {hasSeparateKey && !editingKey ? (
              <StoredApiKeyDisplay
                loading={keyBusy}
                onDelete={() => void clearKey()}
                onReplace={() => setEditingKey(true)}
              />
            ) : (
              <ApiKeyEditor
                loading={keyBusy}
                value={keyDraft}
                onChange={setKeyDraft}
                onSave={() => void saveKey()}
                onCancel={() => {
                  setEditingKey(false);
                  setKeyDraft("");
                }}
                showCancel={hasSeparateKey}
                placeholder={t("textToSpeech.api.keyPlaceholder")}
                hint={
                  hasSeparateKey
                    ? t("textToSpeech.api.replaceKeyHint")
                    : t("textToSpeech.api.noKeyHint")
                }
              />
            )}
          </SettingContainer>
        )}
      </SettingsGroup>

      <SettingsGroup
        title={t("textToSpeech.voice.title")}
        description={t("textToSpeech.voice.description")}
      >
        <SettingContainer
          grouped
          title={t("textToSpeech.voice.voiceTitle")}
          description={t("textToSpeech.voice.voiceDescription")}
        >
          <Input
            className="w-full md:w-72"
            value={voiceValue}
            placeholder={
              tts.provider === "deepgram"
                ? "aura-2-thalia-en"
                : t("textToSpeech.voice.voicePlaceholder")
            }
            onChange={(event) => {
              const value = event.target.value;
              if (tts.provider === "soniox") {
                void updateTts({ soniox_voice: value }, "soniox_voice");
              } else if (tts.provider === "deepgram") {
                void updateTts({ deepgram_model: value }, "deepgram_model");
              } else {
                void updateTts({ openai_voice: value }, "openai_voice");
              }
            }}
          />
        </SettingContainer>
        {tts.provider === "soniox" && (
          <SettingContainer
            grouped
            title={t("textToSpeech.voice.languageTitle")}
            description={t("textToSpeech.voice.languageDescription")}
          >
            <Input
              className="w-full md:w-72"
              value={tts.soniox_language}
              placeholder="en"
              onChange={(event) =>
                void updateTts(
                  { soniox_language: event.target.value },
                  "soniox_language",
                )
              }
            />
          </SettingContainer>
        )}
        {tts.provider !== "deepgram" && (
          <SettingContainer
            grouped
            title={t("textToSpeech.voice.modelTitle")}
            description={t("textToSpeech.voice.modelDescription")}
          >
            <Input
              className="w-full md:w-72"
              value={modelValue}
              onChange={(event) =>
                void updateTts(
                  tts.provider === "soniox"
                    ? { soniox_model: event.target.value }
                    : { openai_model: event.target.value },
                  tts.provider === "soniox" ? "soniox_model" : "openai_model",
                )
              }
            />
          </SettingContainer>
        )}
        <SettingContainer
          grouped
          title={t("textToSpeech.voice.speedTitle")}
          description={t("textToSpeech.voice.speedDescription", {
            minimum: speedMinimum,
            maximum: speedMaximum,
          })}
        >
          <Input
            className="w-28"
            type="number"
            min={speedMinimum}
            max={speedMaximum}
            step={0.05}
            value={tts.speed}
            onChange={(event) =>
              void updateTts(
                {
                  speed: clampNumber(
                    event.target.value,
                    speedMinimum,
                    speedMaximum,
                    1,
                  ),
                },
                "speed",
              )
            }
          />
        </SettingContainer>
        <SettingContainer
          grouped
          layout="stacked"
          title={t("textToSpeech.voice.resourcesTitle")}
          description={t("textToSpeech.voice.resourcesDescription")}
          descriptionMode="inline"
        >
          <div className="flex flex-wrap gap-3">
            <a
              className="inline-flex items-center gap-1 text-sm text-[#d7b9ff] underline-offset-4 hover:underline"
              href={voiceResources.voices}
              target="_blank"
              rel="noopener noreferrer"
            >
              {t("textToSpeech.voice.voiceDocs")}
              <ExternalLink className="h-3.5 w-3.5" aria-hidden="true" />
            </a>
            {voiceResources.playground && (
              <a
                className="inline-flex items-center gap-1 text-sm text-[#d7b9ff] underline-offset-4 hover:underline"
                href={voiceResources.playground}
                target="_blank"
                rel="noopener noreferrer"
              >
                {t("textToSpeech.voice.playground")}
                <ExternalLink className="h-3.5 w-3.5" aria-hidden="true" />
              </a>
            )}
          </div>
        </SettingContainer>
        {tts.provider === "openai" && (
          <>
            {!openAiInstructionsSupported && (
              <div
                role="status"
                className="mx-6 my-4 rounded-lg border border-amber-400/40 bg-amber-400/10 px-4 py-3 text-xs leading-relaxed text-amber-100"
              >
                {t("textToSpeech.prompts.unsupportedModel")}
              </div>
            )}
            <SettingContainer
              grouped
              layout="stacked"
              title={t("textToSpeech.prompts.presetTitle")}
              description={t("textToSpeech.prompts.presetDescription")}
              descriptionMode="inline"
            >
              <div className="flex flex-wrap gap-2">
                <Dropdown
                  className="min-w-64"
                  selectedValue={tts.selected_prompt_id}
                  options={[
                    {
                      value: "",
                      label: t("textToSpeech.prompts.customUnsaved"),
                    },
                    ...tts.prompt_presets.map((preset) => ({
                      value: preset.id,
                      label: preset.name,
                    })),
                  ]}
                  onSelect={(value) => void selectPromptPreset(value)}
                  disabled={savingField !== null}
                  dropUp={false}
                />
                <Button
                  variant="danger"
                  disabled={!tts.selected_prompt_id || savingField !== null}
                  onClick={() => void deleteSelectedPromptPreset()}
                >
                  <Trash2 className="mr-2 inline h-4 w-4" />
                  {t("textToSpeech.prompts.deletePreset")}
                </Button>
              </div>
            </SettingContainer>
            <SettingContainer
              grouped
              layout="stacked"
              title={t("textToSpeech.prompts.instructionsTitle")}
              description={t("textToSpeech.prompts.instructionsDescription")}
              descriptionMode="inline"
            >
              <Textarea
                className="w-full"
                value={tts.openai_instructions}
                placeholder={t("textToSpeech.prompts.instructionsPlaceholder")}
                onChange={(event) =>
                  void updateTts(
                    {
                      openai_instructions: event.target.value,
                      selected_prompt_id: "",
                    },
                    "openai_instructions",
                  )
                }
              />
              <div className="mt-3 flex flex-wrap gap-2">
                <Input
                  className="min-w-64 flex-1"
                  value={promptPresetName}
                  placeholder={t("textToSpeech.prompts.namePlaceholder")}
                  onChange={(event) => setPromptPresetName(event.target.value)}
                />
                <Button
                  variant="secondary"
                  disabled={
                    !promptPresetName.trim() ||
                    !tts.openai_instructions.trim() ||
                    savingField !== null
                  }
                  onClick={() => void savePromptPreset()}
                >
                  <SaveIcon className="mr-2 inline h-4 w-4" />
                  {t("textToSpeech.prompts.saveNamed")}
                </Button>
              </div>
            </SettingContainer>
            <div className="px-6 py-4 text-xs text-amber-200/90">
              {t("textToSpeech.prompts.aiDisclosure")}
            </div>
          </>
        )}
      </SettingsGroup>

      <SettingsGroup title={t("textToSpeech.actions.title")}>
        <SettingContainer
          grouped
          layout="stacked"
          title={t("textToSpeech.actions.readClipboardTitle")}
          description={t("textToSpeech.actions.readClipboardDescription")}
          descriptionMode="inline"
        >
          <HandyShortcut shortcutId="read_clipboard" grouped />
        </SettingContainer>
        <SettingContainer
          grouped
          layout="stacked"
          title={t("textToSpeech.actions.readSelectionTitle")}
          description={t("textToSpeech.actions.readSelectionDescription")}
          descriptionMode="inline"
        >
          <HandyShortcut shortcutId="read_selection_tts" grouped />
        </SettingContainer>
      </SettingsGroup>

      <SettingsGroup
        title={t("textToSpeech.overlay.title")}
        description={t("textToSpeech.overlay.description")}
      >
        <ToggleSwitch
          grouped
          checked={tts.autoplay}
          onChange={(autoplay) => void updateTts({ autoplay }, "autoplay")}
          isUpdating={savingField === "autoplay"}
          label={t("textToSpeech.overlay.autoplayLabel")}
          description={t("textToSpeech.overlay.autoplayDescription")}
          descriptionMode="inline"
        />
        <SettingContainer
          grouped
          title={t("textToSpeech.overlay.playPauseTitle")}
          description={t("textToSpeech.overlay.playPauseDescription")}
        >
          <HotkeyCapture
            value={tts.play_pause_hotkey}
            isCapturing={capturingHotkey === "play_pause"}
            onStartCapture={() => setCapturingHotkey("play_pause")}
            onCaptured={(value) => {
              setCapturingHotkey(null);
              void updateTts({ play_pause_hotkey: value }, "play_pause_hotkey");
            }}
            onCancel={() => setCapturingHotkey(null)}
            onClear={() =>
              void updateTts({ play_pause_hotkey: "" }, "play_pause_hotkey")
            }
            osType={hotkeyOsType}
          />
        </SettingContainer>
        <SettingContainer
          grouped
          title={t("textToSpeech.overlay.stopTitle")}
          description={t("textToSpeech.overlay.stopDescription")}
        >
          <HotkeyCapture
            value={tts.stop_hotkey}
            isCapturing={capturingHotkey === "stop"}
            onStartCapture={() => setCapturingHotkey("stop")}
            onCaptured={(value) => {
              setCapturingHotkey(null);
              void updateTts({ stop_hotkey: value }, "stop_hotkey");
            }}
            onCancel={() => setCapturingHotkey(null)}
            onClear={() => void updateTts({ stop_hotkey: "" }, "stop_hotkey")}
            osType={hotkeyOsType}
          />
        </SettingContainer>
      </SettingsGroup>

      <SettingsGroup
        title={t("textToSpeech.preprocessing.title")}
        description={t("textToSpeech.preprocessing.description")}
      >
        <ToggleSwitch
          grouped
          checked={tts.preprocessing_enabled}
          onChange={(enabled) =>
            void updateTts(
              { preprocessing_enabled: enabled },
              "preprocessing_enabled",
            )
          }
          isUpdating={savingField === "preprocessing_enabled"}
          label={t("textToSpeech.preprocessing.enableLabel")}
          description={t("textToSpeech.preprocessing.enableDescription")}
          descriptionMode="inline"
        />
        <div className="space-y-3 px-6 py-4">
          <div className="grid gap-2 md:grid-cols-[1fr_1fr_auto]">
            <Input
              value={ruleFrom}
              onChange={(event) => setRuleFrom(event.target.value)}
              placeholder={
                ruleIsRegex
                  ? t("textToSpeech.preprocessing.regexPlaceholder")
                  : t("textToSpeech.preprocessing.findPlaceholder")
              }
            />
            <Input
              value={ruleTo}
              onChange={(event) => setRuleTo(event.target.value)}
              placeholder={t(
                "textToSpeech.preprocessing.replacementPlaceholder",
              )}
            />
            <Button
              size="sm"
              disabled={!ruleFrom}
              onClick={() => void addRule()}
            >
              <Plus className="mr-1 inline h-4 w-4" />
              {t("textToSpeech.preprocessing.add")}
            </Button>
          </div>
          <div className="flex flex-wrap gap-5">
            <label className="flex items-center gap-2 text-xs text-[#b8b8b8]">
              <input
                type="checkbox"
                checked={ruleIsRegex}
                onChange={(event) => setRuleIsRegex(event.target.checked)}
              />
              {t("textToSpeech.preprocessing.regularExpression")}
            </label>
            <label className="flex items-center gap-2 text-xs text-[#b8b8b8]">
              <input
                type="checkbox"
                checked={ruleCaseSensitive}
                onChange={(event) => setRuleCaseSensitive(event.target.checked)}
              />
              {t("textToSpeech.preprocessing.caseSensitive")}
            </label>
          </div>
        </div>
        {tts.preprocessing_rules.map((rule) => (
          <div key={rule.id} className="flex items-center gap-3 px-6 py-3">
            <input
              type="checkbox"
              checked={rule.enabled}
              onChange={(event) =>
                void patchRule(rule.id, { enabled: event.target.checked })
              }
              aria-label={t("textToSpeech.preprocessing.enableRule", {
                value: rule.from,
              })}
            />
            <div className="min-w-0 flex-1">
              <div className="break-words text-sm text-[#f5f5f5]">
                <span className="font-mono">{rule.from}</span>
                <span className="mx-2 text-[#707070]">→</span>
                <span className="font-mono">
                  {rule.to || t("textToSpeech.preprocessing.deleteValue")}
                </span>
              </div>
              <div className="mt-1 text-xs text-[#808080]">
                {rule.is_regex
                  ? t("textToSpeech.preprocessing.regex")
                  : t("textToSpeech.preprocessing.literal")}{" "}
                ·{" "}
                {rule.case_sensitive
                  ? t("textToSpeech.preprocessing.caseSensitiveLower")
                  : t("textToSpeech.preprocessing.caseInsensitive")}
              </div>
            </div>
            <button
              type="button"
              className="rounded p-2 text-[#808080] hover:bg-red-500/10 hover:text-red-300"
              onClick={() => void removeRule(rule.id)}
              aria-label={t("textToSpeech.preprocessing.deleteRule")}
            >
              <Trash2 className="h-4 w-4" />
            </button>
          </div>
        ))}
        {tts.preprocessing_rules.length === 0 && (
          <div className="px-6 py-4 text-sm text-[#808080]">
            {t("textToSpeech.preprocessing.empty")}
          </div>
        )}
      </SettingsGroup>

      <SettingsGroup
        title={t("textToSpeech.chunking.title")}
        description={t("textToSpeech.chunking.description", {
          limit: PROVIDER_INPUT_LIMITS[tts.provider],
        })}
      >
        {[
          {
            key: "retry_count" as const,
            title: t("textToSpeech.chunking.retryCountTitle"),
            description: t("textToSpeech.chunking.retryCountDescription"),
            min: 0,
            max: 10,
            step: 1,
          },
          {
            key: "retry_base_delay_ms" as const,
            title: t("textToSpeech.chunking.retryDelayTitle"),
            description: t("textToSpeech.chunking.retryDelayDescription"),
            min: 100,
            max: 30000,
            step: 50,
          },
          {
            key: "interactive_target_chars" as const,
            title: t("textToSpeech.chunking.interactiveTargetTitle"),
            description: t(
              "textToSpeech.chunking.interactiveTargetDescription",
            ),
            min: 50,
            max: 5000,
            step: 50,
          },
          {
            key: "file_target_chars" as const,
            title: t("textToSpeech.chunking.fileTargetTitle"),
            description: t("textToSpeech.chunking.fileTargetDescription"),
            min: 50,
            max: 5000,
            step: 50,
          },
          {
            key: "inter_chunk_pause_ms" as const,
            title: t("textToSpeech.chunking.chunkPauseTitle"),
            description: t("textToSpeech.chunking.chunkPauseDescription"),
            min: 0,
            max: 5000,
            step: 25,
          },
          {
            key: "paragraph_pause_ms" as const,
            title: t("textToSpeech.chunking.paragraphPauseTitle"),
            description: t("textToSpeech.chunking.paragraphPauseDescription"),
            min: 0,
            max: 10000,
            step: 50,
          },
        ].map((item) => (
          <SettingContainer
            key={item.key}
            grouped
            title={item.title}
            description={item.description}
          >
            <Input
              className="w-28"
              type="number"
              min={item.min}
              max={item.max}
              step={item.step}
              value={tts[item.key]}
              onChange={(event) =>
                void updateTts(
                  {
                    [item.key]: clampNumber(
                      event.target.value,
                      item.min,
                      item.max,
                      DEFAULT_TTS_SETTINGS[item.key],
                    ),
                  },
                  item.key,
                )
              }
            />
          </SettingContainer>
        ))}
      </SettingsGroup>

      <TtsFolderAutomation
        tts={tts}
        savingField={savingField}
        updateTts={updateTts}
      />

      <SettingsGroup
        title={t("textToSpeech.conversion.title")}
        description={t("textToSpeech.conversion.description")}
      >
        <details className="group border-b border-white/[0.05] px-6 py-4">
          <summary className="cursor-pointer select-none text-sm font-medium text-[#d7b9ff]">
            {t("textToSpeech.conversion.cliHelpTitle")}
          </summary>
          <div className="mt-3 space-y-2 text-xs leading-relaxed text-[#a0a0a0]">
            <p>{t("textToSpeech.conversion.cliHelpDescription")}</p>
            <code className="block overflow-x-auto rounded-lg bg-black/30 p-3 text-[#e8e8e8]">
              {t("textToSpeech.conversion.cliExample")}
            </code>
            <code className="block overflow-x-auto rounded-lg bg-black/30 p-3 text-[#e8e8e8]">
              {t("textToSpeech.conversion.cliPromptExample")}
            </code>
            <p>{t("textToSpeech.conversion.cliLongInstructions")}</p>
            <p>{t("textToSpeech.conversion.cliHistory")}</p>
            <code className="block overflow-x-auto rounded-lg bg-black/30 p-3 text-[#e8e8e8]">
              {t("textToSpeech.conversion.cliRegenerateExample")}
            </code>
            <p>{t("textToSpeech.conversion.cliRegenerateDescription")}</p>
          </div>
        </details>
        <SettingContainer
          grouped
          layout="stacked"
          title={t("textToSpeech.conversion.sourceTitle")}
          description={t("textToSpeech.conversion.sourceDescription")}
          descriptionMode="inline"
        >
          <div className="flex gap-2">
            <Input
              className="min-w-0 flex-1"
              readOnly
              value={inputPath}
              placeholder={t("textToSpeech.conversion.noFileSelected")}
            />
            <Button variant="secondary" onClick={() => void chooseInputFile()}>
              <FileText className="mr-2 inline h-4 w-4" />
              {t("textToSpeech.conversion.chooseFile")}
            </Button>
          </div>
          <div className="mt-3">
            <Button
              variant="secondary"
              disabled={!inputPath || inspecting || conversionBusy}
              onClick={() => void inspectFile()}
            >
              {inspecting && (
                <Loader2 className="mr-2 inline h-4 w-4 animate-spin" />
              )}
              {t("textToSpeech.conversion.inspect")}
            </Button>
          </div>
          {inspection && (
            <div className="mt-3 grid grid-cols-3 gap-3">
              {[
                [
                  t("textToSpeech.conversion.sourceCharacters"),
                  characterCount(inspection, "source"),
                ],
                [
                  t("textToSpeech.conversion.processedCharacters"),
                  characterCount(inspection, "processed"),
                ],
                [t("textToSpeech.conversion.chunks"), chunkCount(inspection)],
              ].map(([label, value]) => (
                <div
                  key={String(label)}
                  className="rounded-lg border border-white/[0.06] bg-black/15 p-3"
                >
                  <div className="text-xs text-[#808080]">{label}</div>
                  <div className="mt-1 text-lg font-semibold text-[#f5f5f5]">
                    {value ?? "—"}
                  </div>
                </div>
              ))}
            </div>
          )}
        </SettingContainer>

        <SettingContainer
          grouped
          title={t("textToSpeech.conversion.finalFormatTitle")}
          description={t("textToSpeech.conversion.finalFormatDescription")}
        >
          <Dropdown
            selectedValue={outputFormat}
            options={[
              { value: "mp3", label: "MP3" },
              { value: "wav", label: "WAV" },
            ]}
            onSelect={(value) => {
              const format = value as TtsOutputFormat;
              setOutputFormat(format);
              setOutputPath("");
              void updateTts({ output_format: format }, "output_format");
            }}
            disabled={conversionBusy}
            dropUp={false}
          />
        </SettingContainer>
        {outputFormat === "mp3" && (
          <SettingContainer
            grouped
            title={t("textToSpeech.conversion.bitrateTitle")}
            description={t("textToSpeech.conversion.bitrateDescription")}
          >
            <Dropdown
              selectedValue={String(mp3Bitrate)}
              options={BITRATES.map((bitrate) => ({
                value: String(bitrate),
                label: `${bitrate} kb/s`,
              }))}
              onSelect={(value) => {
                const bitrate = Number(value);
                setMp3Bitrate(bitrate);
                void updateTts(
                  { mp3_bitrate_kbps: bitrate },
                  "mp3_bitrate_kbps",
                );
              }}
              disabled={conversionBusy}
              dropUp={false}
            />
          </SettingContainer>
        )}
        <SettingContainer
          grouped
          layout="stacked"
          title={t("textToSpeech.conversion.outputTitle")}
          description={t("textToSpeech.conversion.outputDescription")}
          descriptionMode="inline"
        >
          <div className="flex gap-2">
            <Input
              className="min-w-0 flex-1"
              readOnly
              value={outputPath}
              placeholder={t("textToSpeech.conversion.noOutputSelected")}
            />
            <Button
              variant="secondary"
              disabled={!inputPath || conversionBusy}
              onClick={() => void chooseOutputFile()}
            >
              <FileAudio className="mr-2 inline h-4 w-4" />
              {t("textToSpeech.conversion.saveAs")}
            </Button>
          </div>
        </SettingContainer>

        {(conversionBusy || conversionProgress) && (
          <div className="space-y-2 px-6 py-4">
            <div className="flex items-center justify-between text-xs text-[#b8b8b8]">
              <span>
                {conversionStatus}{" "}
                {totalChunks > 0
                  ? t("textToSpeech.conversion.chunkProgress", {
                      completed: completedChunks,
                      total: totalChunks,
                    })
                  : ""}
              </span>
              <span>{progressPercent}%</span>
            </div>
            <div className="h-2 overflow-hidden rounded-full bg-[#252525]">
              <div
                className="h-full rounded-full bg-[#9b5de5] transition-[width]"
                style={{ width: `${progressPercent}%` }}
              />
            </div>
            {conversionProgress?.message && (
              <p className="text-xs text-[#a0a0a0]">
                {conversionProgress.message}
              </p>
            )}
          </div>
        )}

        <div className="flex flex-wrap items-center gap-3 px-6 py-4">
          <Button
            disabled={!inspection || !outputPath || conversionBusy}
            onClick={() => void convertFile()}
          >
            {conversionBusy && (
              <Loader2 className="mr-2 inline h-4 w-4 animate-spin" />
            )}
            {t("textToSpeech.conversion.convert")}
          </Button>
          {conversionBusy && (
            <Button
              variant="danger"
              disabled={!operationId}
              onClick={() => void cancelConversion()}
            >
              {t("textToSpeech.conversion.cancel")}
            </Button>
          )}
        </div>

        {conversionError && (
          <div
            role="alert"
            className="flex items-start gap-2 px-6 py-4 text-sm text-red-300"
          >
            <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" />
            <span>{conversionError}</span>
          </div>
        )}
        {completedPath && (
          <div
            role="status"
            aria-live="polite"
            className="flex items-start gap-2 px-6 py-4 text-sm text-green-300"
          >
            <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0" />
            <span className="min-w-0 break-all">
              {t("textToSpeech.conversion.completed", {
                path: completedPath,
              })}
            </span>
          </div>
        )}
      </SettingsGroup>

      <TtsHistory tts={tts} savingField={savingField} updateTts={updateTts} />
    </div>
  );
};

export default TextToSpeechSettings;
