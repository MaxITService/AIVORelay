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
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { type as getOsType } from "@tauri-apps/plugin-os";
import { useTranslation } from "react-i18next";
import {
  AlertCircle,
  CheckCircle2,
  ExternalLink,
  FileAudio,
  FileText,
  FolderOpen,
  Loader2,
  Plus,
  RefreshCw,
  Save as SaveIcon,
  Sparkles,
  Square,
  Trash2,
  Volume2,
} from "lucide-react";

import { useSettings } from "@/hooks/useSettings";
import { scrollAndFocusAnchor } from "@/lib/anchorNavigation";
import { ApiKeyEditor, StoredApiKeyDisplay } from "../ApiKeyControls";
import { HandyShortcut } from "../HandyShortcut";
import { TtsFolderAutomation } from "./TtsFolderAutomation";
import { TtsBatchConversion } from "./TtsBatchConversion";
import { TtsBetaBanner } from "./TtsBetaBanner";
import { TtsHelpDisclosure } from "./TtsHelpDisclosure";
import { TtsHistory } from "./TtsHistory";
import { TtsUnfinishedJobs } from "./TtsUnfinishedJobs";
import { CommittedNumberInput } from "./CommittedNumberInput";
import {
  DEFAULT_TTS_LLM_PREPROCESSING,
  TtsAiCleanup,
  type TtsLlmPreprocessingSettings,
} from "./TtsAiCleanup";
import { Button } from "@/components/ui/Button";
import { ConfirmationModal } from "@/components/ui/ConfirmationModal";
import { HotkeyCapture } from "@/components/ui/HotkeyCapture";
import { Input } from "@/components/ui/Input";
import { Select, type SelectOption } from "@/components/ui/Select";
import { SettingContainer } from "@/components/ui/SettingContainer";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { Textarea } from "@/components/ui/Textarea";
import { ToggleSwitch } from "@/components/ui/ToggleSwitch";
import { Tooltip } from "@/components/ui/Tooltip";
import { sessionToast as toast } from "@/lib/sessionToast";
import type { OSType } from "@/lib/utils/keyboard";
import {
  AIVORELAY_TTS_GUIDE_URL,
  CARTESIA_MODEL_OPTIONS,
  ELEVENLABS_MODEL_OPTIONS,
  LOCAL_TTS_INSTALL_METADATA,
  OPENAI_MODEL_OPTIONS,
  MURF_MODEL_OPTIONS,
  SONIOX_LANGUAGE_OPTIONS,
  SONIOX_MODEL_OPTIONS,
  TTS_PROVIDER_DEFAULTS,
  TTS_PROVIDER_DOCUMENTATION,
  TTS_PROVIDER_OPTIONS,
  TTS_PROVIDER_SPEED_RANGES,
  type TtsProvider,
} from "@/lib/tts/ttsProviderMetadata";

type LocalTtsKind = "qwen" | "kokoro";
type TtsKeySource = "shared" | "separate";
type TtsOutputFormat = "mp3" | "wav";
type TtsPlaybackEffect = "none" | "radio" | "retro";
type TtsOperationScope = "interactive" | "file";
type ElevenLabsTextNormalization = "auto" | "on" | "off";

type TtsReplacementRule = {
  id: string;
  from: string;
  to: string;
  enabled: boolean;
  case_sensitive: boolean;
  is_regex: boolean;
};

type TtsDeleteConfirmation =
  | { kind: "synthesisPreset"; name: string }
  | { kind: "promptPreset"; name: string }
  | { kind: "localModel" };

const ActionTooltip: React.FC<{
  content?: string | null;
  children: React.ReactNode;
}> = ({ content, children }) =>
  content ? <Tooltip content={content}>{children}</Tooltip> : children;

const waitForUiTick = (delayMs: number) =>
  new Promise<void>((resolve) => window.setTimeout(resolve, delayMs));

type TtsPromptPreset = {
  id: string;
  name: string;
  instructions: string;
};

type TtsSynthesisConfig = {
  provider: TtsProvider;
  model: string;
  voice: string;
  language: string;
  key_source: TtsKeySource;
  openai_compatible_base_url?: string;
  openai_compatible_allow_insecure_http?: boolean;
  speed: number;
  murf_rate: number;
  murf_pitch: number;
  murf_variation: number;
  murf_style: string | null;
  elevenlabs_stability: number;
  elevenlabs_similarity_boost: number;
  elevenlabs_style: number;
  elevenlabs_use_speaker_boost: boolean;
  elevenlabs_apply_text_normalization: ElevenLabsTextNormalization;
  cartesia_emotion: string | null;
  cartesia_volume: number;
  voice_instructions: string;
  voice_prompt_preset_id: string;
  preprocessing_enabled: boolean;
  preprocessing_rules: TtsReplacementRule[];
  target_chars: number;
  retry_count: number;
  retry_base_delay_ms: number;
  inter_chunk_pause_ms: number;
  paragraph_pause_ms: number;
  output_format: TtsOutputFormat;
  mp3_bitrate_kbps: number;
};

type TtsModelSynthesisSettings = {
  model_key: string;
  config: TtsSynthesisConfig;
};

type TtsScopeSynthesisSettings = {
  active_model_key: string;
  selected_preset_id: string;
  models: TtsModelSynthesisSettings[];
  active_models_by_provider: Array<{
    provider: TtsProvider;
    model_key: string;
  }>;
  initialized_providers: TtsProvider[];
};

type TtsSynthesisPreset = {
  id: string;
  name: string;
  config: TtsSynthesisConfig;
};

const isBuiltinTtsSynthesisPreset = (preset: TtsSynthesisPreset) =>
  preset.id.startsWith("builtin_tts_");

type TtsSettings = {
  enabled: boolean;
  provider: TtsProvider;
  soniox_key_source: TtsKeySource;
  deepgram_key_source: TtsKeySource;
  openai_key_source: TtsKeySource;
  openai_compatible_key_source: TtsKeySource;
  murf_key_source: TtsKeySource;
  elevenlabs_key_source: TtsKeySource;
  cartesia_key_source: TtsKeySource;
  soniox_model: string;
  soniox_language: string;
  soniox_voice: string;
  deepgram_model: string;
  openai_model: string;
  openai_voice: string;
  openai_compatible_base_url: string;
  openai_compatible_allow_insecure_http: boolean;
  openai_compatible_model: string;
  openai_compatible_voice: string;
  murf_model: string;
  murf_voice: string;
  murf_language: string;
  murf_rate: number;
  murf_pitch: number;
  murf_variation: number;
  murf_style: string | null;
  elevenlabs_model: string;
  elevenlabs_voice: string;
  elevenlabs_language: string;
  elevenlabs_stability: number;
  elevenlabs_similarity_boost: number;
  elevenlabs_style: number;
  elevenlabs_use_speaker_boost: boolean;
  elevenlabs_apply_text_normalization: ElevenLabsTextNormalization;
  cartesia_model: string;
  cartesia_voice: string;
  cartesia_language: string;
  cartesia_emotion: string | null;
  cartesia_volume: number;
  edge_voice: string;
  edge_voice_language: string;
  local_qwen_voice: string;
  local_qwen_language: string;
  local_kokoro_voice: string;
  local_kokoro_language: string;
  windows_voice_id: string;
  windows_voice_language: string;
  speed: number;
  openai_instructions: string;
  prompt_presets: TtsPromptPreset[];
  selected_prompt_id: string;
  synthesis_presets: TtsSynthesisPreset[];
  synthesis_presets_seed_version: number;
  interactive_synthesis: TtsScopeSynthesisSettings;
  file_synthesis: TtsScopeSynthesisSettings;
  llm_preprocessing: TtsLlmPreprocessingSettings;
  play_pause_hotkey: string;
  play_history_when_overlay_closed: boolean;
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
  overlay_auto_hide_enabled: boolean;
  overlay_auto_hide_delay_seconds: number;
  playback_pitch: number;
  playback_effect: TtsPlaybackEffect;
  output_format: TtsOutputFormat;
  mp3_bitrate_kbps: number;
  watch_folder_enabled: boolean;
  watch_recursive: boolean;
  watch_input_directory: string;
  watch_output_directory: string;
  watch_settle_delay_ms: number;
  disk_reserve_mb: number;
  interactive_history_enabled: boolean;
  interactive_history_max_entries: number;
  interactive_history_max_storage_mb: number;
  file_history_enabled: boolean;
  file_history_max_entries: number;
  file_history_max_storage_mb: number;
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
  llm_cleanup_enabled?: boolean;
  llmCleanupEnabled?: boolean;
  llm_cleanup_chunk_count?: number;
  llmCleanupChunkCount?: number;
  counts_are_estimates?: boolean;
  countsAreEstimates?: boolean;
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

type LocalTtsStatus = {
  kind: LocalTtsKind;
  installed: boolean;
  installing: boolean;
  phase: string;
  downloaded_bytes: number;
  total_bytes: number;
  percentage: number;
  runtime_profile: string;
  model_repository: string;
  model_revision: string;
  model_download_bytes: number;
  install_root: string;
  installed_size_bytes: number;
  estimated_install_bytes: number;
  model_author: string;
  model_source_url: string;
  model_license_name: string;
  model_license_url: string;
  model_license_path: string;
  model_license_declaration_path: string;
  model_license_available: boolean;
  error?: string | null;
};

type WindowsVoiceCatalog = {
  available: boolean;
  voices: Array<{
    id: string;
    display_name: string;
    language: string;
    description: string;
    gender: "female" | "male" | "unknown";
    is_default: boolean;
  }>;
  default_voice_id?: string | null;
  unavailable_reason?: string | null;
};

type TtsVoiceCatalog = {
  provider: TtsProvider;
  voices: Array<{
    id: string;
    label: string;
    group: string;
    language: string;
    gender: string;
    description: string;
    locales: Array<{
      locale: string;
      label: string;
      styles: string[];
    }>;
  }>;
  source: "live" | "builtin";
  supports_live_refresh: boolean;
  replace_builtin: boolean;
  warning?: string | null;
};

const DEFAULT_TTS_SETTINGS: TtsSettings = {
  enabled: true,
  provider: "soniox",
  soniox_key_source: "shared",
  deepgram_key_source: "shared",
  openai_key_source: "shared",
  openai_compatible_key_source: "separate",
  murf_key_source: "separate",
  elevenlabs_key_source: "separate",
  cartesia_key_source: "separate",
  soniox_model: TTS_PROVIDER_DEFAULTS.soniox.model,
  soniox_language: TTS_PROVIDER_DEFAULTS.soniox.language,
  soniox_voice: TTS_PROVIDER_DEFAULTS.soniox.voice,
  deepgram_model: TTS_PROVIDER_DEFAULTS.deepgram.model,
  openai_model: TTS_PROVIDER_DEFAULTS.openai.model,
  openai_voice: TTS_PROVIDER_DEFAULTS.openai.voice,
  openai_compatible_base_url: "https://api.openai.com/v1",
  openai_compatible_allow_insecure_http: false,
  openai_compatible_model: TTS_PROVIDER_DEFAULTS.openai_compatible.model,
  openai_compatible_voice: TTS_PROVIDER_DEFAULTS.openai_compatible.voice,
  murf_model: TTS_PROVIDER_DEFAULTS.murf.model,
  murf_voice: TTS_PROVIDER_DEFAULTS.murf.voice,
  murf_language: TTS_PROVIDER_DEFAULTS.murf.language,
  murf_rate: 0,
  murf_pitch: 0,
  murf_variation: 1,
  murf_style: null,
  elevenlabs_model: TTS_PROVIDER_DEFAULTS.elevenlabs.model,
  elevenlabs_voice: TTS_PROVIDER_DEFAULTS.elevenlabs.voice,
  elevenlabs_language: TTS_PROVIDER_DEFAULTS.elevenlabs.language,
  elevenlabs_stability: 0.5,
  elevenlabs_similarity_boost: 0.75,
  elevenlabs_style: 0,
  elevenlabs_use_speaker_boost: true,
  elevenlabs_apply_text_normalization: "auto",
  cartesia_model: TTS_PROVIDER_DEFAULTS.cartesia.model,
  cartesia_voice: TTS_PROVIDER_DEFAULTS.cartesia.voice,
  cartesia_language: TTS_PROVIDER_DEFAULTS.cartesia.language,
  cartesia_emotion: null,
  cartesia_volume: 1,
  edge_voice: TTS_PROVIDER_DEFAULTS.edge.voice,
  edge_voice_language: TTS_PROVIDER_DEFAULTS.edge.language,
  local_qwen_voice: TTS_PROVIDER_DEFAULTS.local_qwen.voice,
  local_qwen_language: TTS_PROVIDER_DEFAULTS.local_qwen.language,
  local_kokoro_voice: TTS_PROVIDER_DEFAULTS.local_kokoro.voice,
  local_kokoro_language: TTS_PROVIDER_DEFAULTS.local_kokoro.language,
  windows_voice_id: "",
  windows_voice_language: "",
  speed: 1,
  openai_instructions: "",
  prompt_presets: [],
  selected_prompt_id: "",
  synthesis_presets: [],
  synthesis_presets_seed_version: 0,
  interactive_synthesis: {
    active_model_key: "",
    selected_preset_id: "",
    models: [],
    active_models_by_provider: [],
    initialized_providers: [],
  },
  file_synthesis: {
    active_model_key: "",
    selected_preset_id: "",
    models: [],
    active_models_by_provider: [],
    initialized_providers: [],
  },
  llm_preprocessing: DEFAULT_TTS_LLM_PREPROCESSING,
  play_pause_hotkey: "space",
  play_history_when_overlay_closed: false,
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
  overlay_auto_hide_enabled: true,
  overlay_auto_hide_delay_seconds: 2,
  playback_pitch: 1,
  playback_effect: "none",
  output_format: "mp3",
  mp3_bitrate_kbps: 256,
  watch_folder_enabled: false,
  watch_recursive: false,
  watch_input_directory: "",
  watch_output_directory: "",
  watch_settle_delay_ms: 1500,
  disk_reserve_mb: 512,
  interactive_history_enabled: false,
  interactive_history_max_entries: 100,
  interactive_history_max_storage_mb: 1024,
  file_history_enabled: false,
  file_history_max_entries: 100,
  file_history_max_storage_mb: 1024,
};

const providerInputLimit = (provider: TtsProvider, model: string): number => ({
  soniox: 5000,
  deepgram: 2000,
  openai: 4096,
  openai_compatible: 4096,
  murf: 3000,
  elevenlabs:
    model === "eleven_flash_v2_5"
      ? 40000
      : model === "eleven_multilingual_v2"
        ? 10000
        : 5000,
  cartesia: 4000,
  edge: 4096,
  local_qwen: 4096,
  local_kokoro: 4096,
  windows: 4096,
})[provider];
const providerPublishesInputLimit = (provider: TtsProvider): boolean =>
  ["soniox", "deepgram", "openai", "murf", "elevenlabs"].includes(provider);
const PROVIDER_CAPABILITIES: Record<
  TtsProvider,
  {
    requiresApiKey: boolean;
    supportsSharedKey: boolean;
    localOrSystem: boolean;
    downloadableRuntime: boolean;
    supportsInstructions: boolean;
    speed: [number, number];
  }
> = {
  soniox: {
    requiresApiKey: true,
    supportsSharedKey: true,
    localOrSystem: false,
    downloadableRuntime: false,
    supportsInstructions: false,
    speed: TTS_PROVIDER_SPEED_RANGES.soniox,
  },
  deepgram: {
    requiresApiKey: true,
    supportsSharedKey: true,
    localOrSystem: false,
    downloadableRuntime: false,
    supportsInstructions: false,
    speed: TTS_PROVIDER_SPEED_RANGES.deepgram,
  },
  openai: {
    requiresApiKey: true,
    supportsSharedKey: true,
    localOrSystem: false,
    downloadableRuntime: false,
    supportsInstructions: true,
    speed: TTS_PROVIDER_SPEED_RANGES.openai,
  },
  openai_compatible: {
    requiresApiKey: true,
    supportsSharedKey: true,
    localOrSystem: false,
    downloadableRuntime: false,
    supportsInstructions: false,
    speed: TTS_PROVIDER_SPEED_RANGES.openai_compatible,
  },
  murf: {
    requiresApiKey: true,
    supportsSharedKey: false,
    localOrSystem: false,
    downloadableRuntime: false,
    supportsInstructions: false,
    speed: TTS_PROVIDER_SPEED_RANGES.murf,
  },
  elevenlabs: {
    requiresApiKey: true,
    supportsSharedKey: false,
    localOrSystem: false,
    downloadableRuntime: false,
    supportsInstructions: false,
    speed: TTS_PROVIDER_SPEED_RANGES.elevenlabs,
  },
  cartesia: {
    requiresApiKey: true,
    supportsSharedKey: false,
    localOrSystem: false,
    downloadableRuntime: false,
    supportsInstructions: false,
    speed: TTS_PROVIDER_SPEED_RANGES.cartesia,
  },
  edge: {
    requiresApiKey: false,
    supportsSharedKey: false,
    localOrSystem: false,
    downloadableRuntime: false,
    supportsInstructions: false,
    speed: TTS_PROVIDER_SPEED_RANGES.edge,
  },
  local_qwen: {
    requiresApiKey: false,
    supportsSharedKey: false,
    localOrSystem: true,
    downloadableRuntime: true,
    supportsInstructions: false,
    speed: TTS_PROVIDER_SPEED_RANGES.local_qwen,
  },
  local_kokoro: {
    requiresApiKey: false,
    supportsSharedKey: false,
    localOrSystem: true,
    downloadableRuntime: true,
    supportsInstructions: false,
    speed: TTS_PROVIDER_SPEED_RANGES.local_kokoro,
  },
  windows: {
    requiresApiKey: false,
    supportsSharedKey: false,
    localOrSystem: true,
    downloadableRuntime: false,
    supportsInstructions: false,
    speed: TTS_PROVIDER_SPEED_RANGES.windows,
  },
};
const SONIOX_TTS_FIELD_MAX_LENGTH = 50;
const SONIOX_TTS_API_KEY_MAX_LENGTH = 250;
const OPENAI_TTS_INSTRUCTIONS_MAX_LENGTH = 4096;

const SONIOX_VOICES: SelectOption[] = [
  "Maya",
  "Daniel",
  "Noah",
  "Nina",
  "Emma",
  "Jack",
  "Adrian",
  "Claire",
  "Grace",
  "Owen",
  "Mina",
  "Kenji",
  "Rafael",
  "Mateo",
  "Lucia",
  "Sofia",
  "Oliver",
  "Arthur",
  "Isla",
  "Victoria",
  "Cooper",
  "Mason",
  "Ruby",
  "Elise",
  "Arjun",
  "Rohan",
  "Priya",
  "Meera",
].map((value) => ({ value, label: value, group: "Built-in voices" }));

const DEEPGRAM_AURA_2_VOICES: SelectOption[] = [
  "aura-2-amalthea-en",
  "aura-2-andromeda-en",
  "aura-2-apollo-en",
  "aura-2-arcas-en",
  "aura-2-aries-en",
  "aura-2-asteria-en",
  "aura-2-athena-en",
  "aura-2-atlas-en",
  "aura-2-aurora-en",
  "aura-2-callista-en",
  "aura-2-cora-en",
  "aura-2-cordelia-en",
  "aura-2-delia-en",
  "aura-2-draco-en",
  "aura-2-electra-en",
  "aura-2-harmonia-en",
  "aura-2-helena-en",
  "aura-2-hera-en",
  "aura-2-hermes-en",
  "aura-2-hyperion-en",
  "aura-2-iris-en",
  "aura-2-janus-en",
  "aura-2-juno-en",
  "aura-2-jupiter-en",
  "aura-2-luna-en",
  "aura-2-mars-en",
  "aura-2-minerva-en",
  "aura-2-neptune-en",
  "aura-2-odysseus-en",
  "aura-2-ophelia-en",
  "aura-2-orion-en",
  "aura-2-orpheus-en",
  "aura-2-pandora-en",
  "aura-2-phoebe-en",
  "aura-2-pluto-en",
  "aura-2-saturn-en",
  "aura-2-selene-en",
  "aura-2-thalia-en",
  "aura-2-theia-en",
  "aura-2-vesta-en",
  "aura-2-zeus-en",
  "aura-2-agustina-es",
  "aura-2-alvaro-es",
  "aura-2-antonia-es",
  "aura-2-aquila-es",
  "aura-2-carina-es",
  "aura-2-celeste-es",
  "aura-2-diana-es",
  "aura-2-estrella-es",
  "aura-2-gloria-es",
  "aura-2-javier-es",
  "aura-2-luciano-es",
  "aura-2-nestor-es",
  "aura-2-olivia-es",
  "aura-2-selena-es",
  "aura-2-silvia-es",
  "aura-2-sirio-es",
  "aura-2-valerio-es",
  "aura-2-beatrix-nl",
  "aura-2-cornelia-nl",
  "aura-2-daphne-nl",
  "aura-2-hestia-nl",
  "aura-2-lars-nl",
  "aura-2-leda-nl",
  "aura-2-rhea-nl",
  "aura-2-roman-nl",
  "aura-2-sander-nl",
  "aura-2-agathe-fr",
  "aura-2-hector-fr",
  "aura-2-aurelia-de",
  "aura-2-elara-de",
  "aura-2-fabian-de",
  "aura-2-julius-de",
  "aura-2-kara-de",
  "aura-2-lara-de",
  "aura-2-viktoria-de",
  "aura-2-cesare-it",
  "aura-2-cinzia-it",
  "aura-2-demetra-it",
  "aura-2-dionisio-it",
  "aura-2-elio-it",
  "aura-2-flavio-it",
  "aura-2-livia-it",
  "aura-2-maia-it",
  "aura-2-melia-it",
  "aura-2-perseo-it",
  "aura-2-ama-ja",
  "aura-2-ebisu-ja",
  "aura-2-fujin-ja",
  "aura-2-izanami-ja",
  "aura-2-uzume-ja",
].map((value) => ({
  value,
  label: value,
  group: value.endsWith("-es")
    ? "Spanish"
    : value.endsWith("-nl")
      ? "Dutch"
      : value.endsWith("-fr")
        ? "French"
        : value.endsWith("-de")
          ? "German"
          : value.endsWith("-it")
            ? "Italian"
            : value.endsWith("-ja")
              ? "Japanese"
              : "English",
}));

const OPENAI_VOICES: SelectOption[] = [
  "alloy",
  "ash",
  "ballad",
  "cedar",
  "coral",
  "echo",
  "fable",
  "marin",
  "nova",
  "onyx",
  "sage",
  "shimmer",
  "verse",
].map((value) => ({ value, label: value, group: "Built-in voices" }));

const EDGE_FALLBACK_VOICES: SelectOption[] = [
  "en-US-AriaNeural",
  "en-US-GuyNeural",
  "en-GB-SoniaNeural",
  "de-DE-KatjaNeural",
  "fi-FI-NooraNeural",
  "fr-FR-DeniseNeural",
  "ja-JP-NanamiNeural",
  "ru-RU-SvetlanaNeural",
].map((value) => ({
  value,
  label: value,
  group: value.split("-").slice(0, 2).join("-"),
}));

const LOCAL_QWEN_VOICES: SelectOption[] = [
  "Vivian",
  "Serena",
  "Uncle_Fu",
  "Dylan",
  "Eric",
  "Ryan",
  "Aiden",
  "Ono_Anna",
  "Sohee",
].map((value) => ({ value, label: value }));

const LOCAL_QWEN_LANGUAGES: SelectOption[] = [
  "Auto",
  "Chinese",
  "English",
  "Japanese",
  "Korean",
  "German",
  "French",
  "Russian",
  "Portuguese",
  "Spanish",
  "Italian",
].map((value) => ({ value, label: value }));

const LOCAL_KOKORO_ENGLISH_VOICES: SelectOption[] = [
  "af_maple",
  "af_sol",
  "bf_vale",
].map((value) => ({ value, label: value }));

const LOCAL_KOKORO_CHINESE_VOICES: SelectOption[] = [
  "zf_001",
  "zf_002",
  "zf_003",
  "zf_004",
  "zf_005",
  "zf_006",
  "zf_007",
  "zf_008",
  "zf_017",
  "zf_018",
  "zf_019",
  "zf_021",
  "zf_022",
  "zf_023",
  "zf_024",
  "zf_026",
  "zf_027",
  "zf_028",
  "zf_032",
  "zf_036",
  "zf_038",
  "zf_039",
  "zf_040",
  "zf_042",
  "zf_043",
  "zf_044",
  "zf_046",
  "zf_047",
  "zf_048",
  "zf_049",
  "zf_051",
  "zf_059",
  "zf_060",
  "zf_067",
  "zf_070",
  "zf_071",
  "zf_072",
  "zf_073",
  "zf_074",
  "zf_075",
  "zf_076",
  "zf_077",
  "zf_078",
  "zf_079",
  "zf_083",
  "zf_084",
  "zf_085",
  "zf_086",
  "zf_087",
  "zf_088",
  "zf_090",
  "zf_092",
  "zf_093",
  "zf_094",
  "zf_099",
  "zm_009",
  "zm_010",
  "zm_011",
  "zm_012",
  "zm_013",
  "zm_014",
  "zm_015",
  "zm_016",
  "zm_020",
  "zm_025",
  "zm_029",
  "zm_030",
  "zm_031",
  "zm_033",
  "zm_034",
  "zm_035",
  "zm_037",
  "zm_041",
  "zm_045",
  "zm_050",
  "zm_052",
  "zm_053",
  "zm_054",
  "zm_055",
  "zm_056",
  "zm_057",
  "zm_058",
  "zm_061",
  "zm_062",
  "zm_063",
  "zm_064",
  "zm_065",
  "zm_066",
  "zm_068",
  "zm_069",
  "zm_080",
  "zm_081",
  "zm_082",
  "zm_089",
  "zm_091",
  "zm_095",
  "zm_096",
  "zm_097",
  "zm_098",
  "zm_100",
].map((value) => ({ value, label: value }));

const LOCAL_KOKORO_LANGUAGES: SelectOption[] = [
  { value: "English", label: "English" },
  { value: "Chinese", label: "Chinese" },
];

const SONIOX_LANGUAGE_SELECT_OPTIONS: SelectOption[] =
  SONIOX_LANGUAGE_OPTIONS.map(([value, language]) => ({
    value,
    label: `${language} (${value})`,
    group: "Supported languages",
  }));

const SONIOX_MODEL_SELECT_OPTIONS: SelectOption[] = SONIOX_MODEL_OPTIONS.map(
  (value, index) => ({
    value,
    label: value,
    group: index === 0 ? "Active model" : "Compatibility alias",
  }),
);

const OPENAI_MODEL_SELECT_OPTIONS: SelectOption[] = OPENAI_MODEL_OPTIONS.map(
  (value, index) => ({
    value,
    label: value,
    group: index < 2 ? "Current speech models" : "Legacy speech models",
  }),
);

const MURF_MODEL_SELECT_OPTIONS: SelectOption[] = MURF_MODEL_OPTIONS.map(
  (value) => ({
    value,
    label: value === "falcon-2" ? "Falcon 2" : "Gen2",
  }),
);

const ELEVENLABS_MODEL_SELECT_OPTIONS: SelectOption[] =
  ELEVENLABS_MODEL_OPTIONS.map((value) => ({
    value,
    label:
      value === "eleven_flash_v2_5"
        ? "Flash v2.5"
        : value === "eleven_multilingual_v2"
          ? "Multilingual v2"
          : "Eleven v3",
  }));

const CARTESIA_MODEL_SELECT_OPTIONS: SelectOption[] =
  CARTESIA_MODEL_OPTIONS.map((value) => ({
    value,
    label: "Sonic 3.5",
  }));

type CloudVoiceSelectorProps = {
  value: string;
  options: SelectOption[];
  customLabel: string;
  customPlaceholder: string;
  disabled: boolean;
  busy: boolean;
  refreshLabel: string;
  sourceLabel?: string;
  warning?: string | null;
  error?: string | null;
  maxLength?: number;
  onChange: (value: string) => void;
  onRefresh?: () => void;
};

const CloudVoiceSelector: React.FC<CloudVoiceSelectorProps> = ({
  value,
  options,
  customLabel,
  customPlaceholder,
  disabled,
  busy,
  refreshLabel,
  sourceLabel,
  warning,
  error,
  maxLength,
  onChange,
  onRefresh,
}) => {
  const { t } = useTranslation();
  return (
    <div className="w-full space-y-2 md:w-96">
      <Select
        value={value}
        options={options}
        placeholder={customPlaceholder}
        disabled={disabled}
        isLoading={busy}
        isClearable={false}
        isCreatable
        formatCreateLabel={(input) => `${customLabel}: ${input}`}
        onCreateOption={(input) => {
          const next = input.trim();
          if (next) onChange(maxLength ? next.slice(0, maxLength) : next);
        }}
        onChange={(selected) => {
          if (selected) onChange(selected);
        }}
      />
      <div className="flex flex-wrap items-center gap-2">
        {onRefresh && (
          <ActionTooltip
            content={
              busy ? t("textToSpeech.voice.refreshInProgress") : null
            }
          >
            <Button variant="secondary" disabled={busy} onClick={onRefresh}>
              <RefreshCw
                className={`mr-2 inline h-4 w-4 ${busy ? "animate-spin" : ""}`}
              />
              {refreshLabel}
            </Button>
          </ActionTooltip>
        )}
        {sourceLabel && (
          <span className="text-xs text-emerald-200/80">{sourceLabel}</span>
        )}
      </div>
      {warning && <p className="text-xs text-amber-200/90">{warning}</p>}
      {error && <p className="text-xs text-red-300">{error}</p>}
    </div>
  );
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
  "openai_compatible_base_url",
  "openai_compatible_model",
  "openai_compatible_voice",
  "murf_model",
  "murf_voice",
  "murf_language",
  "murf_rate",
  "murf_pitch",
  "murf_variation",
  "murf_style",
  "elevenlabs_model",
  "elevenlabs_voice",
  "elevenlabs_language",
  "elevenlabs_stability",
  "elevenlabs_similarity_boost",
  "elevenlabs_style",
  "cartesia_model",
  "cartesia_voice",
  "cartesia_language",
  "cartesia_emotion",
  "cartesia_volume",
  "edge_voice",
  "edge_voice_language",
  "speed",
  "playback_pitch",
  "openai_instructions",
  "retry_count",
  "retry_base_delay_ms",
  "interactive_target_chars",
  "file_target_chars",
  "inter_chunk_pause_ms",
  "paragraph_pause_ms",
  "watch_settle_delay_ms",
  "disk_reserve_mb",
  "interactive_history_max_entries",
  "interactive_history_max_storage_mb",
  "file_history_max_entries",
  "file_history_max_storage_mb",
]);
const COALESCED_TTS_LLM_FIELDS = new Set([
  "llm_preprocessing.custom_base_url",
  "llm_preprocessing.interactive_prompts",
  "llm_preprocessing.file_prompts",
  "llm_preprocessing.chunk_target_chars",
  "llm_preprocessing.retry_count",
  "llm_preprocessing.retry_base_delay_ms",
  "llm_preprocessing.request_timeout_seconds",
  "llm_preprocessing.interactive_benchmark_text",
  "llm_preprocessing.file_benchmark_text",
]);
const TTS_SYNTHESIS_CUSTOMIZATION_FIELDS = new Set([
  "provider",
  "soniox_key_source",
  "deepgram_key_source",
  "openai_key_source",
  "murf_key_source",
  "elevenlabs_key_source",
  "cartesia_key_source",
  "soniox_model",
  "soniox_language",
  "soniox_voice",
  "deepgram_model",
  "openai_model",
  "openai_voice",
  "murf_model",
  "murf_voice",
  "murf_language",
  "murf_rate",
  "murf_pitch",
  "murf_variation",
  "murf_style",
  "elevenlabs_model",
  "elevenlabs_voice",
  "elevenlabs_language",
  "elevenlabs_stability",
  "elevenlabs_similarity_boost",
  "elevenlabs_style",
  "elevenlabs_use_speaker_boost",
  "elevenlabs_apply_text_normalization",
  "cartesia_model",
  "cartesia_voice",
  "cartesia_language",
  "cartesia_emotion",
  "cartesia_volume",
  "edge_voice",
  "edge_voice_language",
  "local_qwen_voice",
  "local_qwen_language",
  "local_kokoro_voice",
  "local_kokoro_language",
  "windows_voice_id",
  "windows_voice_language",
  "openai_instructions",
  "selected_prompt_id",
  "prompt_presets",
  "speed",
  "preprocessing_enabled",
  "preprocessing_rules",
  "interactive_target_chars",
  "file_target_chars",
  "retry_count",
  "retry_base_delay_ms",
  "inter_chunk_pause_ms",
  "paragraph_pause_ms",
  "output_format",
  "mp3_bitrate_kbps",
]);

const asErrorMessage = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

const formatBytes = (bytes: number) =>
  bytes >= 1024 ** 3
    ? `${(bytes / 1024 ** 3).toFixed(2)} GiB`
    : `${(bytes / 1024 ** 2).toFixed(1)} MiB`;

const defaultTtsOutputPath = (
  inputPath: string,
  outputFormat: TtsOutputFormat,
): string => {
  const normalizedPath = inputPath.trim();
  if (!normalizedPath) return "";

  const separatorIndex = Math.max(
    normalizedPath.lastIndexOf("\\"),
    normalizedPath.lastIndexOf("/"),
  );
  const directory =
    separatorIndex >= 0 ? normalizedPath.slice(0, separatorIndex + 1) : "";
  const fileName = normalizedPath.slice(separatorIndex + 1);
  const extensionIndex = fileName.lastIndexOf(".");
  const baseName =
    extensionIndex > 0 ? fileName.slice(0, extensionIndex) : fileName;

  return `${directory}${baseName}.${outputFormat}`;
};

const operationScope = (
  mode: TextToSpeechSettingsProps["mode"],
): TtsOperationScope => (mode === "files" ? "file" : "interactive");

const scopeSettingsKey = (
  mode: TextToSpeechSettingsProps["mode"],
): "interactive_synthesis" | "file_synthesis" =>
  mode === "files" ? "file_synthesis" : "interactive_synthesis";

const legacySynthesisModelIdentity = (
  provider: TtsProvider,
  model: string,
): string => `${provider}:${model.trim()}`;

const normalizedOpenAiCompatibleEndpointIdentity = (raw?: string): string => {
  const trimmed = (raw ?? "").trim();
  if (!trimmed) return DEFAULT_TTS_SETTINGS.openai_compatible_base_url;

  let candidate = trimmed;
  if (!trimmed.startsWith("http://") && !trimmed.startsWith("https://")) {
    const hostAndPort = trimmed.split("/", 1)[0] ?? "";
    const host = hostAndPort.startsWith("[")
      ? hostAndPort.slice(1, hostAndPort.indexOf("]"))
      : (hostAndPort.split(":", 1)[0] ?? "");
    const normalizedHost = host.toLowerCase();
    const isLoopback =
      normalizedHost === "localhost" ||
      normalizedHost === "::1" ||
      /^127(?:\.\d{1,3}){3}$/.test(normalizedHost);
    candidate = `${isLoopback ? "http" : "https"}://${trimmed}`;
  }

  try {
    return new URL(candidate).toString().replace(/\/+$/, "");
  } catch {
    return trimmed.replace(/\/+$/, "");
  }
};

const synthesisModelIdentity = (config: TtsSynthesisConfig): string => {
  if (config.provider !== "openai_compatible") {
    return legacySynthesisModelIdentity(config.provider, config.model);
  }
  const endpoint = normalizedOpenAiCompatibleEndpointIdentity(
    config.openai_compatible_base_url,
  );
  return `${config.provider}:${Array.from(endpoint).length}:${endpoint}:${config.model.trim()}`;
};

const normalizeScopeModelIdentities = (
  scope: TtsScopeSynthesisSettings,
): TtsScopeSynthesisSettings => {
  const migrations = scope.models.map((entry) => ({
    provider: entry.config.provider,
    oldKey: entry.model_key,
    legacyKey: legacySynthesisModelIdentity(
      entry.config.provider,
      entry.config.model,
    ),
    newKey: synthesisModelIdentity(entry.config),
    config: entry.config,
  }));

  const migrateKey = (key: string, provider?: TtsProvider): string => {
    for (let index = migrations.length - 1; index >= 0; index -= 1) {
      const migration = migrations[index];
      if (
        (provider === undefined || migration.provider === provider) &&
        migration.oldKey === key
      ) {
        return migration.newKey;
      }
    }
    for (let index = migrations.length - 1; index >= 0; index -= 1) {
      const migration = migrations[index];
      if (
        (provider === undefined || migration.provider === provider) &&
        migration.legacyKey === key
      ) {
        return migration.newKey;
      }
    }
    return key;
  };

  const seenModelKeys = new Set<string>();
  const models = migrations.flatMap((migration) => {
    if (seenModelKeys.has(migration.newKey)) return [];
    seenModelKeys.add(migration.newKey);
    return [{ model_key: migration.newKey, config: migration.config }];
  });
  const activeModelKey = migrateKey(scope.active_model_key);
  const active_model_key = models.some(
    (entry) => entry.model_key === activeModelKey,
  )
    ? activeModelKey
    : (models[models.length - 1]?.model_key ?? "");

  const seenProviders = new Set<TtsProvider>();
  const active_models_by_provider = scope.active_models_by_provider.flatMap(
    (selection) => {
      if (seenProviders.has(selection.provider)) return [];
      const modelKey = migrateKey(selection.model_key, selection.provider);
      if (
        !models.some(
          (entry) =>
            entry.model_key === modelKey &&
            entry.config.provider === selection.provider,
        )
      ) {
        return [];
      }
      seenProviders.add(selection.provider);
      return [{ provider: selection.provider, model_key: modelKey }];
    },
  );

  const activeModel = models.find(
    (entry) => entry.model_key === active_model_key,
  );
  if (activeModel) {
    const existingIndex = active_models_by_provider.findIndex(
      (selection) => selection.provider === activeModel.config.provider,
    );
    const selection = {
      provider: activeModel.config.provider,
      model_key: active_model_key,
    };
    if (existingIndex >= 0) {
      active_models_by_provider[existingIndex] = selection;
    } else {
      active_models_by_provider.push(selection);
    }
  }

  return {
    ...scope,
    active_model_key,
    models,
    active_models_by_provider,
  };
};

function assertNever(x: never): never {
  throw new Error(`Unexpected provider: ${x}`);
}

const synthesisConfigFromSettings = (
  settings: TtsSettings,
  mode: TextToSpeechSettingsProps["mode"],
): TtsSynthesisConfig => {
  let model: string;
  let voice: string;
  let language: string;
  switch (settings.provider) {
    case "soniox":
      model = settings.soniox_model;
      voice = settings.soniox_voice;
      language = settings.soniox_language;
      break;
    case "deepgram":
      model = settings.deepgram_model;
      voice = settings.deepgram_model;
      language = "";
      break;
    case "openai":
      model = settings.openai_model;
      voice = settings.openai_voice;
      language = "";
      break;
    case "openai_compatible":
      model = settings.openai_compatible_model;
      voice = settings.openai_compatible_voice;
      language = "";
      break;
    case "murf":
      model = settings.murf_model;
      voice = settings.murf_voice;
      language = settings.murf_language;
      break;
    case "elevenlabs":
      model = settings.elevenlabs_model;
      voice = settings.elevenlabs_voice;
      language = settings.elevenlabs_language;
      break;
    case "cartesia":
      model = settings.cartesia_model;
      voice = settings.cartesia_voice;
      language = settings.cartesia_language;
      break;
    case "edge":
      model = "microsoft-edge-read-aloud";
      voice = settings.edge_voice;
      language = settings.edge_voice_language;
      break;
    case "local_qwen":
      model = "qwen3-tts-12hz-0.6b-customvoice";
      voice = settings.local_qwen_voice;
      language = settings.local_qwen_language;
      break;
    case "local_kokoro":
      model = "kokoro-82m";
      voice = settings.local_kokoro_voice;
      language = settings.local_kokoro_language;
      break;
    case "windows":
      model = "windows.media.speechsynthesis";
      voice = settings.windows_voice_id;
      language = settings.windows_voice_language;
      break;
    default:
      assertNever(settings.provider);
  }

  let keySource: TtsKeySource;
  switch (settings.provider) {
    case "soniox":
      keySource = settings.soniox_key_source;
      break;
    case "deepgram":
      keySource = settings.deepgram_key_source;
      break;
    case "openai":
      keySource = settings.openai_key_source;
      break;
    case "openai_compatible":
      keySource = settings.openai_compatible_key_source;
      break;
    case "murf":
      keySource = settings.murf_key_source;
      break;
    case "elevenlabs":
      keySource = settings.elevenlabs_key_source;
      break;
    case "cartesia":
      keySource = settings.cartesia_key_source;
      break;
    case "edge":
    case "local_qwen":
    case "local_kokoro":
    case "windows":
      keySource = "shared";
      break;
    default:
      assertNever(settings.provider);
  }

  return {
    provider: settings.provider,
    model,
    voice,
    language,
    key_source: keySource,
    openai_compatible_base_url: settings.openai_compatible_base_url,
    openai_compatible_allow_insecure_http:
      settings.openai_compatible_allow_insecure_http,
    speed: settings.speed,
    murf_rate: settings.murf_rate,
    murf_pitch: settings.murf_pitch,
    murf_variation: settings.murf_variation,
    murf_style: settings.murf_style,
    elevenlabs_stability: settings.elevenlabs_stability,
    elevenlabs_similarity_boost: settings.elevenlabs_similarity_boost,
    elevenlabs_style: settings.elevenlabs_style,
    elevenlabs_use_speaker_boost: settings.elevenlabs_use_speaker_boost,
    elevenlabs_apply_text_normalization:
      settings.elevenlabs_apply_text_normalization,
    cartesia_emotion: settings.cartesia_emotion,
    cartesia_volume: settings.cartesia_volume,
    voice_instructions: settings.openai_instructions,
    voice_prompt_preset_id: settings.selected_prompt_id,
    preprocessing_enabled: settings.preprocessing_enabled,
    preprocessing_rules: settings.preprocessing_rules,
    target_chars:
      mode === "files"
        ? settings.file_target_chars
        : settings.interactive_target_chars,
    retry_count: settings.retry_count,
    retry_base_delay_ms: settings.retry_base_delay_ms,
    inter_chunk_pause_ms: settings.inter_chunk_pause_ms,
    paragraph_pause_ms: settings.paragraph_pause_ms,
    output_format: settings.output_format,
    mp3_bitrate_kbps: settings.mp3_bitrate_kbps,
  };
};

const applySynthesisConfig = (
  settings: TtsSettings,
  config: TtsSynthesisConfig,
  mode: TextToSpeechSettingsProps["mode"],
): TtsSettings => {
  const next: TtsSettings = {
    ...settings,
    provider: config.provider,
    speed:
      config.provider === "murf" ||
      (config.provider === "elevenlabs" && config.model === "eleven_v3")
        ? 1
        : config.speed,
    openai_instructions: config.voice_instructions,
    selected_prompt_id: config.voice_prompt_preset_id,
    preprocessing_enabled: config.preprocessing_enabled,
    preprocessing_rules: config.preprocessing_rules,
    retry_count: config.retry_count,
    retry_base_delay_ms: config.retry_base_delay_ms,
    inter_chunk_pause_ms: config.inter_chunk_pause_ms,
    paragraph_pause_ms: config.paragraph_pause_ms,
    output_format: config.output_format,
    mp3_bitrate_kbps: config.mp3_bitrate_kbps,
    ...(mode === "files"
      ? { file_target_chars: config.target_chars }
      : { interactive_target_chars: config.target_chars }),
  };

  switch (config.provider) {
    case "soniox":
      next.soniox_model = config.model;
      next.soniox_voice = config.voice;
      next.soniox_language = config.language;
      next.soniox_key_source = config.key_source ?? "separate";
      break;
    case "deepgram":
      next.deepgram_model = config.model;
      next.deepgram_key_source = config.key_source ?? "separate";
      break;
    case "openai":
      next.openai_model = config.model;
      next.openai_voice = config.voice;
      next.openai_key_source = config.key_source ?? "separate";
      break;
    case "openai_compatible":
      next.openai_compatible_model = config.model;
      next.openai_compatible_voice = config.voice;
      next.openai_compatible_key_source = config.key_source ?? "separate";
      if (config.openai_compatible_base_url !== undefined) {
        next.openai_compatible_base_url = config.openai_compatible_base_url;
      }
      if (config.openai_compatible_allow_insecure_http !== undefined) {
        next.openai_compatible_allow_insecure_http =
          config.openai_compatible_allow_insecure_http;
      }
      break;
    case "murf":
      next.murf_model = config.model;
      next.murf_voice = config.voice;
      next.murf_language = config.language;
      next.murf_key_source = "separate";
      next.murf_rate = config.murf_rate;
      next.murf_pitch = config.murf_pitch;
      next.murf_variation = config.murf_variation;
      next.murf_style = config.murf_style;
      break;
    case "elevenlabs":
      next.elevenlabs_model = config.model;
      next.elevenlabs_voice = config.voice;
      next.elevenlabs_language = config.language;
      next.elevenlabs_key_source = "separate";
      next.elevenlabs_stability = config.elevenlabs_stability;
      next.elevenlabs_similarity_boost = config.elevenlabs_similarity_boost;
      next.elevenlabs_style = config.elevenlabs_style;
      next.elevenlabs_use_speaker_boost = config.elevenlabs_use_speaker_boost;
      next.elevenlabs_apply_text_normalization =
        config.elevenlabs_apply_text_normalization;
      break;
    case "cartesia":
      next.cartesia_model = config.model;
      next.cartesia_voice = config.voice;
      next.cartesia_language = config.language;
      next.cartesia_key_source = "separate";
      next.cartesia_emotion = config.cartesia_emotion;
      next.cartesia_volume = config.cartesia_volume;
      break;
    case "edge":
      next.edge_voice = config.voice;
      next.edge_voice_language = config.language;
      break;
    case "local_qwen":
      next.local_qwen_voice = config.voice;
      next.local_qwen_language = config.language;
      break;
    case "local_kokoro":
      next.local_kokoro_voice = config.voice;
      next.local_kokoro_language = config.language;
      break;
    case "windows":
      next.windows_voice_id = config.voice;
      next.windows_voice_language = config.language;
      break;
    default:
      assertNever(config.provider);
  }
  return next;
};

const synthesisPresetConfigForMode = (
  settings: TtsSettings,
  preset: TtsSynthesisPreset,
  mode: TextToSpeechSettingsProps["mode"],
): TtsSynthesisConfig =>
  mode === "files" && isBuiltinTtsSynthesisPreset(preset)
    ? { ...preset.config, target_chars: settings.file_target_chars }
    : preset.config;

const upsertScopeConfig = (
  scope: TtsScopeSynthesisSettings,
  config: TtsSynthesisConfig,
  selectedPresetId: string,
): TtsScopeSynthesisSettings => {
  const normalizedScope = normalizeScopeModelIdentities(scope);
  const modelKey = synthesisModelIdentity(config);
  const existingIndex = normalizedScope.models.findIndex(
    (entry) => entry.model_key === modelKey,
  );
  const models =
    existingIndex >= 0
      ? normalizedScope.models.map((entry, index) =>
          index === existingIndex ? { model_key: modelKey, config } : entry,
        )
      : [...normalizedScope.models, { model_key: modelKey, config }];
  return {
    ...normalizedScope,
    active_model_key: modelKey,
    selected_preset_id: selectedPresetId,
    models: models.slice(-100),
    active_models_by_provider: [
      ...normalizedScope.active_models_by_provider.filter(
        (selection) => selection.provider !== config.provider,
      ),
      { provider: config.provider, model_key: modelKey },
    ],
    initialized_providers: normalizedScope.initialized_providers.includes(
      config.provider,
    )
      ? normalizedScope.initialized_providers
      : [...normalizedScope.initialized_providers, config.provider],
  };
};

const materializeSynthesisScope = (
  settings: TtsSettings,
  mode: TextToSpeechSettingsProps["mode"],
): TtsSettings => {
  const scopeKey = scopeSettingsKey(mode);
  const scope = normalizeScopeModelIdentities(settings[scopeKey]);
  const active = scope.models.find(
    (entry) => entry.model_key === scope.active_model_key,
  );
  return active
    ? applySynthesisConfig({ ...settings, [scopeKey]: scope }, active.config, mode)
    : { ...settings, [scopeKey]: scope };
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

const llmCleanupEnabled = (inspection: FileInspection | null) =>
  inspection?.llm_cleanup_enabled ?? inspection?.llmCleanupEnabled ?? false;

const llmCleanupChunkCount = (inspection: FileInspection | null) =>
  inspection?.llm_cleanup_chunk_count ??
  inspection?.llmCleanupChunkCount ??
  0;

const inspectionCountsAreEstimates = (inspection: FileInspection | null) =>
  inspection?.counts_are_estimates ?? inspection?.countsAreEstimates ?? false;

export interface TextToSpeechSettingsProps {
  mode?: "interactive" | "files";
}

const FIRST_VISIT_NOTICE_STORAGE_KEYS = {
  interactive: "aivorelay.tts.first-visit.interactive.v1",
  files: "aivorelay.tts.first-visit.files.v1",
} as const;

const shouldShowFirstVisitNotice = (
  mode: NonNullable<TextToSpeechSettingsProps["mode"]>,
): boolean => {
  if (typeof window === "undefined") return true;
  try {
    return (
      window.localStorage.getItem(FIRST_VISIT_NOTICE_STORAGE_KEYS[mode]) !==
      "dismissed"
    );
  } catch {
    return true;
  }
};

export const TextToSpeechSettings: React.FC<TextToSpeechSettingsProps> = ({
  mode = "interactive",
}) => {
  const { t } = useTranslation();
  const { settings, refreshSettings } = useSettings();
  const storedTts = (settings as any)?.tts as Partial<TtsSettings> | undefined;
  const storedTtsSnapshot = useMemo<TtsSettings>(() => {
    const merged: TtsSettings = {
      ...DEFAULT_TTS_SETTINGS,
      ...storedTts,
      llm_preprocessing: {
        ...DEFAULT_TTS_LLM_PREPROCESSING,
        ...storedTts?.llm_preprocessing,
        interactive_prompts:
          storedTts?.llm_preprocessing?.interactive_prompts ??
          DEFAULT_TTS_LLM_PREPROCESSING.interactive_prompts,
        file_prompts:
          storedTts?.llm_preprocessing?.file_prompts ??
          DEFAULT_TTS_LLM_PREPROCESSING.file_prompts,
        interactive_benchmark_log:
          storedTts?.llm_preprocessing?.interactive_benchmark_log ?? [],
        file_benchmark_log:
          storedTts?.llm_preprocessing?.file_benchmark_log ?? [],
      },
      preprocessing_rules: storedTts?.preprocessing_rules ?? [],
      prompt_presets: storedTts?.prompt_presets ?? [],
      synthesis_presets: storedTts?.synthesis_presets ?? [],
      interactive_synthesis: {
        ...DEFAULT_TTS_SETTINGS.interactive_synthesis,
        ...storedTts?.interactive_synthesis,
        models: storedTts?.interactive_synthesis?.models ?? [],
        active_models_by_provider:
          storedTts?.interactive_synthesis?.active_models_by_provider ?? [],
        initialized_providers:
          storedTts?.interactive_synthesis?.initialized_providers ?? [],
      },
      file_synthesis: {
        ...DEFAULT_TTS_SETTINGS.file_synthesis,
        ...storedTts?.file_synthesis,
        models: storedTts?.file_synthesis?.models ?? [],
        active_models_by_provider:
          storedTts?.file_synthesis?.active_models_by_provider ?? [],
        initialized_providers:
          storedTts?.file_synthesis?.initialized_providers ?? [],
      },
    };
    return materializeSynthesisScope(merged, mode);
  }, [mode, storedTts]);
  const [tts, setTts] = useState(storedTtsSnapshot);
  const ttsRef = useRef(storedTtsSnapshot);
  const settingsWriteQueueRef = useRef<Promise<void>>(Promise.resolve());
  const pendingSettingsWritesRef = useRef(0);
  const settingsWriteGenerationRef = useRef(0);
  const keyStatusGenerationRef = useRef(0);
  const voiceCatalogGenerationRef = useRef(0);

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
  const [deleteConfirmation, setDeleteConfirmation] =
    useState<TtsDeleteConfirmation | null>(null);
  const [showFirstVisitNotice, setShowFirstVisitNotice] = useState(() =>
    shouldShowFirstVisitNotice(mode),
  );

  useEffect(() => {
    setShowFirstVisitNotice(shouldShowFirstVisitNotice(mode));
  }, [mode]);

  const [hasSeparateKey, setHasSeparateKey] = useState(false);
  const [hasEffectiveKey, setHasEffectiveKey] = useState(false);
  const [keyStatusLoaded, setKeyStatusLoaded] = useState(false);
  const [keyDraft, setKeyDraft] = useState("");
  const [editingKey, setEditingKey] = useState(false);
  const [keyBusy, setKeyBusy] = useState(false);
  const [localTtsStatus, setLocalTtsStatus] = useState<LocalTtsStatus | null>(
    null,
  );
  const [localTtsBusy, setLocalTtsBusy] = useState(false);
  const [localInstallConsent, setLocalInstallConsent] = useState<
    Record<LocalTtsKind, { sourceTrusted: boolean; riskAcknowledged: boolean }>
  >({
    qwen: { sourceTrusted: false, riskAcknowledged: false },
    kokoro: { sourceTrusted: false, riskAcknowledged: false },
  });
  const [windowsCatalog, setWindowsCatalog] =
    useState<WindowsVoiceCatalog | null>(null);
  const [windowsCatalogBusy, setWindowsCatalogBusy] = useState(false);
  const [voiceCatalog, setVoiceCatalog] = useState<TtsVoiceCatalog | null>(
    null,
  );
  const [voiceCatalogBusy, setVoiceCatalogBusy] = useState(false);
  const [voiceCatalogError, setVoiceCatalogError] = useState<string | null>(
    null,
  );
  const [voicePreviewText, setVoicePreviewText] = useState(() =>
    t("textToSpeech.voice.previewDefaultText"),
  );
  const [voicePreviewBusy, setVoicePreviewBusy] = useState(false);
  const [voicePreviewStopping, setVoicePreviewStopping] = useState(false);
  const [voicePreviewError, setVoicePreviewError] = useState<string | null>(
    null,
  );
  const voicePreviewCancelRequestedRef = useRef(false);
  const [capturingHotkey, setCapturingHotkey] = useState<
    "play_pause" | "stop" | null
  >(null);
  const [interactiveHistoryHasEntries, setInteractiveHistoryHasEntries] =
    useState(false);
  const [ruleFrom, setRuleFrom] = useState("");
  const [ruleTo, setRuleTo] = useState("");
  const [ruleCaseSensitive, setRuleCaseSensitive] = useState(true);
  const [ruleIsRegex, setRuleIsRegex] = useState(false);
  const [promptPresetName, setPromptPresetName] = useState("");
  const [synthesisPresetName, setSynthesisPresetName] = useState(() => {
    const scope = storedTtsSnapshot[scopeSettingsKey(mode)];
    return (
      storedTtsSnapshot.synthesis_presets.find(
        (preset) => preset.id === scope.selected_preset_id,
      )?.name ?? ""
    );
  });
  const synthesisPresetModeRef = useRef(mode);

  useEffect(() => {
    if (synthesisPresetModeRef.current === mode) return;
    synthesisPresetModeRef.current = mode;
    const scope = storedTtsSnapshot[scopeSettingsKey(mode)];
    setSynthesisPresetName(
      storedTtsSnapshot.synthesis_presets.find(
        (preset) => preset.id === scope.selected_preset_id,
      )?.name ?? "",
    );
  }, [mode, storedTtsSnapshot]);

  const [inputPath, setInputPath] = useState("");
  const [outputPath, setOutputPath] = useState("");
  const [inspection, setInspection] = useState<FileInspection | null>(null);
  const [inspecting, setInspecting] = useState(false);
  const [outputFormat, setOutputFormat] = useState<TtsOutputFormat>("mp3");
  const [mp3Bitrate, setMp3Bitrate] = useState(256);
  const [conversionBusy, setConversionBusy] = useState(false);
  const [conversionStopping, setConversionStopping] = useState(false);
  const [conversionProgress, setConversionProgress] =
    useState<ConversionProgress | null>(null);
  const [conversionError, setConversionError] = useState<string | null>(null);
  const [completedPath, setCompletedPath] = useState("");
  const [operationId, setOperationId] = useState<string | null>(null);
  const conversionBusyRef = useRef(false);
  const conversionOperationIdRef = useRef<string | null>(null);
  const inspectionGenerationRef = useRef(0);
  const outputPathCustomizedRef = useRef(false);
  const llmProviders = useMemo(
    () =>
      ((settings as any)?.post_process_providers as
        | Array<{ id: string; label: string }>
        | undefined) ?? [
        { id: "openai", label: "OpenAI" },
        { id: "openrouter", label: "OpenRouter" },
        { id: "anthropic", label: "Anthropic" },
        { id: "groq", label: "Groq" },
        { id: "custom", label: "Custom" },
      ],
    [settings],
  );
  const providerCapabilities = PROVIDER_CAPABILITIES[tts.provider];
  const activeLocalKind: LocalTtsKind =
    tts.provider === "local_kokoro" ? "kokoro" : "qwen";
  const activeLocalKindRef = useRef(activeLocalKind);
  const localTtsStatusGenerationRef = useRef(0);
  activeLocalKindRef.current = activeLocalKind;
  const invalidateInspection = useCallback(() => {
    inspectionGenerationRef.current += 1;
    setInspecting(false);
    setInspection(null);
  }, []);
  const activeLocalInstallConsent = localInstallConsent[activeLocalKind];
  const localInstallMetadata = LOCAL_TTS_INSTALL_METADATA[activeLocalKind];
  const keySourceField:
    | "soniox_key_source"
    | "deepgram_key_source"
    | "openai_key_source"
    | "openai_compatible_key_source"
    | "murf_key_source"
    | "elevenlabs_key_source"
    | "cartesia_key_source"
    | null =
    tts.provider === "soniox"
      ? "soniox_key_source"
      : tts.provider === "deepgram"
        ? "deepgram_key_source"
        : tts.provider === "openai"
          ? "openai_key_source"
          : tts.provider === "openai_compatible"
            ? "openai_compatible_key_source"
            : tts.provider === "murf"
              ? "murf_key_source"
              : tts.provider === "elevenlabs"
                ? "elevenlabs_key_source"
                : tts.provider === "cartesia"
                  ? "cartesia_key_source"
                  : null;
  const keySource = keySourceField ? tts[keySourceField] : "shared";
  const voiceValue =
    tts.provider === "soniox"
      ? tts.soniox_voice
      : tts.provider === "deepgram"
        ? tts.deepgram_model
        : tts.provider === "openai"
          ? tts.openai_voice
          : tts.provider === "openai_compatible"
            ? tts.openai_compatible_voice
            : tts.provider === "murf"
              ? tts.murf_voice
              : tts.provider === "elevenlabs"
                ? tts.elevenlabs_voice
                : tts.provider === "cartesia"
                  ? tts.cartesia_voice
                  : tts.provider === "edge"
                    ? tts.edge_voice
                    : tts.provider === "windows"
                      ? tts.windows_voice_id
                      : tts.provider === "local_kokoro"
                        ? tts.local_kokoro_voice
                        : tts.local_qwen_voice;
  const modelValue =
    tts.provider === "soniox"
      ? tts.soniox_model
      : tts.provider === "deepgram"
        ? tts.deepgram_model
        : tts.provider === "openai"
          ? tts.openai_model
          : tts.provider === "openai_compatible"
            ? tts.openai_compatible_model
            : tts.provider === "murf"
              ? tts.murf_model
              : tts.provider === "elevenlabs"
                ? tts.elevenlabs_model
              : tts.provider === "cartesia"
                ? tts.cartesia_model
                : tts.provider === "edge"
                  ? "microsoft-edge-read-aloud"
                  : tts.provider === "windows"
                    ? "windows.media.speechsynthesis"
                    : tts.provider === "local_kokoro"
                      ? "kokoro-int8-multi-lang-v1_1"
                      : "Qwen3-TTS-12Hz-0.6B-CustomVoice";
  const [speedMinimum, speedMaximum] = providerCapabilities.speed;
  const elevenV3Selected =
    tts.provider === "elevenlabs" && tts.elevenlabs_model === "eleven_v3";
  const openAiInstructionsSupported =
    !tts.openai_model.trim() ||
    tts.openai_model.trim().startsWith("gpt-4o-mini-tts");
  const voiceResources = TTS_PROVIDER_DOCUMENTATION[tts.provider];
  const providerSelectOptions = useMemo<SelectOption[]>(
    () =>
      TTS_PROVIDER_OPTIONS.map((option) => ({
        ...option,
        group:
          option.group === "Cloud"
            ? t("textToSpeech.providerGroups.cloud")
            : option.group === "Online without an API key"
              ? t("textToSpeech.providerGroups.onlineNoKey")
              : option.group === "On device"
                ? t("textToSpeech.providerGroups.onDevice")
                : t("textToSpeech.providerGroups.system"),
      })),
    [t],
  );
  const providerLabel =
    providerSelectOptions.find((option) => option.value === tts.provider)
      ?.label ?? tts.provider;
  const providerHelpLinks = [
    {
      label: t("textToSpeech.help.providerOverview"),
      href: voiceResources.overview,
    },
    {
      label: t("textToSpeech.help.providerParameters"),
      href: voiceResources.parameters,
    },
    {
      label: t("textToSpeech.help.providerVoices"),
      href: voiceResources.voices,
    },
    ...(voiceResources.playground
      ? [
          {
            label: t("textToSpeech.help.providerPlayground"),
            href: voiceResources.playground,
          },
        ]
      : []),
  ];
  const aivoRelayTtsGuideLink = [
    {
      label: t("textToSpeech.help.aivoRelayGuide"),
      href: AIVORELAY_TTS_GUIDE_URL,
    },
  ];
  const builtinVoiceOptions = useMemo<SelectOption[]>(() => {
    switch (tts.provider) {
      case "soniox":
        return SONIOX_VOICES;
      case "deepgram":
        return DEEPGRAM_AURA_2_VOICES;
      case "openai":
        return OPENAI_VOICES;
      case "murf": {
        const voice =
          tts.murf_model === "gen2"
            ? "en-US-natalie"
            : TTS_PROVIDER_DEFAULTS.murf.voice;
        return [
          {
            value: voice,
            label: voice,
            group: "Recommended voice",
          },
        ];
      }
      case "elevenlabs":
      case "cartesia":
        return [
          {
            value: TTS_PROVIDER_DEFAULTS[tts.provider].voice,
            label: TTS_PROVIDER_DEFAULTS[tts.provider].voice,
            group: "Recommended voice",
          },
        ];
      case "edge":
        return EDGE_FALLBACK_VOICES;
      default:
        return [];
    }
  }, [tts.murf_model, tts.provider]);
  const cloudVoiceOptions = useMemo<SelectOption[]>(() => {
    if (!voiceCatalog || voiceCatalog.provider !== tts.provider) {
      return builtinVoiceOptions;
    }
    const liveOptions = voiceCatalog.voices.map((voice) => ({
      value: voice.id,
      label: voice.label || voice.id,
      group: voice.group || voice.language || "Other",
    }));
    const candidates = voiceCatalog.replace_builtin
      ? liveOptions
      : [...builtinVoiceOptions, ...liveOptions];
    const seen = new Set<string>();
    return candidates
      .filter((option) => {
        if (seen.has(option.value)) return false;
        seen.add(option.value);
        return true;
      })
      .map((option) => ({
        ...option,
        group:
          option.group === "Built-in voices"
            ? t("textToSpeech.voice.builtinGroup")
            : option.group === "Custom voices"
              ? t("textToSpeech.voice.customGroup")
              : option.group === "Other"
                ? t("textToSpeech.voice.otherGroup")
                : option.group,
      }));
  }, [builtinVoiceOptions, t, tts.provider, voiceCatalog]);
  const providerLanguageValue =
    tts.provider === "murf"
      ? tts.murf_language
      : tts.provider === "elevenlabs"
        ? tts.elevenlabs_language
        : tts.provider === "cartesia"
          ? tts.cartesia_language
          : "";
  const selectedCatalogVoice =
    voiceCatalog?.provider === tts.provider
      ? voiceCatalog.voices.find((voice) => voice.id === voiceValue)
      : undefined;
  const providerLanguageOptions = useMemo<SelectOption[]>(() => {
    if (
      tts.provider !== "murf" &&
      tts.provider !== "elevenlabs" &&
      tts.provider !== "cartesia"
    ) {
      return [];
    }
    const languageCode = (value: string) => {
      const candidate = value.split(/[-_]/, 1)[0] ?? "";
      return /^[a-z]{2}$/i.test(candidate) ? candidate.toLowerCase() : "";
    };
    const catalogLocales = selectedCatalogVoice?.locales ?? [];
    const catalogValues =
      tts.provider === "murf"
        ? catalogLocales.map((locale) => locale.locale)
        : [
            selectedCatalogVoice?.language ?? "",
            ...catalogLocales.map((locale) => languageCode(locale.locale)),
          ];
    const values = [
      providerLanguageValue,
      TTS_PROVIDER_DEFAULTS[tts.provider].language,
      ...catalogValues,
    ].filter(Boolean);
    return [...new Set(values)].map((value) => {
      const locale = catalogLocales.find((candidate) =>
        tts.provider === "murf"
          ? candidate.locale === value
          : languageCode(candidate.locale) === value,
      );
      return {
        value,
        label: locale?.label
          ? [locale.label, " (", value, ")"].join("")
          : value,
      };
    });
  }, [providerLanguageValue, selectedCatalogVoice, tts.provider]);
  const murfStyleOptions = useMemo<SelectOption[]>(() => {
    const locale = (selectedCatalogVoice?.locales ?? []).find(
      (candidate) =>
        candidate.locale.toLowerCase() === tts.murf_language.toLowerCase(),
    );
    return [
      { value: "", label: t("textToSpeech.voice.unsetStyle") },
      ...(locale?.styles ?? []).map((style) => ({
        value: style,
        label: style,
      })),
    ];
  }, [selectedCatalogVoice, t, tts.murf_language]);
  const retainedMurfStyle = (voiceId: string, localeId: string) => {
    if (!tts.murf_style) return null;
    const voice =
      voiceCatalog?.provider === "murf"
        ? voiceCatalog.voices.find((candidate) => candidate.id === voiceId)
        : undefined;
    const locale = (voice?.locales ?? []).find(
      (candidate) =>
        candidate.locale.toLowerCase() === localeId.toLowerCase(),
    );
    return locale?.styles.some(
      (style) => style.toLowerCase() === tts.murf_style?.toLowerCase(),
    )
      ? tts.murf_style
      : null;
  };

  const refreshLocalTtsStatus = useCallback(async () => {
    const requestedKind = activeLocalKind;
    const requestGeneration = ++localTtsStatusGenerationRef.current;
    try {
      const status = await invoke<LocalTtsStatus>("get_local_tts_status", {
        kind: requestedKind,
      });
      if (
        requestGeneration === localTtsStatusGenerationRef.current &&
        activeLocalKindRef.current === requestedKind &&
        status.kind === requestedKind
      ) {
        setLocalTtsStatus(status);
      }
    } catch (error) {
      if (
        requestGeneration === localTtsStatusGenerationRef.current &&
        activeLocalKindRef.current === requestedKind
      ) {
        setSettingsError(asErrorMessage(error));
      }
    }
  }, [activeLocalKind]);

  const refreshWindowsCatalog = useCallback(async () => {
    setWindowsCatalogBusy(true);
    try {
      setWindowsCatalog(
        await invoke<WindowsVoiceCatalog>("get_windows_tts_voice_catalog"),
      );
    } catch (error) {
      const message = asErrorMessage(error);
      setWindowsCatalog({
        available: false,
        voices: [],
        default_voice_id: null,
        unavailable_reason: message,
      });
      setSettingsError(message);
    } finally {
      setWindowsCatalogBusy(false);
    }
  }, []);

  const refreshVoiceCatalog = useCallback(async () => {
    const provider = ttsRef.current.provider;
    const murfModel = provider === "murf" ? ttsRef.current.murf_model : null;
    if (
      !(
        [
          "soniox",
          "deepgram",
          "openai",
          "murf",
          "elevenlabs",
          "cartesia",
          "edge",
        ] as TtsProvider[]
      ).includes(provider)
    ) {
      return false;
    }
    const requestGeneration = ++voiceCatalogGenerationRef.current;
    const requestIsCurrent = () =>
      requestGeneration === voiceCatalogGenerationRef.current &&
      ttsRef.current.provider === provider &&
      (provider !== "murf" || ttsRef.current.murf_model === murfModel);
    setVoiceCatalogBusy(true);
    setVoiceCatalogError(null);
    try {
      const catalog = await invoke<TtsVoiceCatalog>("get_tts_voice_catalog", {
        provider,
        scope: operationScope(mode),
        model: murfModel,
      });
      if (requestIsCurrent()) setVoiceCatalog(catalog);
      return true;
    } catch (error) {
      if (requestIsCurrent()) {
        setVoiceCatalogError(asErrorMessage(error));
      }
      return false;
    } finally {
      if (requestIsCurrent()) setVoiceCatalogBusy(false);
    }
  }, [mode]);

  useEffect(() => {
    if (tts.provider === "windows" && windowsCatalog === null) {
      void refreshWindowsCatalog();
    }
  }, [refreshWindowsCatalog, tts.provider, windowsCatalog]);

  useEffect(() => {
    voiceCatalogGenerationRef.current += 1;
    setVoiceCatalog(null);
    setVoiceCatalogError(null);
    setVoiceCatalogBusy(false);
    if (
      tts.provider === "edge" ||
      tts.provider === "openai" ||
      ([
        "soniox",
        "deepgram",
        "murf",
        "elevenlabs",
        "cartesia",
      ].includes(tts.provider) &&
        keyStatusLoaded &&
        hasEffectiveKey)
    ) {
      void refreshVoiceCatalog();
    }
  }, [
    hasEffectiveKey,
    keyStatusLoaded,
    refreshVoiceCatalog,
    tts.murf_model,
    tts.provider,
  ]);

  useEffect(() => {
    setLocalTtsStatus(null);
    void refreshLocalTtsStatus();
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<LocalTtsStatus>("local-tts://status", (event) => {
      if (
        !disposed &&
        activeLocalKindRef.current === activeLocalKind &&
        event.payload.kind === activeLocalKind
      ) {
        localTtsStatusGenerationRef.current += 1;
        setLocalTtsStatus(event.payload);
      }
    }).then((dispose) => {
      if (disposed) dispose();
      else unlisten = dispose;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [activeLocalKind, refreshLocalTtsStatus]);

  const updateTts = useCallback(
    async (patch: Partial<TtsSettings>, field: string) => {
      const writeGeneration = ++settingsWriteGenerationRef.current;
      const nextSettings: TtsSettings = { ...ttsRef.current, ...patch };
      if (TTS_SYNTHESIS_CUSTOMIZATION_FIELDS.has(field)) {
        const scopeKey = scopeSettingsKey(mode);
        nextSettings[scopeKey] = {
          ...nextSettings[scopeKey],
          selected_preset_id: "",
        };
        setSynthesisPresetName("");
      }
      ttsRef.current = nextSettings;
      setTts(nextSettings);
      if (
        Object.keys(patch).some((key) =>
          [
            "provider",
            "llm_preprocessing",
            "preprocessing_enabled",
            "preprocessing_rules",
            "file_target_chars",
          ].includes(key),
        )
      ) {
        invalidateInspection();
      }
      pendingSettingsWritesRef.current += 1;
      setSavingField(field);
      setSettingsError(null);
      if (
        COALESCED_TTS_FIELDS.has(field) ||
        COALESCED_TTS_LLM_FIELDS.has(field)
      ) {
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
                scope: operationScope(mode),
                changedField: field,
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
    [invalidateInspection, mode, refreshSettings],
  );

  const updateTtsLlmPreprocessing = useCallback(
    (
      update:
        | Partial<TtsLlmPreprocessingSettings>
        | ((
            current: TtsLlmPreprocessingSettings,
          ) => Partial<TtsLlmPreprocessingSettings>),
      field: string,
    ) => {
      const current = ttsRef.current.llm_preprocessing;
      const partial = typeof update === "function" ? update(current) : update;
      return updateTts(
        {
          llm_preprocessing: {
            ...current,
            ...partial,
          },
        },
        field,
      );
    },
    [updateTts],
  );

  const flushPendingSettingsWrites = useCallback(async () => {
    if (pendingSettingsWritesRef.current === 0) {
      await settingsWriteQueueRef.current;
      return;
    }
    await updateTts({}, "__tts_settings_flush__");
  }, [updateTts]);

  const runVoicePreview = useCallback(async () => {
    const text = voicePreviewText.trim();
    if (!text) {
      setVoicePreviewError(t("textToSpeech.voice.previewEmpty"));
      return;
    }

    voicePreviewCancelRequestedRef.current = false;
    setVoicePreviewBusy(true);
    setVoicePreviewStopping(false);
    setVoicePreviewError(null);
    try {
      await flushPendingSettingsWrites();
      if (voicePreviewCancelRequestedRef.current) {
        return;
      }
      await invoke("preview_tts_voice", { text });
    } catch (error) {
      if (!voicePreviewCancelRequestedRef.current) {
        setVoicePreviewError(asErrorMessage(error));
      }
    } finally {
      voicePreviewCancelRequestedRef.current = false;
      setVoicePreviewBusy(false);
      setVoicePreviewStopping(false);
    }
  }, [flushPendingSettingsWrites, t, voicePreviewText]);

  const cancelVoicePreview = useCallback(async () => {
    voicePreviewCancelRequestedRef.current = true;
    setVoicePreviewStopping(true);
    setVoicePreviewError(null);
    try {
      await invoke("cancel_tts_operation", { operationId: null });
    } catch (error) {
      voicePreviewCancelRequestedRef.current = false;
      setVoicePreviewError(asErrorMessage(error));
      setVoicePreviewStopping(false);
    }
  }, []);

  const refreshKeyStatus = useCallback(async () => {
    const generation = ++keyStatusGenerationRef.current;
    if (!providerCapabilities.requiresApiKey) {
      setHasSeparateKey(false);
      setHasEffectiveKey(true);
      setKeyStatusLoaded(true);
      setKeyBusy(false);
      return;
    }
    setKeyBusy(true);
    setKeyStatusLoaded(false);
    try {
      const separateKeyExists = await invoke<boolean>("tts_has_api_key", {
        provider: tts.provider,
      });

      let effectiveKeyExists = separateKeyExists;
      if (providerCapabilities.supportsSharedKey && keySource === "shared") {
        effectiveKeyExists =
          tts.provider === "soniox"
            ? await invoke<boolean>("soniox_has_api_key")
            : tts.provider === "deepgram"
              ? await invoke<boolean>("deepgram_has_api_key")
              : await invoke<boolean>("llm_has_stored_api_key", {
                  feature: "post_processing",
                  providerId: "openai",
                });
      }
      if (generation !== keyStatusGenerationRef.current) return;
      setHasSeparateKey(separateKeyExists);
      setHasEffectiveKey(effectiveKeyExists);
      setKeyStatusLoaded(true);
    } catch (error) {
      if (generation !== keyStatusGenerationRef.current) return;
      setKeyStatusLoaded(false);
      setSettingsError(asErrorMessage(error));
    } finally {
      if (generation === keyStatusGenerationRef.current) {
        setKeyBusy(false);
      }
    }
  }, [
    keySource,
    providerCapabilities.requiresApiKey,
    providerCapabilities.supportsSharedKey,
    tts.provider,
  ]);

  useEffect(() => {
    void refreshKeyStatus();
    setEditingKey(false);
    setKeyDraft("");
  }, [refreshKeyStatus]);

  useEffect(() => {
    if (mode !== "files") return;

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
              attempt:
                progress.attempt ??
                progress.current_attempt ??
                progress.currentAttempt,
            };
            const reportedStatus = progress.status ?? progress.phase;
            if (reportedStatus !== undefined) {
              normalizedProgress.status = reportedStatus;
            }
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
  }, [mode]);

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

  const chooseProvider = async (provider: TtsProvider) => {
    await updateTts({ provider }, "provider");
  };

  const installLocalTts = async () => {
    const installKind = activeLocalKind;
    const installConsent = activeLocalInstallConsent;
    const requestGeneration = ++localTtsStatusGenerationRef.current;
    setLocalTtsBusy(true);
    setSettingsError(null);
    try {
      const status = await invoke<LocalTtsStatus>("install_local_tts", {
        kind: installKind,
        sourceTrusted: installConsent.sourceTrusted,
        riskAcknowledged: installConsent.riskAcknowledged,
      });
      if (
        requestGeneration === localTtsStatusGenerationRef.current &&
        activeLocalKindRef.current === installKind &&
        status.kind === installKind
      ) {
        setLocalTtsStatus(status);
      }
      setLocalInstallConsent((current) => ({
        ...current,
        [installKind]: {
          sourceTrusted: false,
          riskAcknowledged: false,
        },
      }));
    } catch (error) {
      if (activeLocalKindRef.current === installKind) {
        setSettingsError(asErrorMessage(error));
        await refreshLocalTtsStatus();
      }
    } finally {
      setLocalTtsBusy(false);
    }
  };

  const updateLocalInstallConsent = (
    field: "sourceTrusted" | "riskAcknowledged",
    value: boolean,
  ) => {
    setLocalInstallConsent((current) => ({
      ...current,
      [activeLocalKind]: {
        ...current[activeLocalKind],
        [field]: value,
      },
    }));
  };

  const openLocalInstallPath = async (path: string) => {
    if (!path) return;
    try {
      await openPath(path);
    } catch (error) {
      setSettingsError(asErrorMessage(error));
    }
  };

  const cancelLocalTtsInstall = async () => {
    try {
      await invoke("cancel_local_tts_install", { kind: activeLocalKind });
    } catch (error) {
      setSettingsError(asErrorMessage(error));
    }
  };

  const deleteLocalTts = async () => {
    setLocalTtsBusy(true);
    setSettingsError(null);
    try {
      await invoke("delete_local_tts", { kind: activeLocalKind });
      setLocalInstallConsent((current) => ({
        ...current,
        [activeLocalKind]: {
          sourceTrusted: false,
          riskAcknowledged: false,
        },
      }));
      await refreshLocalTtsStatus();
    } catch (error) {
      setSettingsError(asErrorMessage(error));
    } finally {
      setLocalTtsBusy(false);
    }
  };

  const saveKey = async () => {
    if (!keyDraft.trim() || !providerCapabilities.requiresApiKey) return;
    setKeyBusy(true);
    setSettingsError(null);
    try {
      await invoke("tts_set_api_key", {
        provider: tts.provider,
        apiKey: keyDraft.trim(),
      });
      setHasSeparateKey(true);
      setHasEffectiveKey(true);
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
      setHasEffectiveKey(false);
      setEditingKey(false);
      setKeyDraft("");
      setVoiceCatalog(null);
      setVoiceCatalogError(null);
    } catch (error) {
      setSettingsError(asErrorMessage(error));
    } finally {
      setKeyBusy(false);
    }
  };

  const testProviderKey = async () => {
    const valid = await refreshVoiceCatalog();
    if (valid) {
      toast.success(t("textToSpeech.api.testKeySuccess"));
    } else {
      toast.error(t("textToSpeech.api.testKeyFailed"));
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

  const removeRule = async (id: string) => {
    const rules = ttsRef.current.preprocessing_rules;
    const removedIndex = rules.findIndex((rule) => rule.id === id);
    if (removedIndex < 0) return;
    const removedRule = rules[removedIndex];
    await updateTts(
      {
        preprocessing_rules: rules.filter((rule) => rule.id !== id),
      },
      "preprocessing_rules",
    );
    toast.success(t("textToSpeech.preprocessing.ruleDeleted"), {
      action: {
        label: t("textToSpeech.preprocessing.undo"),
        onClick: () => {
          const currentRules = ttsRef.current.preprocessing_rules;
          if (currentRules.some((rule) => rule.id === removedRule.id)) return;
          const restoredRules = [...currentRules];
          restoredRules.splice(
            Math.min(removedIndex, restoredRules.length),
            0,
            removedRule,
          );
          void updateTts(
            { preprocessing_rules: restoredRules },
            "preprocessing_rules",
          );
        },
      },
    });
  };

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

  const currentSynthesisScope = tts[scopeSettingsKey(mode)];
  const selectedSynthesisPresetId = currentSynthesisScope.selected_preset_id;

  const saveSynthesisPreset = async () => {
    const name = synthesisPresetName.trim();
    if (!name) return;
    const existing = tts.synthesis_presets.find(
      (preset) => preset.name.toLocaleLowerCase() === name.toLocaleLowerCase(),
    );
    const config = synthesisConfigFromSettings(tts, mode);
    const preset: TtsSynthesisPreset = {
      id:
        existing?.id ??
        `tts_synthesis_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
      name,
      config,
    };
    const synthesisPresets = existing
      ? tts.synthesis_presets.map((candidate) =>
          candidate.id === existing.id ? preset : candidate,
        )
      : [...tts.synthesis_presets, preset];
    const scopeKey = scopeSettingsKey(mode);
    const updatedScope = upsertScopeConfig(tts[scopeKey], config, preset.id);
    const otherScopeKey =
      scopeKey === "interactive_synthesis"
        ? "file_synthesis"
        : "interactive_synthesis";
    const otherScope =
      existing && tts[otherScopeKey].selected_preset_id === existing.id
        ? { ...tts[otherScopeKey], selected_preset_id: "" }
        : tts[otherScopeKey];
    const scopePatch =
      scopeKey === "interactive_synthesis"
        ? {
            interactive_synthesis: updatedScope,
            file_synthesis: otherScope,
          }
        : {
            interactive_synthesis: otherScope,
            file_synthesis: updatedScope,
          };
    await updateTts(
      {
        synthesis_presets: synthesisPresets,
        ...scopePatch,
      },
      "synthesis_presets",
    );
    setSynthesisPresetName("");
  };

  const loadSynthesisPreset = async (presetId: string) => {
    if (!presetId) {
      setSynthesisPresetName("");
      const scopeKey = scopeSettingsKey(mode);
      await updateTts(
        {
          [scopeKey]: {
            ...tts[scopeKey],
            selected_preset_id: "",
          },
        },
        "__tts_synthesis_preset_clear__",
      );
      return;
    }
    const preset = tts.synthesis_presets.find(
      (candidate) => candidate.id === presetId,
    );
    if (!preset) {
      setSettingsError(t("textToSpeech.synthesisPresets.missing"));
      return;
    }
    setSynthesisPresetName(preset.name);
    const scopeKey = scopeSettingsKey(mode);
    const config = synthesisPresetConfigForMode(tts, preset, mode);
    const applied = applySynthesisConfig(tts, config, mode);
    await updateTts(
      {
        ...applied,
        [scopeKey]: upsertScopeConfig(tts[scopeKey], config, preset.id),
      },
      "synthesis_preset_load",
    );
    setOutputFormat(config.output_format);
    setMp3Bitrate(config.mp3_bitrate_kbps);
    invalidateInspection();
  };

  const deleteSelectedSynthesisPreset = async () => {
    if (!selectedSynthesisPresetId) return;
    const clearSelection = (
      scope: TtsScopeSynthesisSettings,
    ): TtsScopeSynthesisSettings =>
      scope.selected_preset_id === selectedSynthesisPresetId
        ? { ...scope, selected_preset_id: "" }
        : scope;
    await updateTts(
      {
        synthesis_presets: tts.synthesis_presets.filter(
          (preset) => preset.id !== selectedSynthesisPresetId,
        ),
        interactive_synthesis: clearSelection(tts.interactive_synthesis),
        file_synthesis: clearSelection(tts.file_synthesis),
      },
      "synthesis_presets",
    );
    setSynthesisPresetName("");
  };

  const confirmDelete = () => {
    if (!deleteConfirmation) return;
    switch (deleteConfirmation.kind) {
      case "synthesisPreset":
        void deleteSelectedSynthesisPreset();
        break;
      case "promptPreset":
        void deleteSelectedPromptPreset();
        break;
      case "localModel":
        void deleteLocalTts();
        break;
    }
  };

  const inspectionSettingsKey = useMemo(
    () =>
      JSON.stringify({
        provider: tts.provider,
        sonioxKeySource: tts.soniox_key_source,
        sonioxModel: tts.soniox_model,
        sonioxVoice: tts.soniox_voice,
        sonioxLanguage: tts.soniox_language,
        deepgramKeySource: tts.deepgram_key_source,
        deepgramModel: tts.deepgram_model,
        openaiKeySource: tts.openai_key_source,
        openaiModel: tts.openai_model,
        openaiVoice: tts.openai_voice,
        edgeVoice: tts.edge_voice,
        edgeVoiceLanguage: tts.edge_voice_language,
        localQwenVoice: tts.local_qwen_voice,
        localQwenLanguage: tts.local_qwen_language,
        localKokoroVoice: tts.local_kokoro_voice,
        localKokoroLanguage: tts.local_kokoro_language,
        windowsVoiceId: tts.windows_voice_id,
        windowsVoiceLanguage: tts.windows_voice_language,
        speed: tts.speed,
        openaiInstructions: tts.openai_instructions,
        selectedPromptId: tts.selected_prompt_id,
        preprocessingEnabled: tts.preprocessing_enabled,
        preprocessingRules: tts.preprocessing_rules,
        fileTargetChars: tts.file_target_chars,
        retryCount: tts.retry_count,
        retryBaseDelayMs: tts.retry_base_delay_ms,
        interChunkPauseMs: tts.inter_chunk_pause_ms,
        paragraphPauseMs: tts.paragraph_pause_ms,
        llmCleanupEnabled: tts.llm_preprocessing.file_enabled,
        llmChunkTargetChars: tts.llm_preprocessing.chunk_target_chars,
        outputFormat,
        mp3Bitrate,
      }),
    [
      mp3Bitrate,
      outputFormat,
      tts.deepgram_key_source,
      tts.deepgram_model,
      tts.edge_voice,
      tts.edge_voice_language,
      tts.file_target_chars,
      tts.inter_chunk_pause_ms,
      tts.llm_preprocessing.chunk_target_chars,
      tts.llm_preprocessing.file_enabled,
      tts.local_kokoro_language,
      tts.local_kokoro_voice,
      tts.local_qwen_language,
      tts.local_qwen_voice,
      tts.openai_instructions,
      tts.openai_key_source,
      tts.openai_model,
      tts.openai_voice,
      tts.paragraph_pause_ms,
      tts.preprocessing_enabled,
      tts.preprocessing_rules,
      tts.provider,
      tts.retry_base_delay_ms,
      tts.retry_count,
      tts.selected_prompt_id,
      tts.soniox_key_source,
      tts.soniox_language,
      tts.soniox_model,
      tts.soniox_voice,
      tts.speed,
      tts.windows_voice_id,
      tts.windows_voice_language,
    ],
  );

  const chooseInputFile = async () => {
    if (conversionBusyRef.current) return;
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
    if (typeof selected !== "string" || conversionBusyRef.current) return;
    setInputPath(selected);
    outputPathCustomizedRef.current = false;
    setOutputPath(defaultTtsOutputPath(selected, outputFormat));
    invalidateInspection();
    setCompletedPath("");
    setConversionError(null);
  };

  const inspectFile = useCallback(async () => {
    if (!inputPath) return;
    const inspectionGeneration = ++inspectionGenerationRef.current;
    const inspectedPath = inputPath;
    setInspecting(true);
    setConversionError(null);
    try {
      await flushPendingSettingsWrites();
      if (inspectionGeneration !== inspectionGenerationRef.current) return;
      const result = await invoke<FileInspection>("inspect_tts_text_file", {
        path: inspectedPath,
      });
      if (inspectionGeneration === inspectionGenerationRef.current) {
        setInspection(result);
      }
    } catch (error) {
      if (inspectionGeneration === inspectionGenerationRef.current) {
        setInspection(null);
        setConversionError(asErrorMessage(error));
      }
    } finally {
      if (inspectionGeneration === inspectionGenerationRef.current) {
        setInspecting(false);
      }
    }
  }, [flushPendingSettingsWrites, inputPath]);

  useEffect(() => {
    if (!inputPath) return;
    const timeoutId = window.setTimeout(() => {
      void inspectFile();
    }, 0);
    return () => window.clearTimeout(timeoutId);
  }, [inputPath, inspectFile, inspectionSettingsKey]);

  const chooseOutputFile = async () => {
    const defaultPath = defaultTtsOutputPath(inputPath, outputFormat);
    const selected = await save({
      defaultPath: outputPath || defaultPath || undefined,
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
    if (selected) {
      outputPathCustomizedRef.current = true;
      setOutputPath(selected);
    }
  };

  const openCompletedOutputFolder = async () => {
    if (!completedPath) return;
    try {
      await revealItemInDir(completedPath);
    } catch (error) {
      console.error("Failed to reveal completed TTS output:", error);
      toast.error(t("textToSpeech.conversion.openContainingFolderError"), {
        description: asErrorMessage(error),
      });
    }
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
      await flushPendingSettingsWrites();
      const result = await invoke<{
        jobId: string;
        conversion: {
          operation_id?: string;
          operationId?: string;
          output_path?: string;
          outputPath?: string;
          resumed_chunks?: number;
          resumedChunks?: number;
        };
      }>("start_tts_file_job", {
        request: {
          inputPath,
          outputPath,
          outputFormat,
          mp3Bitrate,
        },
      });
      const conversion = result.conversion;
      const returnedOperationId =
        conversion.operation_id ?? conversion.operationId;
      setOperationId(
        returnedOperationId === undefined ? null : String(returnedOperationId),
      );
      conversionOperationIdRef.current =
        returnedOperationId === undefined ? null : String(returnedOperationId);
      const finalPath = conversion.output_path ?? conversion.outputPath;
      if (finalPath) setCompletedPath(finalPath);
      const resumedChunks =
        conversion.resumed_chunks ?? conversion.resumedChunks ?? 0;
      if (resumedChunks > 0) {
        setConversionProgress((previous) => ({
          ...previous,
          message: t("textToSpeech.conversion.resumeRecovered", {
            count: resumedChunks,
          }),
        }));
      }
    } catch (error) {
      setConversionError(asErrorMessage(error));
      console.error("TTS file conversion failed:", error);
    } finally {
      conversionBusyRef.current = false;
      setConversionBusy(false);
      setConversionStopping(false);
      window.dispatchEvent(new Event("aivorelay:tts-jobs-changed"));
    }
  };

  const cancelConversion = async () => {
    setConversionStopping(true);
    setConversionError(null);
    try {
      let activeOperationId = conversionOperationIdRef.current;
      for (
        let attempt = 0;
        !activeOperationId && conversionBusyRef.current && attempt < 1_300;
        attempt += 1
      ) {
        await waitForUiTick(100);
        activeOperationId = conversionOperationIdRef.current;
      }
      if (!activeOperationId) {
        if (!conversionBusyRef.current) {
          setConversionStopping(false);
          return;
        }
        throw new Error(t("textToSpeech.conversion.operationUnavailable"));
      }
      await invoke("cancel_tts_operation", {
        operationId: activeOperationId,
      });
    } catch (error) {
      setConversionError(asErrorMessage(error));
      setConversionStopping(false);
    }
  };

  const completedChunks =
    conversionProgress?.completed_chunks ??
    conversionProgress?.completedChunks ??
    0;
  const totalChunks =
    conversionProgress?.total_chunks ??
    conversionProgress?.totalChunks ??
    (conversionBusy && llmCleanupEnabled(inspection)
      ? llmCleanupChunkCount(inspection)
      : (chunkCount(inspection) ?? 0));
  const progressPercent =
    totalChunks > 0
      ? Math.min(100, Math.round((completedChunks / totalChunks) * 100))
      : 0;
  const conversionPhase =
    conversionProgress?.phase ?? conversionProgress?.status ?? "preparing";
  const conversionStatus =
    conversionPhase === "preprocessing"
      ? t("textToSpeech.conversion.aiCleanup")
      : conversionPhase === "retrying"
      ? t("textToSpeech.overlayPlayer.retryingAttempt", {
          attempt: conversionProgress?.attempt ?? 1,
        })
      : conversionPhase === "preparing"
        ? t("textToSpeech.overlayPlayer.preparing")
        : conversionPhase === "ready" &&
            totalChunks > 0 &&
            completedChunks >= totalChunks
          ? t("textToSpeech.conversion.encoding")
          : t("textToSpeech.conversion.speechGeneration");
  const conversionProgressLabel =
    conversionPhase === "preprocessing"
      ? t("textToSpeech.conversion.aiPartProgress", {
          completed: completedChunks,
          total: totalChunks,
        })
      : t("textToSpeech.conversion.speechChunkProgress", {
          completed: completedChunks,
          total: totalChunks,
        });
  const keySourceLabel =
    tts.provider === "openai" || tts.provider === "openai_compatible"
      ? t("textToSpeech.api.sameAsOpenAi")
      : t("textToSpeech.api.sameAsStt", {
          provider: tts.provider === "soniox" ? "Soniox" : "Deepgram",
        });
  const selectedApiKeyMissing =
    providerCapabilities.requiresApiKey &&
    tts.provider !== "openai_compatible" &&
    !keyBusy &&
    (keySource === "separate"
      ? !hasSeparateKey
      : keyStatusLoaded && !hasEffectiveKey);

  useEffect(() => {
    setOutputFormat(tts.output_format);
    setMp3Bitrate(tts.mp3_bitrate_kbps);
  }, [tts.mp3_bitrate_kbps, tts.output_format]);

  useEffect(() => {
    if (!inputPath || outputPathCustomizedRef.current) return;
    setOutputPath(defaultTtsOutputPath(inputPath, outputFormat));
  }, [inputPath, outputFormat]);

  const handleInteractiveHistoryAvailability = useCallback(
    (hasEntries: boolean) => setInteractiveHistoryHasEntries(hasEntries),
    [],
  );
  const playHistoryFallbackUnavailable =
    !tts.interactive_history_enabled || !interactiveHistoryHasEntries;
  const playHistoryFallbackDescription = !tts.interactive_history_enabled
    ? t("textToSpeech.overlay.historyFallbackRequiresHistory")
    : !interactiveHistoryHasEntries
      ? t("textToSpeech.overlay.historyFallbackRequiresResult")
      : t("textToSpeech.overlay.historyFallbackDescription");

  const dismissFirstVisitNotice = () => {
    setShowFirstVisitNotice(false);
    try {
      window.localStorage.setItem(
        FIRST_VISIT_NOTICE_STORAGE_KEYS[mode],
        "dismissed",
      );
    } catch {
      // The notice still stays dismissed for this session when storage is unavailable.
    }
  };

  return (
    <div className="mx-auto w-full max-w-3xl space-y-6 pb-12">
      <TtsBetaBanner />

      {showFirstVisitNotice && (
        <div
          role="note"
          className="flex flex-col gap-3 rounded-lg border border-violet-300/30 bg-violet-400/[0.09] px-4 py-4 text-sm text-violet-50 sm:flex-row sm:items-start"
        >
          <Sparkles className="mt-0.5 h-5 w-5 shrink-0 text-violet-200" />
          <div className="min-w-0 flex-1 space-y-1.5">
            <p className="font-semibold">
              {t("textToSpeech.firstVisit.title")}
            </p>
            <p className="leading-relaxed text-violet-50/85">
              {t("textToSpeech.firstVisit.description")}
            </p>
            {mode === "interactive" && (
              <p className="leading-relaxed text-violet-100">
                {t("textToSpeech.firstVisit.interactiveReminder")}
              </p>
            )}
          </div>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="shrink-0 self-start"
            onClick={dismissFirstVisitNotice}
          >
            {t("textToSpeech.firstVisit.dismiss")}
          </Button>
        </div>
      )}

      {settingsError && (
        <div
          role="alert"
          className="flex items-start gap-2 rounded-lg border border-red-500/35 bg-red-500/10 px-4 py-3 text-sm text-red-200"
        >
          <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" />
          <span>{settingsError}</span>
        </div>
      )}

      <>
        {mode === "interactive" && (
          <SettingsGroup
            title={t("textToSpeech.title")}
            description={t("textToSpeech.description")}
            help={
              <TtsHelpDisclosure
                summary={t("textToSpeech.help.providerSummary", {
                  provider: providerLabel,
                })}
                items={[
                  {
                    term: t("textToSpeech.help.provider"),
                    description: providerLabel,
                  },
                  {
                    term: t("textToSpeech.help.currentModel"),
                    description: modelValue,
                  },
                  {
                    term: t("textToSpeech.help.defaultChoice"),
                    description: t(
                      "textToSpeech.help.defaultChoiceDescription",
                    ),
                  },
                ]}
                links={providerHelpLinks}
              />
            }
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
              <Select
                className="w-full md:w-72"
                value={tts.provider}
                options={providerSelectOptions}
                onChange={(value) => {
                  if (value) void chooseProvider(value as TtsProvider);
                }}
                isClearable={false}
                disabled={savingField !== null}
              />
            </SettingContainer>
            {selectedApiKeyMissing && (
              <div
                role="alert"
                className="flex items-start gap-3 bg-amber-400/10 px-6 py-4 text-sm text-amber-100"
              >
                <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" />
                <div>
                  <span>
                    {t("textToSpeech.api.missingKeyWarning", {
                      provider: providerSelectOptions.find(
                        (item) => item.value === tts.provider,
                      )?.label,
                    })}
                  </span>{" "}
                  <a
                    href="#tts-api-settings"
                    onClick={(event) => {
                      event.preventDefault();
                      const target = document.getElementById("tts-api-settings");
                      if (target) {
                        scrollAndFocusAnchor(target, "nearest");
                      }
                    }}
                    className="font-semibold text-amber-200 underline decoration-amber-200/60 underline-offset-2 hover:text-amber-100"
                  >
                    {t("textToSpeech.api.configureKey")}
                  </a>
                </div>
              </div>
            )}
          </SettingsGroup>
        )}

        {mode === "files" && (
          <SettingsGroup
            title={t("textToSpeech.fileSettings.title")}
            description={t("textToSpeech.fileSettings.description")}
            help={
              <TtsHelpDisclosure
                summary={t("textToSpeech.help.providerSummary", {
                  provider: providerLabel,
                })}
                items={[
                  {
                    term: t("textToSpeech.help.provider"),
                    description: providerLabel,
                  },
                  {
                    term: t("textToSpeech.help.currentModel"),
                    description: modelValue,
                  },
                  {
                    term: t("textToSpeech.help.fileScope"),
                    description: t("textToSpeech.help.fileScopeDescription"),
                  },
                ]}
                links={providerHelpLinks}
              />
            }
          >
            <SettingContainer
              grouped
              title={t("textToSpeech.provider.title")}
              description={t("textToSpeech.provider.description")}
            >
              <Select
                className="w-full md:w-72"
                value={tts.provider}
                options={providerSelectOptions}
                onChange={(value) => {
                  if (value) void chooseProvider(value as TtsProvider);
                }}
                isClearable={false}
                disabled={savingField !== null}
              />
            </SettingContainer>
            {selectedApiKeyMissing && (
              <div
                role="alert"
                className="flex items-start gap-3 bg-amber-400/10 px-6 py-4 text-sm text-amber-100"
              >
                <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" />
                <div>
                  <span>
                    {t("textToSpeech.api.missingKeyWarning", {
                      provider: providerSelectOptions.find(
                        (item) => item.value === tts.provider,
                      )?.label,
                    })}
                  </span>{" "}
                  <a
                    href="#tts-api-settings"
                    onClick={(event) => {
                      event.preventDefault();
                      const target = document.getElementById("tts-api-settings");
                      if (target) {
                        scrollAndFocusAnchor(target, "nearest");
                      }
                    }}
                    className="font-semibold text-amber-200 underline decoration-amber-200/60 underline-offset-2 hover:text-amber-100"
                  >
                    {t("textToSpeech.api.configureKey")}
                  </a>
                </div>
              </div>
            )}
          </SettingsGroup>
        )}

        {mode === "files" && (
          <>
            <TtsUnfinishedJobs
              activeOperationId={conversionBusy ? operationId : null}
              activeCompletedChunks={
                conversionBusy ? completedChunks : undefined
              }
              activeTotalChunks={conversionBusy ? totalChunks : undefined}
              activeProgressStage={
                conversionBusy && conversionPhase === "preprocessing"
                  ? "ai_cleanup"
                  : conversionBusy
                    ? "speech"
                    : undefined
              }
            />
            <SettingsGroup
              title={t("textToSpeech.conversion.title")}
              description={t("textToSpeech.conversion.description")}
              help={
                <TtsHelpDisclosure
                  summary={t("textToSpeech.help.conversionSummary")}
                  items={[
                    {
                      term: t("textToSpeech.help.input"),
                      description: t("textToSpeech.help.inputDescription"),
                    },
                    {
                      term: t("textToSpeech.help.output"),
                      description: t("textToSpeech.help.outputDescription"),
                    },
                    {
                      term: t("textToSpeech.help.resume"),
                      description: t("textToSpeech.help.resumeDescription"),
                    },
                  ]}
                  links={aivoRelayTtsGuideLink}
                />
              }
            >
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
                  <ActionTooltip
                    content={
                      conversionBusy
                        ? t("textToSpeech.conversion.disabledWhileRunning")
                        : null
                    }
                  >
                    <Button
                      variant="secondary"
                      disabled={conversionBusy}
                      onClick={() => void chooseInputFile()}
                    >
                      <FileText className="mr-2 inline h-4 w-4" />
                      {t("textToSpeech.conversion.chooseFile")}
                    </Button>
                  </ActionTooltip>
                </div>
                {inputPath && inspecting && (
                  <div
                    role="status"
                    aria-live="polite"
                    className="mt-3 flex items-center gap-2 text-xs text-[#a0a0a0]"
                  >
                    <Loader2 className="h-4 w-4 animate-spin" />
                    {t("textToSpeech.conversion.inspect")}
                  </div>
                )}
                {inspection && (
                  <>
                    <div className="mt-3 grid grid-cols-2 gap-3 xl:grid-cols-4">
                      {[
                        [
                          t("textToSpeech.conversion.sourceCharacters"),
                          characterCount(inspection, "source"),
                        ],
                        [
                          inspectionCountsAreEstimates(inspection)
                            ? t(
                                "textToSpeech.conversion.estimatedProcessedCharacters",
                              )
                            : t(
                                "textToSpeech.conversion.processedCharacters",
                              ),
                          characterCount(inspection, "processed"),
                        ],
                        [
                          t("textToSpeech.conversion.aiCleanupRequests"),
                          llmCleanupChunkCount(inspection),
                        ],
                        [
                          inspectionCountsAreEstimates(inspection)
                            ? t(
                                "textToSpeech.conversion.estimatedSpeechRequests",
                              )
                            : t("textToSpeech.conversion.speechRequests"),
                          chunkCount(inspection),
                        ],
                      ].map(([label, value]) => (
                        <div
                          key={String(label)}
                          className="rounded-lg border border-white/[0.06] bg-black/15 p-3"
                        >
                          <div className="text-xs text-[#808080]">{label}</div>
                          <div className="mt-1 text-lg font-semibold text-[#f5f5f5]">
                            {typeof value === "number"
                              ? value.toLocaleString()
                              : "—"}
                          </div>
                        </div>
                      ))}
                    </div>
                    <p className="mt-3 text-xs leading-relaxed text-[#909090]">
                      {inspectionCountsAreEstimates(inspection)
                        ? t("textToSpeech.conversion.localEstimateDisclosure")
                        : t("textToSpeech.conversion.localInspectionDisclosure")}
                    </p>
                  </>
                )}
              </SettingContainer>
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
                    {t("textToSpeech.conversion.cliBatchExample")}
                  </code>
                  <p>{t("textToSpeech.conversion.cliBatchDescription")}</p>
                  <code className="block overflow-x-auto rounded-lg bg-black/30 p-3 text-[#e8e8e8]">
                    {t("textToSpeech.conversion.cliOverrideExample")}
                  </code>
                  <p>{t("textToSpeech.conversion.cliOverrideDescription")}</p>
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
                title={t("textToSpeech.conversion.finalFormatTitle")}
                description={t(
                  "textToSpeech.conversion.finalFormatDescription",
                )}
              >
                <Select
                  className="w-full md:w-72"
                  value={outputFormat}
                  options={[
                    { value: "mp3", label: "MP3" },
                    { value: "wav", label: "WAV" },
                  ]}
                  onChange={(value) => {
                    if (!value) return;
                    const format = value as TtsOutputFormat;
                    setOutputFormat(format);
                    if (!outputPathCustomizedRef.current) {
                      setOutputPath(defaultTtsOutputPath(inputPath, format));
                    }
                    void updateTts({ output_format: format }, "output_format");
                  }}
                  isClearable={false}
                  disabled={conversionBusy}
                />
              </SettingContainer>
              {outputFormat === "mp3" && (
                <SettingContainer
                  grouped
                  title={t("textToSpeech.conversion.bitrateTitle")}
                  description={t("textToSpeech.conversion.bitrateDescription")}
                >
                  <Select
                    className="w-full md:w-72"
                    value={String(mp3Bitrate)}
                    options={BITRATES.map((bitrate) => ({
                      value: String(bitrate),
                      label: `${bitrate} kb/s`,
                    }))}
                    onChange={(value) => {
                      if (!value) return;
                      const bitrate = Number(value);
                      setMp3Bitrate(bitrate);
                      void updateTts(
                        { mp3_bitrate_kbps: bitrate },
                        "mp3_bitrate_kbps",
                      );
                    }}
                    isClearable={false}
                    disabled={conversionBusy}
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
                  <ActionTooltip
                    content={
                      conversionBusy
                        ? t("textToSpeech.conversion.disabledWhileRunning")
                        : !inputPath
                          ? t("textToSpeech.conversion.chooseInputFirst")
                          : null
                    }
                  >
                    <Button
                      variant="secondary"
                      disabled={!inputPath || conversionBusy}
                      onClick={() => void chooseOutputFile()}
                    >
                      <FileAudio className="mr-2 inline h-4 w-4" />
                      {t("textToSpeech.conversion.saveAs")}
                    </Button>
                  </ActionTooltip>
                </div>
              </SettingContainer>

              {(conversionBusy || conversionProgress) && (
                <div className="space-y-2 px-6 py-4">
                  <div className="flex items-center justify-between text-xs text-[#b8b8b8]">
                    <span>
                      {conversionStatus}{" "}
                      {totalChunks > 0 && conversionPhase !== "preparing"
                        ? conversionProgressLabel
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
                <ActionTooltip
                  content={
                    conversionBusy
                      ? t("textToSpeech.conversion.alreadyRunning")
                      : !inputPath
                        ? t("textToSpeech.conversion.chooseInputFirst")
                        : inspecting || !inspection
                          ? t("textToSpeech.conversion.waitForInspection")
                          : !outputPath
                            ? t("textToSpeech.conversion.chooseOutputFirst")
                            : null
                  }
                >
                  <Button
                    disabled={!inspection || !outputPath || conversionBusy}
                    onClick={() => void convertFile()}
                  >
                    {conversionBusy && (
                      <Loader2 className="mr-2 inline h-4 w-4 animate-spin" />
                    )}
                    {conversionBusy
                      ? t("textToSpeech.conversion.convertingAction")
                      : t("textToSpeech.conversion.startConversion")}
                  </Button>
                </ActionTooltip>
                {conversionBusy && (
                  <ActionTooltip
                    content={
                      conversionStopping
                        ? t("textToSpeech.conversion.stopAlreadyRequested")
                        : null
                    }
                  >
                    <Button
                      variant="secondary"
                      disabled={conversionStopping}
                      onClick={() => void cancelConversion()}
                    >
                      {conversionStopping && (
                        <Loader2 className="mr-2 inline h-4 w-4 animate-spin" />
                      )}
                      {conversionStopping
                        ? t("textToSpeech.conversion.stopping")
                        : t("textToSpeech.conversion.pause")}
                    </Button>
                  </ActionTooltip>
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
                <div className="flex flex-wrap items-start justify-between gap-3 px-6 py-4 text-sm text-green-300">
                  <div
                    role="status"
                    aria-live="polite"
                    className="flex min-w-0 flex-1 items-start gap-2"
                  >
                    <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0" />
                    <span className="min-w-0 break-all">
                      {t("textToSpeech.conversion.completed", {
                        path: completedPath,
                      })}
                    </span>
                  </div>
                  <Button
                    variant="secondary"
                    size="sm"
                    className="shrink-0"
                    title={t("textToSpeech.conversion.openContainingFolder")}
                    onClick={() => void openCompletedOutputFolder()}
                  >
                    <FolderOpen className="mr-2 inline h-4 w-4" />
                    {t("textToSpeech.conversion.openContainingFolder")}
                  </Button>
                </div>
              )}
            </SettingsGroup>
          </>
        )}

        <SettingsGroup
          title={t("textToSpeech.synthesisPresets.title")}
          description={t("textToSpeech.synthesisPresets.description")}
          help={
            <TtsHelpDisclosure
              summary={t("textToSpeech.help.presetsSummary")}
              items={[
                {
                  term: t("textToSpeech.help.savedValues"),
                  description: t("textToSpeech.help.savedValuesDescription"),
                },
                {
                  term: t("textToSpeech.help.excludedValues"),
                  description: t("textToSpeech.help.excludedValuesDescription"),
                },
              ]}
              links={aivoRelayTtsGuideLink}
            />
          }
        >
          <div className="border-b border-white/[0.05] px-6 py-4 text-xs leading-relaxed text-text/65">
            {t("textToSpeech.synthesisPresets.modelMemory")}
          </div>
          <SettingContainer
            grouped
            layout="stacked"
            title={t("textToSpeech.synthesisPresets.loadTitle")}
            description={t("textToSpeech.synthesisPresets.loadDescription")}
            descriptionMode="inline"
          >
            <div className="flex flex-wrap gap-2">
              <Select
                className="min-w-64 flex-1"
                value={selectedSynthesisPresetId}
                options={[
                  {
                    value: "",
                    label: t("textToSpeech.synthesisPresets.custom"),
                  },
                  ...tts.synthesis_presets.map((preset) => ({
                    value: preset.id,
                    label: preset.name,
                  })),
                ]}
                onChange={(value) => void loadSynthesisPreset(value ?? "")}
                isClearable={false}
                disabled={savingField !== null}
              />
              <ActionTooltip
                content={
                  savingField !== null
                    ? t("textToSpeech.disabledWhileSaving")
                    : !selectedSynthesisPresetId
                      ? t("textToSpeech.synthesisPresets.selectToDelete")
                      : null
                }
              >
                <Button
                  variant="danger"
                  disabled={!selectedSynthesisPresetId || savingField !== null}
                  onClick={() =>
                    setDeleteConfirmation({
                      kind: "synthesisPreset",
                      name:
                        tts.synthesis_presets.find(
                          (preset) => preset.id === selectedSynthesisPresetId,
                        )?.name ?? "",
                    })
                  }
                >
                  <Trash2 className="mr-2 inline h-4 w-4" />
                  {t("textToSpeech.synthesisPresets.delete")}
                </Button>
              </ActionTooltip>
            </div>
          </SettingContainer>
          <SettingContainer
            grouped
            layout="stacked"
            title={t("textToSpeech.synthesisPresets.saveTitle")}
            description={t("textToSpeech.synthesisPresets.saveDescription")}
            descriptionMode="inline"
          >
            <div className="flex flex-wrap gap-2">
              <Input
                className="min-w-64 flex-1"
                value={synthesisPresetName}
                maxLength={256}
                placeholder={t("textToSpeech.synthesisPresets.namePlaceholder")}
                onChange={(event) => setSynthesisPresetName(event.target.value)}
              />
              <ActionTooltip
                content={
                  savingField !== null
                    ? t("textToSpeech.disabledWhileSaving")
                    : !synthesisPresetName.trim()
                      ? t("textToSpeech.synthesisPresets.enterName")
                      : null
                }
              >
                <Button
                  variant="secondary"
                  disabled={!synthesisPresetName.trim() || savingField !== null}
                  onClick={() => void saveSynthesisPreset()}
                >
                  <SaveIcon className="mr-2 inline h-4 w-4" />
                  {t("textToSpeech.synthesisPresets.save")}
                </Button>
              </ActionTooltip>
            </div>
          </SettingContainer>
          <div className="px-6 py-4 text-xs leading-relaxed text-text/60">
            {t("textToSpeech.synthesisPresets.exclusions")}
          </div>
        </SettingsGroup>

        {(tts.provider === "local_qwen" || tts.provider === "local_kokoro") && (
          <SettingsGroup
            title={t("textToSpeech.local.title")}
            description={t("textToSpeech.local.description")}
            help={
              <TtsHelpDisclosure
                summary={t("textToSpeech.help.localSummary")}
                items={[
                  {
                    term: t("textToSpeech.help.source"),
                    description: t("textToSpeech.help.localSourceDescription"),
                  },
                  {
                    term: t("textToSpeech.help.storage"),
                    description: t("textToSpeech.help.localStorageDescription"),
                  },
                  {
                    term: t("textToSpeech.help.license"),
                    description: t("textToSpeech.help.localLicenseDescription"),
                  },
                ]}
                links={[
                  {
                    label: t("textToSpeech.help.modelSource"),
                    href:
                      localTtsStatus?.model_source_url ??
                      localInstallMetadata.sourceUrl,
                  },
                  {
                    label: t("textToSpeech.help.webLicense"),
                    href:
                      localTtsStatus?.model_license_url ??
                      localInstallMetadata.licenseUrl,
                  },
                ]}
              />
            }
          >
            <SettingContainer
              grouped
              layout="stacked"
              title={t("textToSpeech.local.modelTitle")}
              description={t("textToSpeech.local.modelDescription", {
                size: formatBytes(
                  localTtsStatus?.model_download_bytes ??
                    (tts.provider === "local_kokoro"
                      ? 147_031_220
                      : 2_498_388_392),
                ),
              })}
              descriptionMode="inline"
            >
              <div className="space-y-4">
                <dl className="grid gap-2 rounded-lg border border-white/10 bg-white/[0.035] p-3 text-xs sm:grid-cols-[minmax(8rem,0.32fr)_minmax(0,1fr)]">
                  <dt className="font-semibold text-text/75">
                    {t("textToSpeech.local.source")}
                  </dt>
                  <dd className="min-w-0">
                    <div className="font-medium text-text/80">
                      {localTtsStatus?.model_author ??
                        localInstallMetadata.author}
                    </div>
                    <a
                      className="mt-1 inline-flex items-start gap-1 break-all text-[#d7b9ff] underline-offset-4 hover:underline"
                      href={
                        localTtsStatus?.model_source_url ??
                        localInstallMetadata.sourceUrl
                      }
                      target="_blank"
                      rel="noopener noreferrer"
                    >
                      {localTtsStatus?.model_source_url ??
                        localInstallMetadata.sourceUrl}
                      <ExternalLink className="h-3.5 w-3.5 shrink-0" />
                    </a>
                    <div className="mt-1 break-all text-text/55">
                      {localTtsStatus?.model_repository ??
                        (tts.provider === "local_kokoro"
                          ? "k2-fsa/sherpa-onnx/tts-models/kokoro-int8-multi-lang-v1_1"
                          : "Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice")}
                    </div>
                    <div className="break-all text-text/45">
                      {localTtsStatus?.model_revision ??
                        (tts.provider === "local_kokoro"
                          ? "tts-models:a1e94694776049035c4f2c6529f003aaece993c76aae9a78995831c3c4dcafc6"
                          : "85e237c12c027371202489a0ec509ded67b5e4b5")}
                    </div>
                  </dd>

                  <dt className="font-semibold text-text/75">
                    {t("textToSpeech.local.installPath")}
                  </dt>
                  <dd className="min-w-0 break-all text-text/65">
                    {localTtsStatus?.install_root ||
                      t("textToSpeech.local.pathLoading")}
                  </dd>

                  <dt className="font-semibold text-text/75">
                    {t("textToSpeech.local.diskSpace")}
                  </dt>
                  <dd className="text-text/65">
                    {localTtsStatus?.installed
                      ? t("textToSpeech.local.installedSize", {
                          size: formatBytes(
                            localTtsStatus.installed_size_bytes,
                          ),
                        })
                      : (localTtsStatus?.installed_size_bytes ?? 0) > 0
                        ? t("textToSpeech.local.existingAndEstimatedSize", {
                            actual: formatBytes(
                              localTtsStatus?.installed_size_bytes ?? 0,
                            ),
                            estimated: formatBytes(
                              localTtsStatus?.estimated_install_bytes ??
                                localInstallMetadata.estimatedInstallBytes,
                            ),
                          })
                        : t("textToSpeech.local.estimatedSize", {
                            size: formatBytes(
                              localTtsStatus?.estimated_install_bytes ??
                                localInstallMetadata.estimatedInstallBytes,
                            ),
                          })}
                  </dd>

                  <dt className="font-semibold text-text/75">
                    {t("textToSpeech.local.licenseTitle")}
                  </dt>
                  <dd className="min-w-0 text-text/65">
                    <a
                      className="inline-flex items-center gap-1 text-[#d7b9ff] underline-offset-4 hover:underline"
                      href={
                        localTtsStatus?.model_license_url ??
                        localInstallMetadata.licenseUrl
                      }
                      target="_blank"
                      rel="noopener noreferrer"
                    >
                      {localTtsStatus?.model_license_name ??
                        localInstallMetadata.licenseName}
                      <ExternalLink className="h-3.5 w-3.5 shrink-0" />
                    </a>
                  </dd>
                </dl>

                <div
                  role="status"
                  className="rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-sm"
                >
                  {localTtsStatus?.installed
                    ? t("textToSpeech.local.ready", {
                        profile: localTtsStatus.runtime_profile.toUpperCase(),
                      })
                    : localTtsStatus?.installing
                      ? t("textToSpeech.local.installing", {
                          phase: localTtsStatus.phase,
                          percentage: localTtsStatus.percentage.toFixed(1),
                        })
                      : t("textToSpeech.local.notInstalled")}
                </div>
                {localTtsStatus?.installing &&
                  localTtsStatus.total_bytes > 0 && (
                    <div className="h-2 overflow-hidden rounded-full bg-white/10">
                      <div
                        className="h-full bg-[#a66cff] transition-[width]"
                        style={{
                          width: `${Math.min(100, localTtsStatus.percentage)}%`,
                        }}
                      />
                    </div>
                  )}
                {localTtsStatus?.error && (
                  <div role="alert" className="text-sm text-red-300">
                    {localTtsStatus.error}
                  </div>
                )}

                {!localTtsStatus?.installed && !localTtsStatus?.installing && (
                  <div className="space-y-3 rounded-lg border border-amber-300/25 bg-amber-300/[0.07] p-3">
                    <p className="text-xs leading-relaxed text-amber-100/90">
                      {t("textToSpeech.local.downloadWarning", {
                        author:
                          localTtsStatus?.model_author ??
                          localInstallMetadata.author,
                      })}
                    </p>
                    <label className="flex cursor-pointer items-start gap-3 text-xs leading-relaxed text-text/80">
                      <input
                        className="mt-0.5 h-4 w-4 shrink-0 accent-[#a66cff]"
                        type="checkbox"
                        checked={activeLocalInstallConsent.sourceTrusted}
                        onChange={(event) =>
                          updateLocalInstallConsent(
                            "sourceTrusted",
                            event.target.checked,
                          )
                        }
                      />
                      <span>
                        {t("textToSpeech.local.trustSource", {
                          author:
                            localTtsStatus?.model_author ??
                            localInstallMetadata.author,
                        })}
                      </span>
                    </label>
                    <label className="flex cursor-pointer items-start gap-3 text-xs leading-relaxed text-text/80">
                      <input
                        className="mt-0.5 h-4 w-4 shrink-0 accent-[#a66cff]"
                        type="checkbox"
                        checked={activeLocalInstallConsent.riskAcknowledged}
                        onChange={(event) =>
                          updateLocalInstallConsent(
                            "riskAcknowledged",
                            event.target.checked,
                          )
                        }
                      />
                      <span>{t("textToSpeech.local.understandRisks")}</span>
                    </label>
                  </div>
                )}

                <div className="flex flex-wrap gap-2">
                  {!localTtsStatus?.installed &&
                    !localTtsStatus?.installing && (
                      <ActionTooltip
                        content={
                          localTtsBusy
                            ? t("textToSpeech.local.operationInProgress")
                            : !localTtsStatus?.install_root
                              ? t("textToSpeech.local.pathUnavailable")
                              : !activeLocalInstallConsent.sourceTrusted ||
                                  !activeLocalInstallConsent.riskAcknowledged
                                ? t("textToSpeech.local.acceptWarningsFirst")
                                : null
                        }
                      >
                        <Button
                          variant="primary"
                          disabled={
                            localTtsBusy ||
                            !localTtsStatus?.install_root ||
                            !activeLocalInstallConsent.sourceTrusted ||
                            !activeLocalInstallConsent.riskAcknowledged
                          }
                          onClick={() => void installLocalTts()}
                        >
                          {localTtsBusy && (
                            <Loader2 className="mr-2 inline h-4 w-4 animate-spin" />
                          )}
                          {(localTtsStatus?.installed_size_bytes ?? 0) > 0
                            ? t("textToSpeech.local.repairOrInstall")
                            : t("textToSpeech.local.install")}
                        </Button>
                      </ActionTooltip>
                    )}
                  {localTtsStatus?.installing && (
                    <Button
                      variant="secondary"
                      onClick={() => void cancelLocalTtsInstall()}
                    >
                      {t("textToSpeech.local.cancel")}
                    </Button>
                  )}
                  {localTtsStatus?.installed && (
                    <>
                      <ActionTooltip
                        content={
                          !localTtsStatus.install_root
                            ? t("textToSpeech.local.pathUnavailable")
                            : null
                        }
                      >
                        <Button
                          variant="secondary"
                          disabled={!localTtsStatus.install_root}
                          onClick={() =>
                            void openLocalInstallPath(
                              localTtsStatus.install_root,
                            )
                          }
                        >
                          {t("textToSpeech.local.openInstallFolder")}
                        </Button>
                      </ActionTooltip>
                      {localTtsStatus.model_license_available &&
                        localTtsStatus.model_license_path && (
                          <Button
                            variant="secondary"
                            onClick={() =>
                              void openLocalInstallPath(
                                localTtsStatus.model_license_path,
                              )
                            }
                          >
                            {t("textToSpeech.local.openLocalLicense")}
                          </Button>
                        )}
                      <ActionTooltip
                        content={
                          localTtsBusy
                            ? t("textToSpeech.local.operationInProgress")
                            : null
                        }
                      >
                        <Button
                          variant="danger"
                          disabled={localTtsBusy}
                          onClick={() =>
                            setDeleteConfirmation({ kind: "localModel" })
                          }
                        >
                          <Trash2 className="mr-2 inline h-4 w-4" />
                          {t("textToSpeech.local.delete")}
                        </Button>
                      </ActionTooltip>
                    </>
                  )}
                </div>
                {localTtsStatus?.installed && (
                  <div className="space-y-1 text-xs leading-relaxed text-text/60">
                    <p className="break-all">
                      {t("textToSpeech.local.localLicensePath", {
                        path:
                          localTtsStatus.model_license_path ||
                          t("textToSpeech.local.unavailable"),
                      })}
                    </p>
                    {localTtsStatus.model_license_declaration_path && (
                      <p className="break-all">
                        {t("textToSpeech.local.localDeclarationPath", {
                          path: localTtsStatus.model_license_declaration_path,
                        })}
                      </p>
                    )}
                  </div>
                )}
                <p className="text-xs leading-relaxed text-amber-200/90">
                  {t("textToSpeech.local.resourceWarning")}
                </p>
                <p className="text-xs leading-relaxed text-text/60">
                  {t("textToSpeech.local.license")}
                </p>
              </div>
            </SettingContainer>
          </SettingsGroup>
        )}

        {mode === "interactive" && (
          <SettingsGroup
            title={t("textToSpeech.actions.title")}
            help={
              <TtsHelpDisclosure
                summary={t("textToSpeech.help.actionsSummary")}
                items={[
                  {
                    term: t("textToSpeech.help.clipboardAction"),
                    description: t(
                      "textToSpeech.help.clipboardActionDescription",
                    ),
                  },
                  {
                    term: t("textToSpeech.help.directSelection"),
                    description: t(
                      "textToSpeech.help.directSelectionDescription",
                    ),
                  },
                ]}
                links={aivoRelayTtsGuideLink}
              />
            }
          >
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
            <SettingContainer
              grouped
              layout="stacked"
              title={t("textToSpeech.actions.readSelectionDirectTitle")}
              description={
                osKind === "windows"
                  ? t("textToSpeech.actions.readSelectionDirectDescription")
                  : t("textToSpeech.actions.readSelectionDirectWindowsOnly")
              }
              descriptionMode="inline"
            >
              <HandyShortcut
                shortcutId="read_selection_direct_tts"
                grouped
                disabled={osKind !== "windows"}
              />
            </SettingContainer>
          </SettingsGroup>
        )}

        <SettingsGroup
          title={t("textToSpeech.voice.title")}
          description={t("textToSpeech.voice.description")}
          help={
            <TtsHelpDisclosure
              summary={t("textToSpeech.help.voiceSummary", {
                provider: providerLabel,
              })}
              items={[
                {
                  term: t("textToSpeech.help.model"),
                  description: t("textToSpeech.help.modelDescription"),
                },
                {
                  term: t("textToSpeech.help.voice"),
                  description: t("textToSpeech.help.voiceDescription"),
                },
                {
                  term: t("textToSpeech.help.language"),
                  description: t("textToSpeech.help.languageDescription"),
                },
                {
                  term: t("textToSpeech.help.speed"),
                  description: t("textToSpeech.help.speedDescription", {
                    minimum: speedMinimum,
                    maximum: speedMaximum,
                  }),
                },
              ]}
              links={providerHelpLinks}
            />
          }
        >
          <SettingContainer
            grouped
            title={t("textToSpeech.voice.voiceTitle")}
            description={t("textToSpeech.voice.voiceDescription")}
          >
            {tts.provider === "local_qwen" ? (
              <Select
                className="w-full md:w-72"
                value={tts.local_qwen_voice}
                options={LOCAL_QWEN_VOICES}
                onChange={(value) => {
                  if (value) {
                    void updateTts(
                      { local_qwen_voice: value },
                      "local_qwen_voice",
                    );
                  }
                }}
                isClearable={false}
                disabled={savingField !== null}
              />
            ) : tts.provider === "local_kokoro" ? (
              <Select
                className="w-full md:w-72"
                value={tts.local_kokoro_voice}
                options={
                  tts.local_kokoro_language === "Chinese"
                    ? LOCAL_KOKORO_CHINESE_VOICES
                    : LOCAL_KOKORO_ENGLISH_VOICES
                }
                onChange={(value) => {
                  if (value) {
                    void updateTts(
                      { local_kokoro_voice: value },
                      "local_kokoro_voice",
                    );
                  }
                }}
                isClearable={false}
                disabled={savingField !== null}
              />
            ) : tts.provider === "windows" ? (
              <div className="w-full space-y-2 md:w-96">
                {windowsCatalogBusy ? (
                  <p className="text-sm text-text/60">
                    {t("textToSpeech.windows.loading")}
                  </p>
                ) : windowsCatalog && !windowsCatalog.available ? (
                  <p className="text-sm text-red-300">
                    {windowsCatalog.unavailable_reason}
                  </p>
                ) : windowsCatalog?.voices.length === 0 ? (
                  <p className="text-sm text-amber-200">
                    {t("textToSpeech.windows.noVoices")}
                  </p>
                ) : (
                  <Select
                    value={tts.windows_voice_id}
                    options={[
                      {
                        value: "",
                        label: t("textToSpeech.windows.defaultVoice"),
                        group: t("textToSpeech.windows.systemGroup"),
                      },
                      ...(windowsCatalog?.voices ?? []).map((voice) => ({
                        value: voice.id,
                        label: `${voice.display_name} - ${voice.language}`,
                        group:
                          voice.language || t("textToSpeech.voice.otherGroup"),
                      })),
                    ]}
                    isClearable={false}
                    onChange={(value) => {
                      const selected = value ?? "";
                      const language =
                        windowsCatalog?.voices.find(
                          (voice) => voice.id === selected,
                        )?.language ?? "";
                      void updateTts(
                        {
                          windows_voice_id: selected,
                          windows_voice_language: language,
                        },
                        "windows_voice_id",
                      );
                    }}
                    disabled={savingField !== null}
                  />
                )}
                <ActionTooltip
                  content={
                    windowsCatalogBusy
                      ? t("textToSpeech.windows.refreshInProgress")
                      : null
                  }
                >
                  <Button
                    variant="secondary"
                    disabled={windowsCatalogBusy}
                    onClick={() => void refreshWindowsCatalog()}
                  >
                    {windowsCatalogBusy && (
                      <Loader2 className="mr-2 inline h-4 w-4 animate-spin" />
                    )}
                    {t("textToSpeech.windows.refresh")}
                  </Button>
                </ActionTooltip>
                <p className="text-xs text-text/60">
                  {t("textToSpeech.windows.help")}
                </p>
              </div>
            ) : tts.provider === "soniox" ||
              tts.provider === "deepgram" ||
              tts.provider === "openai" ||
              tts.provider === "murf" ||
              tts.provider === "elevenlabs" ||
              tts.provider === "cartesia" ||
              tts.provider === "edge" ? (
              <CloudVoiceSelector
                key={tts.provider}
                value={voiceValue}
                options={cloudVoiceOptions}
                customLabel={t("textToSpeech.voice.custom")}
                customPlaceholder={t("textToSpeech.voice.searchPlaceholder")}
                maxLength={
                  tts.provider === "soniox" ? SONIOX_TTS_FIELD_MAX_LENGTH : 256
                }
                disabled={savingField !== null}
                busy={voiceCatalogBusy}
                refreshLabel={t("textToSpeech.voice.refresh")}
                sourceLabel={
                  voiceCatalog?.provider === tts.provider
                    ? voiceCatalog.source === "live"
                      ? t("textToSpeech.voice.liveCatalog")
                      : t("textToSpeech.voice.builtinCatalog")
                    : undefined
                }
                warning={
                  tts.provider === "edge"
                    ? t("textToSpeech.edge.experimentalWarning")
                    : tts.provider === "openai"
                      ? t("textToSpeech.voice.openAiCatalogWarning")
                      : tts.provider === "soniox" &&
                          voiceCatalog?.provider === "soniox"
                        ? t("textToSpeech.voice.sonioxCatalogWarning")
                        : voiceCatalog?.provider === tts.provider
                          ? voiceCatalog.warning
                          : undefined
                }
                error={voiceCatalogError}
                onRefresh={
                  tts.provider === "openai"
                    ? undefined
                    : () => void refreshVoiceCatalog()
                }
                onChange={(value) => {
                  if (tts.provider === "soniox") {
                    void updateTts({ soniox_voice: value }, "soniox_voice");
                  } else if (tts.provider === "deepgram") {
                    void updateTts({ deepgram_model: value }, "deepgram_model");
                  } else if (tts.provider === "openai") {
                    void updateTts({ openai_voice: value }, "openai_voice");
                  } else if (tts.provider === "murf") {
                    void updateTts(
                      {
                        murf_voice: value,
                        murf_style: retainedMurfStyle(
                          value,
                          tts.murf_language,
                        ),
                      },
                      "murf_voice",
                    );
                  } else if (tts.provider === "elevenlabs") {
                    void updateTts(
                      { elevenlabs_voice: value },
                      "elevenlabs_voice",
                    );
                  } else if (tts.provider === "cartesia") {
                    void updateTts(
                      { cartesia_voice: value },
                      "cartesia_voice",
                    );
                  } else {
                    void updateTts(
                      {
                        edge_voice: value,
                        edge_voice_language: value
                          .split("-")
                          .slice(0, 2)
                          .join("-"),
                      },
                      "edge_voice",
                    );
                  }
                }}
              />
            ) : tts.provider === "openai_compatible" ? (
              <Input
                className="w-full md:w-72"
                value={tts.openai_compatible_voice}
                placeholder="alloy"
                onChange={(event) =>
                  void updateTts(
                    { openai_compatible_voice: event.target.value },
                    "openai_compatible_voice",
                  )
                }
              />
            ) : null}
          </SettingContainer>
          {tts.provider === "soniox" && (
            <SettingContainer
              grouped
              title={t("textToSpeech.voice.languageTitle")}
              description={t("textToSpeech.voice.languageDescription")}
            >
              <Select
                className="w-full md:w-72"
                value={tts.soniox_language}
                options={SONIOX_LANGUAGE_SELECT_OPTIONS}
                placeholder="en"
                isClearable={false}
                isCreatable
                formatCreateLabel={(input) =>
                  `${t("textToSpeech.voice.custom")}: ${input}`
                }
                onCreateOption={(input) => {
                  const value = input
                    .trim()
                    .slice(0, SONIOX_TTS_FIELD_MAX_LENGTH);
                  if (value) {
                    void updateTts(
                      { soniox_language: value },
                      "soniox_language",
                    );
                  }
                }}
                onChange={(value) => {
                  if (value) {
                    void updateTts(
                      { soniox_language: value },
                      "soniox_language",
                    );
                  }
                }}
                disabled={savingField !== null}
              />
            </SettingContainer>
          )}
          {(tts.provider === "murf" ||
            (tts.provider === "elevenlabs" &&
              tts.elevenlabs_model !== "eleven_multilingual_v2") ||
            tts.provider === "cartesia") && (
            <SettingContainer
              grouped
              title={t("textToSpeech.voice.languageTitle")}
              description={t("textToSpeech.voice.languageDescription")}
            >
              <Select
                className="w-full md:w-72"
                value={providerLanguageValue}
                options={providerLanguageOptions}
                placeholder={TTS_PROVIDER_DEFAULTS[tts.provider].language}
                isClearable={false}
                isCreatable
                formatCreateLabel={(input) =>
                  `${t("textToSpeech.voice.custom")}: ${input}`
                }
                onCreateOption={(input) => {
                  const value = input.trim().slice(0, 32);
                  if (!value) return;
                  if (tts.provider === "murf") {
                    void updateTts(
                      {
                        murf_language: value,
                        murf_style: retainedMurfStyle(tts.murf_voice, value),
                      },
                      "murf_language",
                    );
                  } else if (tts.provider === "elevenlabs") {
                    void updateTts(
                      { elevenlabs_language: value },
                      "elevenlabs_language",
                    );
                  } else {
                    void updateTts(
                      { cartesia_language: value },
                      "cartesia_language",
                    );
                  }
                }}
                onChange={(value) => {
                  if (!value) return;
                  if (tts.provider === "murf") {
                    void updateTts(
                      {
                        murf_language: value,
                        murf_style: retainedMurfStyle(tts.murf_voice, value),
                      },
                      "murf_language",
                    );
                  } else if (tts.provider === "elevenlabs") {
                    void updateTts(
                      { elevenlabs_language: value },
                      "elevenlabs_language",
                    );
                  } else {
                    void updateTts(
                      { cartesia_language: value },
                      "cartesia_language",
                    );
                  }
                }}
                disabled={savingField !== null}
              />
            </SettingContainer>
          )}
          {tts.provider === "local_qwen" && (
            <SettingContainer
              grouped
              title={t("textToSpeech.voice.languageTitle")}
              description={t("textToSpeech.local.languageDescription")}
            >
              <Select
                className="w-full md:w-72"
                value={tts.local_qwen_language}
                options={LOCAL_QWEN_LANGUAGES}
                onChange={(value) => {
                  if (value) {
                    void updateTts(
                      { local_qwen_language: value },
                      "local_qwen_language",
                    );
                  }
                }}
                isClearable={false}
                disabled={savingField !== null}
              />
            </SettingContainer>
          )}
          {tts.provider === "local_kokoro" && (
            <SettingContainer
              grouped
              title={t("textToSpeech.voice.languageTitle")}
              description={t("textToSpeech.local.languageDescription")}
            >
              <Select
                className="w-full md:w-72"
                value={tts.local_kokoro_language}
                options={LOCAL_KOKORO_LANGUAGES}
                onChange={(value) => {
                  if (value) {
                    void updateTts(
                      {
                        local_kokoro_language: value,
                        local_kokoro_voice:
                          value === "Chinese" ? "zf_001" : "af_maple",
                      },
                      "local_kokoro_language",
                    );
                  }
                }}
                isClearable={false}
                disabled={savingField !== null}
              />
            </SettingContainer>
          )}
          {tts.provider !== "deepgram" &&
            tts.provider !== "local_qwen" &&
            tts.provider !== "local_kokoro" &&
            tts.provider !== "edge" &&
            tts.provider !== "windows" && (
              <SettingContainer
                grouped
                title={t("textToSpeech.voice.modelTitle")}
                description={t("textToSpeech.voice.modelDescription")}
              >
                {tts.provider === "openai_compatible" ? (
                  <Input
                    className="w-full md:w-72"
                    value={tts.openai_compatible_model}
                    placeholder="tts-1"
                    onChange={(event) =>
                      void updateTts(
                        { openai_compatible_model: event.target.value },
                        "openai_compatible_model",
                      )
                    }
                  />
                ) : (
                <Select
                  className="w-full md:w-72"
                  value={modelValue}
                  options={
                    tts.provider === "soniox"
                      ? SONIOX_MODEL_SELECT_OPTIONS
                      : tts.provider === "openai"
                        ? OPENAI_MODEL_SELECT_OPTIONS
                        : tts.provider === "murf"
                          ? MURF_MODEL_SELECT_OPTIONS
                          : tts.provider === "elevenlabs"
                            ? ELEVENLABS_MODEL_SELECT_OPTIONS
                            : CARTESIA_MODEL_SELECT_OPTIONS
                  }
                  isClearable={false}
                  {...(tts.provider === "soniox" || tts.provider === "openai"
                    ? {
                        isCreatable: true as const,
                        formatCreateLabel: (input: string) =>
                          `${t("textToSpeech.voice.custom")}: ${input}`,
                        onCreateOption: (input: string) => {
                          const value = input
                            .trim()
                            .slice(
                              0,
                              tts.provider === "soniox"
                                ? SONIOX_TTS_FIELD_MAX_LENGTH
                                : 256,
                            );
                          if (!value) return;
                          void updateTts(
                            tts.provider === "soniox"
                              ? { soniox_model: value }
                              : { openai_model: value },
                            tts.provider === "soniox"
                              ? "soniox_model"
                              : "openai_model",
                          );
                        },
                      }
                    : { isCreatable: false as const })}
                  onChange={(value) => {
                    if (!value) return;
                    if (tts.provider === "soniox") {
                      void updateTts({ soniox_model: value }, "soniox_model");
                    } else if (tts.provider === "openai") {
                      void updateTts({ openai_model: value }, "openai_model");
                    } else if (tts.provider === "murf") {
                      void updateTts({ murf_model: value }, "murf_model");
                    } else if (tts.provider === "elevenlabs") {
                      void updateTts(
                        { elevenlabs_model: value },
                        "elevenlabs_model",
                      );
                    } else {
                      void updateTts(
                        { cartesia_model: value },
                        "cartesia_model",
                      );
                    }
                  }}
                  disabled={savingField !== null}
                />
                )}
              </SettingContainer>
            )}
          {tts.provider !== "murf" && !elevenV3Selected && (
              <SettingContainer
                grouped
                title={t("textToSpeech.voice.speedTitle")}
                description={t("textToSpeech.voice.speedDescription", {
                  minimum: speedMinimum,
                  maximum: speedMaximum,
                })}
              >
                <CommittedNumberInput
                  className="w-28"
                  min={speedMinimum}
                  max={speedMaximum}
                  step={0.05}
                  value={tts.speed}
                  onCommit={(speed) =>
                    void updateTts({ speed }, "speed")
                  }
                />
              </SettingContainer>
            )}
          {tts.provider === "murf" && (
            <>
              <SettingContainer
                grouped
                title={t("textToSpeech.voice.murfRateTitle")}
                description={t("textToSpeech.voice.murfRateDescription")}
              >
                <CommittedNumberInput
                  className="w-28"
                  min={-50}
                  max={50}
                  step={1}
                  value={tts.murf_rate}
                  onCommit={(murf_rate) =>
                    void updateTts({ murf_rate }, "murf_rate")
                  }
                />
              </SettingContainer>
              <SettingContainer
                grouped
                title={t("textToSpeech.voice.murfPitchTitle")}
                description={t("textToSpeech.voice.murfPitchDescription")}
              >
                <CommittedNumberInput
                  className="w-28"
                  min={-50}
                  max={50}
                  step={1}
                  value={tts.murf_pitch}
                  onCommit={(murf_pitch) =>
                    void updateTts({ murf_pitch }, "murf_pitch")
                  }
                />
              </SettingContainer>
              {tts.murf_model === "gen2" && (
                <SettingContainer
                  grouped
                  title={t("textToSpeech.voice.murfVariationTitle")}
                  description={t(
                    "textToSpeech.voice.murfVariationDescription",
                  )}
                >
                  <CommittedNumberInput
                    className="w-28"
                    min={0}
                    max={5}
                    step={1}
                    value={tts.murf_variation}
                    onCommit={(murf_variation) =>
                      void updateTts({ murf_variation }, "murf_variation")
                    }
                  />
                </SettingContainer>
              )}
              <SettingContainer
                grouped
                title={t("textToSpeech.voice.murfStyleTitle")}
                description={t("textToSpeech.voice.murfStyleDescription")}
              >
                <Select
                  className="w-full md:w-72"
                  value={tts.murf_style ?? ""}
                  options={murfStyleOptions}
                  isClearable={false}
                  onChange={(value) =>
                    void updateTts(
                      { murf_style: value ? value : null },
                      "murf_style",
                    )
                  }
                  disabled={savingField !== null}
                />
              </SettingContainer>
            </>
          )}
          {tts.provider === "elevenlabs" && (
            <>
              {elevenV3Selected && (
                <div className="bg-sky-400/10 px-6 py-3 text-sm text-sky-100">
                  {t("textToSpeech.voice.elevenV3ControlsNote")}
                </div>
              )}
              <SettingContainer
                grouped
                title={t("textToSpeech.voice.elevenStabilityTitle")}
                description={t(
                  "textToSpeech.voice.elevenStabilityDescription",
                )}
              >
                <CommittedNumberInput
                  className="w-28"
                  min={0}
                  max={1}
                  step={0.05}
                  value={tts.elevenlabs_stability}
                  onCommit={(elevenlabs_stability) =>
                    void updateTts(
                      { elevenlabs_stability },
                      "elevenlabs_stability",
                    )
                  }
                />
              </SettingContainer>
              {!elevenV3Selected && (
                <SettingContainer
                  grouped
                  title={t("textToSpeech.voice.elevenSimilarityTitle")}
                  description={t(
                    "textToSpeech.voice.elevenSimilarityDescription",
                  )}
                >
                  <CommittedNumberInput
                    className="w-28"
                    min={0}
                    max={1}
                    step={0.05}
                    value={tts.elevenlabs_similarity_boost}
                    onCommit={(elevenlabs_similarity_boost) =>
                      void updateTts(
                        { elevenlabs_similarity_boost },
                        "elevenlabs_similarity_boost",
                      )
                    }
                  />
                </SettingContainer>
              )}
              <SettingContainer
                grouped
                title={t("textToSpeech.voice.elevenStyleTitle")}
                description={t("textToSpeech.voice.elevenStyleDescription")}
              >
                <CommittedNumberInput
                  className="w-28"
                  min={0}
                  max={1}
                  step={0.05}
                  value={tts.elevenlabs_style}
                  onCommit={(elevenlabs_style) =>
                    void updateTts(
                      { elevenlabs_style },
                      "elevenlabs_style",
                    )
                  }
                />
              </SettingContainer>
              {!elevenV3Selected && (
                <ToggleSwitch
                  grouped
                  checked={tts.elevenlabs_use_speaker_boost}
                  onChange={(elevenlabs_use_speaker_boost) =>
                    void updateTts(
                      { elevenlabs_use_speaker_boost },
                      "elevenlabs_use_speaker_boost",
                    )
                  }
                  isUpdating={
                    savingField === "elevenlabs_use_speaker_boost"
                  }
                  label={t("textToSpeech.voice.elevenSpeakerBoostTitle")}
                  description={t(
                    "textToSpeech.voice.elevenSpeakerBoostDescription",
                  )}
                  descriptionMode="inline"
                />
              )}
              <SettingContainer
                grouped
                title={t("textToSpeech.voice.elevenNormalizationTitle")}
                description={t(
                  "textToSpeech.voice.elevenNormalizationDescription",
                )}
              >
                <Select
                  className="w-full md:w-72"
                  value={tts.elevenlabs_apply_text_normalization}
                  options={[
                    {
                      value: "auto",
                      label: t("textToSpeech.voice.normalizationAuto"),
                    },
                    {
                      value: "on",
                      label: t("textToSpeech.voice.normalizationOn"),
                    },
                    {
                      value: "off",
                      label: t("textToSpeech.voice.normalizationOff"),
                    },
                  ]}
                  isClearable={false}
                  onChange={(value) => {
                    if (value) {
                      void updateTts(
                        {
                          elevenlabs_apply_text_normalization:
                            value as ElevenLabsTextNormalization,
                        },
                        "elevenlabs_apply_text_normalization",
                      );
                    }
                  }}
                  disabled={savingField !== null}
                />
              </SettingContainer>
            </>
          )}
          {tts.provider === "cartesia" && (
            <>
              <div className="bg-sky-400/10 px-6 py-3 text-sm text-sky-100">
                {t("textToSpeech.voice.cartesiaSonic35ControlsNote")}
              </div>
              <SettingContainer
                grouped
                title={t("textToSpeech.voice.cartesiaVolumeTitle")}
                description={t(
                  "textToSpeech.voice.cartesiaVolumeDescription",
                )}
              >
                <CommittedNumberInput
                  className="w-28"
                  min={0.5}
                  max={2}
                  step={0.05}
                  value={tts.cartesia_volume}
                  onCommit={(cartesia_volume) =>
                    void updateTts(
                      { cartesia_volume },
                      "cartesia_volume",
                    )
                  }
                />
              </SettingContainer>
              <SettingContainer
                grouped
                title={t("textToSpeech.voice.cartesiaEmotionTitle")}
                description={t(
                  "textToSpeech.voice.cartesiaEmotionDescription",
                )}
              >
                <Select
                  className="w-full md:w-72"
                  value={tts.cartesia_emotion ?? ""}
                  options={[
                    {
                      value: "",
                      label: t("textToSpeech.voice.emotionAutomatic"),
                    },
                    ...[
                      "neutral",
                      "calm",
                      "angry",
                      "content",
                      "sad",
                      "scared",
                    ].map((emotion) => ({
                      value: emotion,
                      label: t(`textToSpeech.voice.emotion.${emotion}`),
                    })),
                  ]}
                  isClearable={false}
                  onChange={(value) =>
                    void updateTts(
                      { cartesia_emotion: value ? value : null },
                      "cartesia_emotion",
                    )
                  }
                  disabled={savingField !== null}
                />
              </SettingContainer>
            </>
          )}
          {mode === "interactive" && (
            <SettingContainer
              grouped
              layout="stacked"
              title={t("textToSpeech.voice.previewTitle")}
              description={t("textToSpeech.voice.previewDescription")}
              descriptionMode="inline"
            >
              <Textarea
                className="min-h-[84px] w-full"
                variant="compact"
                rows={3}
                maxLength={500}
                value={voicePreviewText}
                disabled={voicePreviewBusy}
                aria-describedby="tts-voice-preview-note"
                onChange={(event) => {
                  setVoicePreviewText(event.target.value);
                  setVoicePreviewError(null);
                }}
              />
              <div className="mt-3 flex flex-wrap items-center gap-3">
                <Button
                  variant="secondary"
                  disabled={
                    voicePreviewStopping ||
                    (!voicePreviewBusy && !voicePreviewText.trim())
                  }
                  onClick={() =>
                    void (voicePreviewBusy
                      ? cancelVoicePreview()
                      : runVoicePreview())
                  }
                >
                  {voicePreviewBusy ? (
                    <>
                      {voicePreviewStopping ? (
                        <Loader2 className="mr-2 inline h-4 w-4 animate-spin" />
                      ) : (
                        <Square className="mr-2 inline h-4 w-4" />
                      )}
                      {t(
                        voicePreviewStopping
                          ? "textToSpeech.voice.previewStopping"
                          : "textToSpeech.voice.previewCancel",
                      )}
                    </>
                  ) : (
                    <>
                      <Volume2 className="mr-2 inline h-4 w-4" />
                      {t("textToSpeech.voice.previewPlay")}
                    </>
                  )}
                </Button>
                <p
                  id="tts-voice-preview-note"
                  className="min-w-64 flex-1 text-xs leading-relaxed text-text/60"
                >
                  {t("textToSpeech.voice.previewCostDisclosure")}
                </p>
              </div>
              {voicePreviewError && (
                <p
                  className="mt-2 text-xs leading-relaxed text-red-300"
                  role="alert"
                >
                  {voicePreviewError}
                </p>
              )}
            </SettingContainer>
          )}
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
          {(tts.provider === "local_qwen" ||
            tts.provider === "local_kokoro") && (
            <div
              role="status"
              className="mx-6 my-4 rounded-lg border border-amber-400/40 bg-amber-400/10 px-4 py-3 text-xs leading-relaxed text-amber-100"
            >
              {t("textToSpeech.prompts.localUnsupported")}
            </div>
          )}
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
                  <Select
                    className="min-w-64"
                    value={tts.selected_prompt_id}
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
                    onChange={(value) => void selectPromptPreset(value ?? "")}
                    isClearable={false}
                    disabled={savingField !== null}
                  />
                  <ActionTooltip
                    content={
                      savingField !== null
                        ? t("textToSpeech.disabledWhileSaving")
                        : !tts.selected_prompt_id
                          ? t("textToSpeech.prompts.selectToDelete")
                          : null
                    }
                  >
                    <Button
                      variant="danger"
                      disabled={!tts.selected_prompt_id || savingField !== null}
                      onClick={() =>
                        setDeleteConfirmation({
                          kind: "promptPreset",
                          name:
                            tts.prompt_presets.find(
                              (preset) => preset.id === tts.selected_prompt_id,
                            )?.name ?? "",
                        })
                      }
                    >
                      <Trash2 className="mr-2 inline h-4 w-4" />
                      {t("textToSpeech.prompts.deletePreset")}
                    </Button>
                  </ActionTooltip>
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
                  maxLength={OPENAI_TTS_INSTRUCTIONS_MAX_LENGTH}
                  placeholder={t(
                    "textToSpeech.prompts.instructionsPlaceholder",
                  )}
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
                <p className="mt-1 text-right text-xs text-text/60">
                  {tts.openai_instructions.length}/
                  {OPENAI_TTS_INSTRUCTIONS_MAX_LENGTH}
                </p>
                <div className="mt-3 flex flex-wrap gap-2">
                  <Input
                    className="min-w-64 flex-1"
                    value={promptPresetName}
                    placeholder={t("textToSpeech.prompts.namePlaceholder")}
                    onChange={(event) =>
                      setPromptPresetName(event.target.value)
                    }
                  />
                  <ActionTooltip
                    content={
                      savingField !== null
                        ? t("textToSpeech.disabledWhileSaving")
                        : !promptPresetName.trim()
                          ? t("textToSpeech.prompts.enterName")
                          : !tts.openai_instructions.trim()
                            ? t("textToSpeech.prompts.enterInstructions")
                            : null
                    }
                  >
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
                  </ActionTooltip>
                </div>
              </SettingContainer>
              <div className="px-6 py-4 text-xs text-amber-200/90">
                {t("textToSpeech.prompts.aiDisclosure")}
              </div>
            </>
          )}
        </SettingsGroup>

        <SettingsGroup
          title={t("textToSpeech.playback.title")}
          description={t("textToSpeech.playback.description")}
          help={
            <TtsHelpDisclosure
              summary={t("textToSpeech.help.playbackSummary")}
              items={[
                {
                  term: t("textToSpeech.help.pitch"),
                  description: t("textToSpeech.help.pitchDescription"),
                },
                {
                  term: t("textToSpeech.help.effects"),
                  description: t("textToSpeech.help.effectsDescription"),
                },
              ]}
              links={aivoRelayTtsGuideLink}
            />
          }
        >
          <SettingContainer
            grouped
            title={t("textToSpeech.overlay.pitchTitle")}
            description={t("textToSpeech.overlay.pitchDescription")}
          >
            <CommittedNumberInput
              className="w-28"
              min={0.5}
              max={2}
              step={0.05}
              value={tts.playback_pitch}
              onCommit={(playback_pitch) =>
                void updateTts(
                  { playback_pitch },
                  "playback_pitch",
                )
              }
            />
          </SettingContainer>
          <SettingContainer
            grouped
            title={t("textToSpeech.overlay.effectTitle")}
            description={t("textToSpeech.overlay.effectDescription")}
          >
            <Select
              className="w-full md:w-72"
              value={tts.playback_effect}
              options={[
                {
                  value: "none",
                  label: t("textToSpeech.overlay.effectNone"),
                },
                {
                  value: "radio",
                  label: t("textToSpeech.overlay.effectRadio"),
                },
                {
                  value: "retro",
                  label: t("textToSpeech.overlay.effectRetro"),
                },
              ]}
              onChange={(value) => {
                if (value) {
                  void updateTts(
                    { playback_effect: value as TtsPlaybackEffect },
                    "playback_effect",
                  );
                }
              }}
              isClearable={false}
              disabled={savingField !== null}
            />
          </SettingContainer>
        </SettingsGroup>

        {mode === "interactive" && (
          <SettingsGroup
            title={t("textToSpeech.overlay.title")}
            description={t("textToSpeech.overlay.description")}
            help={
              <TtsHelpDisclosure
                summary={t("textToSpeech.help.overlaySummary")}
                items={[
                  {
                    term: t("textToSpeech.help.autoplay"),
                    description: t("textToSpeech.help.autoplayDescription"),
                  },
                  {
                    term: t("textToSpeech.help.autoHide"),
                    description: t("textToSpeech.help.autoHideDescription"),
                  },
                  {
                    term: t("textToSpeech.help.hotkeys"),
                    description: t("textToSpeech.help.hotkeysDescription"),
                  },
                ]}
                links={aivoRelayTtsGuideLink}
              />
            }
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
            <ToggleSwitch
              grouped
              checked={tts.overlay_auto_hide_enabled}
              onChange={(overlay_auto_hide_enabled) =>
                void updateTts(
                  { overlay_auto_hide_enabled },
                  "overlay_auto_hide_enabled",
                )
              }
              isUpdating={savingField === "overlay_auto_hide_enabled"}
              label={t("textToSpeech.overlay.autoHideLabel")}
              description={t("textToSpeech.overlay.autoHideDescription")}
              descriptionMode="inline"
            />
            <SettingContainer
              grouped
              title={t("textToSpeech.overlay.autoHideDelayTitle")}
              description={t("textToSpeech.overlay.autoHideDelayDescription")}
            >
              <CommittedNumberInput
                className="w-28"
                min={1}
                max={300}
                step={1}
                value={tts.overlay_auto_hide_delay_seconds}
                disabled={
                  !tts.overlay_auto_hide_enabled || savingField !== null
                }
                onCommit={(overlay_auto_hide_delay_seconds) =>
                  void updateTts(
                    { overlay_auto_hide_delay_seconds },
                    "overlay_auto_hide_delay_seconds",
                  )
                }
              />
            </SettingContainer>
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
                  void updateTts(
                    { play_pause_hotkey: value },
                    "play_pause_hotkey",
                  );
                }}
                onCancel={() => setCapturingHotkey(null)}
                onClear={() =>
                  void updateTts({ play_pause_hotkey: "" }, "play_pause_hotkey")
                }
                osType={hotkeyOsType}
              />
            </SettingContainer>
            <ToggleSwitch
              grouped
              checked={tts.play_history_when_overlay_closed}
              disabled={playHistoryFallbackUnavailable}
              onChange={(play_history_when_overlay_closed) =>
                void updateTts(
                  { play_history_when_overlay_closed },
                  "play_history_when_overlay_closed",
                )
              }
              isUpdating={savingField === "play_history_when_overlay_closed"}
              label={t("textToSpeech.overlay.historyFallbackLabel")}
              description={playHistoryFallbackDescription}
              descriptionMode="inline"
            />
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
                onClear={() =>
                  void updateTts({ stop_hotkey: "" }, "stop_hotkey")
                }
                osType={hotkeyOsType}
              />
            </SettingContainer>
          </SettingsGroup>
        )}
      </>

      {mode === "files" && (
        <>
          <TtsBatchConversion
            outputFormat={outputFormat}
            mp3Bitrate={mp3Bitrate}
            flushPendingSettingsWrites={flushPendingSettingsWrites}
          />

          <TtsFolderAutomation
            tts={tts}
            savingField={savingField}
            updateTts={updateTts}
          />
        </>
      )}

      <TtsAiCleanup
        mode={mode}
        value={tts.llm_preprocessing}
        providers={llmProviders}
        saving={savingField?.startsWith("llm_preprocessing") ?? false}
        flushPendingSettingsWrites={flushPendingSettingsWrites}
        onChange={updateTtsLlmPreprocessing}
      />

      <SettingsGroup
        title={t("textToSpeech.preprocessing.title")}
        description={t("textToSpeech.preprocessing.description")}
        help={
          <TtsHelpDisclosure
            summary={t("textToSpeech.help.preprocessingSummary")}
            items={[
              {
                term: t("textToSpeech.help.literalRule"),
                description: t("textToSpeech.help.literalRuleDescription"),
              },
              {
                term: t("textToSpeech.help.regexRule"),
                description: t("textToSpeech.help.regexRuleDescription"),
              },
            ]}
            links={aivoRelayTtsGuideLink}
          />
        }
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
            <ActionTooltip
              content={
                !ruleFrom
                  ? t("textToSpeech.preprocessing.enterFindText")
                  : null
              }
            >
              <Button
                size="sm"
                disabled={!ruleFrom}
                onClick={() => void addRule()}
              >
                <Plus className="mr-1 inline h-4 w-4" />
                {t("textToSpeech.preprocessing.add")}
              </Button>
            </ActionTooltip>
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
            <Tooltip content={t("textToSpeech.preprocessing.deleteRule")}>
              <button
                type="button"
                className="rounded p-2 text-[#808080] hover:bg-red-500/10 hover:text-red-300"
                onClick={() => void removeRule(rule.id)}
                aria-label={t("textToSpeech.preprocessing.deleteRule")}
              >
                <Trash2 className="h-4 w-4" />
              </button>
            </Tooltip>
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
        description={t(
          providerPublishesInputLimit(tts.provider)
            ? "textToSpeech.chunking.description"
            : "textToSpeech.help.chunkingSafetyCapSummary",
          {
            provider: providerLabel,
            limit: providerInputLimit(tts.provider, modelValue),
          },
        )}
        help={
          <TtsHelpDisclosure
            summary={t(
              providerPublishesInputLimit(tts.provider)
                ? "textToSpeech.help.chunkingSummary"
                : "textToSpeech.help.chunkingSafetyCapSummary",
              {
                provider: providerLabel,
                limit: providerInputLimit(tts.provider, modelValue),
              },
            )}
            items={[
              {
                term: t("textToSpeech.help.targetSize"),
                description: t("textToSpeech.help.targetSizeDescription"),
              },
              {
                term: t("textToSpeech.help.retries"),
                description: t("textToSpeech.help.retriesDescription"),
              },
              {
                term: t("textToSpeech.help.pauses"),
                description: t("textToSpeech.help.pausesDescription"),
              },
            ]}
            links={[...providerHelpLinks, ...aivoRelayTtsGuideLink]}
          />
        }
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
        ]
          .filter((item) =>
            mode === "files"
              ? item.key !== "interactive_target_chars"
              : item.key !== "file_target_chars",
          )
          .map((item) => (
            <SettingContainer
              key={item.key}
              grouped
              title={item.title}
              description={item.description}
            >
              <CommittedNumberInput
                className="w-28"
                min={item.min}
                max={item.max}
                step={item.step}
                value={tts[item.key]}
                onCommit={(value) =>
                  void updateTts(
                    { [item.key]: value },
                    item.key,
                  )
                }
              />
            </SettingContainer>
          ))}
      </SettingsGroup>

      {mode === "interactive" && (
        <TtsHistory
          scope="interactive"
          tts={tts}
          savingField={savingField}
          onAvailabilityChange={handleInteractiveHistoryAvailability}
          updateTts={updateTts}
          flushPendingSettingsWrites={flushPendingSettingsWrites}
        />
      )}
      {providerCapabilities.requiresApiKey && (
        <SettingsGroup
          id="tts-api-settings"
          title={t("textToSpeech.api.title")}
          description={t("textToSpeech.api.description")}
          help={
            <TtsHelpDisclosure
              summary={t(
                providerCapabilities.supportsSharedKey
                  ? "textToSpeech.help.apiSummary"
                  : "textToSpeech.help.apiSeparateOnlySummary",
                { provider: providerLabel },
              )}
              items={[
                {
                  term: t("textToSpeech.help.keySource"),
                  description: t("textToSpeech.help.keySourceDescription"),
                },
                {
                  term: t("textToSpeech.help.secureStorage"),
                  description: t("textToSpeech.help.secureStorageDescription"),
                },
              ]}
              links={[
                ...providerHelpLinks,
                ...(voiceResources.authentication
                  ? [
                      {
                        label: t("textToSpeech.help.providerAuthentication"),
                        href: voiceResources.authentication,
                      },
                    ]
                  : []),
              ]}
            />
          }
        >
          <SettingContainer
            grouped
            layout="stacked"
            title={t("textToSpeech.api.sourceTitle")}
            description={
              providerCapabilities.supportsSharedKey
                ? t("textToSpeech.api.sourceDescription")
                : t("textToSpeech.api.separateOnlyDescription")
            }
            descriptionMode="inline"
          >
            <Select
              className="max-w-md"
              value={keySource}
              options={
                providerCapabilities.supportsSharedKey
                  ? [
                      { value: "shared", label: keySourceLabel },
                      {
                        value: "separate",
                        label: t("textToSpeech.api.separateKey"),
                      },
                    ]
                  : [
                      {
                        value: "separate",
                        label: t("textToSpeech.api.separateKey"),
                      },
                    ]
              }
              onChange={(value) => {
                if (keySourceField && value) {
                  void updateTts(
                    { [keySourceField]: value as TtsKeySource },
                    keySourceField,
                  );
                }
              }}
              isClearable={false}
              disabled={
                savingField !== null || !providerCapabilities.supportsSharedKey
              }
            />
          </SettingContainer>
          {tts.provider === "openai_compatible" && (
            <>
              <SettingContainer
                grouped
                layout="stacked"
                title={t("textToSpeech.openaiCompatible.baseUrlTitle")}
                description={t("textToSpeech.openaiCompatible.baseUrlDescription")}
                descriptionMode="inline"
              >
                <Input
                  className="w-full max-w-md"
                  value={tts.openai_compatible_base_url}
                  placeholder="https://api.openai.com/v1"
                  onChange={(event) =>
                    void updateTts(
                      { openai_compatible_base_url: event.target.value },
                      "openai_compatible_base_url",
                    )
                  }
                />
              </SettingContainer>
              <SettingContainer
                grouped
                layout="horizontal"
                title={t("textToSpeech.openaiCompatible.allowInsecureHttpTitle", {
                  defaultValue: "Allow Insecure HTTP",
                })}
                description={t(
                  "textToSpeech.openaiCompatible.allowInsecureHttpDescription",
                  {
                    defaultValue:
                      "Allow unencrypted http:// connections to non-loopback endpoints.",
                  },
                )}
                descriptionMode="inline"
              >
                <ToggleSwitch
                  checked={tts.openai_compatible_allow_insecure_http ?? false}
                  onChange={(checked) =>
                    void updateTts(
                      { openai_compatible_allow_insecure_http: checked },
                      "openai_compatible_allow_insecure_http",
                    )
                  }
                />
              </SettingContainer>
            </>
          )}
          {keySource === "separate" && (
            <SettingContainer
              grouped
              layout="stacked"
              title={t("textToSpeech.api.providerKeyTitle", {
                provider: providerSelectOptions.find(
                  (item) => item.value === tts.provider,
                )?.label,
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
                  maxLength={
                    tts.provider === "soniox"
                      ? SONIOX_TTS_API_KEY_MAX_LENGTH
                      : undefined
                  }
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
              {hasSeparateKey &&
                !editingKey &&
                !providerCapabilities.supportsSharedKey && (
                  <Button
                    variant="secondary"
                    disabled={voiceCatalogBusy}
                    onClick={() => void testProviderKey()}
                  >
                    {voiceCatalogBusy
                      ? t("textToSpeech.api.testingKey")
                      : t("textToSpeech.api.testKey")}
                  </Button>
                )}
            </SettingContainer>
          )}
        </SettingsGroup>
      )}
      {mode === "files" && (
        <TtsHistory
          scope="file"
          tts={tts}
          savingField={savingField}
          updateTts={updateTts}
          flushPendingSettingsWrites={flushPendingSettingsWrites}
        />
      )}
      <ConfirmationModal
        isOpen={deleteConfirmation !== null}
        onClose={() => setDeleteConfirmation(null)}
        onConfirm={confirmDelete}
        title={
          deleteConfirmation?.kind === "localModel"
            ? t("textToSpeech.local.deleteConfirmTitle")
            : deleteConfirmation?.kind === "promptPreset"
              ? t("textToSpeech.prompts.deleteConfirmTitle")
              : t("textToSpeech.synthesisPresets.deleteConfirmTitle")
        }
        message={
          deleteConfirmation?.kind === "localModel"
            ? t("textToSpeech.local.deleteConfirmMessage")
            : deleteConfirmation?.kind === "promptPreset"
              ? t("textToSpeech.prompts.deleteConfirmMessage", {
                  name: deleteConfirmation.name,
                })
              : t("textToSpeech.synthesisPresets.deleteConfirmMessage", {
                  name:
                    deleteConfirmation?.kind === "synthesisPreset"
                      ? deleteConfirmation.name
                      : "",
                })
        }
        confirmText={
          deleteConfirmation?.kind === "localModel"
            ? t("textToSpeech.local.delete")
            : deleteConfirmation?.kind === "promptPreset"
              ? t("textToSpeech.prompts.deletePreset")
              : t("textToSpeech.synthesisPresets.delete")
        }
        cancelText={t("common.cancel")}
        variant="danger"
      />
    </div>
  );
};

export default TextToSpeechSettings;
