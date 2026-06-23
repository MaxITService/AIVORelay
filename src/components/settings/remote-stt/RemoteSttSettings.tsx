import React, { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { type } from "@tauri-apps/plugin-os";
import { RefreshCw } from "lucide-react";
import { sessionToast as toast } from "@/lib/sessionToast";
import {
  commands,
  type TranscriptionProfile,
  type UpdateTranscriptionProfilePayload,
} from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import {
  REMOTE_STT_PRESETS,
  type RemoteSttPreset,
} from "../../../lib/constants/remoteSttProviders";
import { LANGUAGES } from "../../../lib/constants/languages";
import { parseAndNormalizeSonioxLanguageHints } from "../../../lib/constants/sonioxLanguages";
import { ApiKeyEditor, StoredApiKeyDisplay } from "../ApiKeyControls";
import { Button } from "../../ui/Button";
import { Input } from "../../ui/Input";
import { Select, type SelectOption } from "../../ui/Select";
import { SettingContainer } from "../../ui/SettingContainer";
import { Textarea } from "../../ui/Textarea";
import { TellMeMore } from "../../ui/TellMeMore";
import { ToggleSwitch } from "../../ui/ToggleSwitch";

interface RemoteSttSettingsProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  hideProviderSelector?: boolean;
  hideRemoteInterfaceSelector?: boolean;
}

type RemoteSttInterfaceId =
  | "groq"
  | "openai_realtime_whisper"
  | "openai_realtime_agent"
  | "openai_realtime_translate"
  | "custom";

const REALTIME_AGENT_PROMPT_TEMPLATE =
  "Additional context for speech-to-text transcription. Current language setting: ${language}. Translate to English: ${translate_to_english}. Preserve the speaker's language unless translation is enabled. Use context to create proper punctuation and fix recognition errors only when the intended words are recoverable from audio and context. If speech is not recoverable because of microphone noise, speech defects, or background noise, use [⚠️inaudible⚠️] instead of guessing. The user may provide custom words that are rare in the language; try to recognize them properly. Make sure to properly recognize names, product names, and vocabulary exactly when recognizable.";

const SONIOX_DEFAULT_REALTIME_MODEL = "stt-rt-v5";
const SONIOX_DEFAULT_ASYNC_MODEL = "stt-async-v5";

type PersistentHintInputProps = React.ComponentProps<typeof Input> & {
  hint: React.ReactNode;
  hintClassName?: string;
  inputPaddingClassName?: string;
};

const PersistentHintInput: React.FC<PersistentHintInputProps> = ({
  className = "",
  disabled,
  hint,
  hintClassName = "",
  inputPaddingClassName = "pr-28",
  ...props
}) => (
  <div className="relative">
    <Input
      {...props}
      disabled={disabled}
      className={`w-full ${inputPaddingClassName} ${className}`}
    />
    <span
      aria-hidden="true"
      className={`pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 select-none text-xs font-medium ${
        disabled ? "text-mid-gray/25" : "text-mid-gray/45"
      } ${hintClassName}`}
    >
      {hint}
    </span>
  </div>
);

const resolveRealtimeAgentPrompt = (prompt?: string | null) =>
  prompt?.trim() ? prompt : REALTIME_AGENT_PROMPT_TEMPLATE;

const applyRealtimeAgentPromptVars = (
  template: string,
  language: string,
  translateToEnglish: boolean,
) =>
  template
    .split("${language}")
    .join(language || "auto")
    .split("${translate_to_english}")
    .join(String(translateToEnglish));

const getLanguageLabel = (value: string) => {
  const normalized = value || "auto";
  const option = LANGUAGES.find((language) => language.value === normalized);
  return option ? `${option.label} (${normalized})` : normalized;
};

const buildProfileUpdatePayload = (
  profile: TranscriptionProfile,
  overrides: Partial<{
    systemPrompt: string;
    sttPromptOverrideEnabled: boolean;
  }>,
): UpdateTranscriptionProfilePayload => ({
  id: profile.id,
  name: profile.name,
  language: profile.language,
  translateToEnglish: profile.translate_to_english,
  systemPrompt: overrides.systemPrompt ?? profile.system_prompt ?? "",
  sttPromptOverrideEnabled:
    overrides.sttPromptOverrideEnabled ??
    profile.stt_prompt_override_enabled ??
    false,
  includeInCycle: profile.include_in_cycle ?? true,
  pushToTalk: profile.push_to_talk ?? true,
  previewOutputOnlyEnabled: profile.preview_output_only_enabled ?? false,
  sonioxLanguageHintsStrict: profile.soniox_language_hints_strict ?? null,
  llmSettings: {
    enabled: profile.llm_post_process_enabled ?? false,
    promptOverride: profile.llm_prompt_override ?? null,
    modelOverride: profile.llm_model_override ?? null,
  },
  sonioxContextGeneralJson: profile.soniox_context_general_json ?? "",
  sonioxContextText: profile.soniox_context_text ?? "",
  sonioxContextTerms: profile.soniox_context_terms ?? [],
});

export const RemoteSttSettings: React.FC<RemoteSttSettingsProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
  hideProviderSelector = false,
  hideRemoteInterfaceSelector = false,
}) => {
  const { t } = useTranslation();
  const isWindows = type() === "windows";
  const {
    settings,
    isUpdating,
    updateSetting,
    refreshSettings,
    setTranscriptionProvider,
    updateRemoteSttBaseUrl,
    updateRemoteSttModelId,
    updateRemoteSttDebugCapture,
    updateRemoteSttDebugMode,
  } = useSettings();

  const provider = String(settings?.transcription_provider ?? "local");
  const remoteSettings = settings?.remote_stt;
  const rawRemotePreset = String(
    (remoteSettings as any)?.provider_preset ?? "groq",
  );
  const remotePreset: RemoteSttPreset =
    rawRemotePreset in REMOTE_STT_PRESETS
      ? (rawRemotePreset as RemoteSttPreset)
      : "custom";
  const remoteAllowInsecureHttp = Boolean(
    (remoteSettings as any)?.allow_insecure_http ?? false,
  );
  const sonioxModel = (settings as any)?.soniox_model ?? SONIOX_DEFAULT_REALTIME_MODEL;
  const sonioxTimeout = Number((settings as any)?.soniox_timeout_seconds ?? 30);
  const sonioxLiveEnabled = Boolean(
    (settings as any)?.soniox_live_enabled ?? true,
  );
  const sonioxOptimizeDeliveryPreconnectEnabled = Boolean(
    (settings as any)?.soniox_optimize_delivery_preconnect_enabled ?? false,
  );
  const sonioxLanguageHints = ((settings as any)?.soniox_language_hints ??
    ["en"]) as string[];
  const sonioxUseProfileLanguageHintOnly = Boolean(
    (settings as any)?.soniox_use_profile_language_hint_only ?? false,
  );
  const sonioxLanguageHintsStrict = Boolean(
    (settings as any)?.soniox_language_hints_strict ?? false,
  );
  const sonioxEnableEndpointDetection = Boolean(
    (settings as any)?.soniox_enable_endpoint_detection ?? true,
  );
  const sonioxMaxEndpointDelayMs = Number(
    (settings as any)?.soniox_max_endpoint_delay_ms ?? 2000,
  );
  const sonioxEndpointSensitivity = Number(
    (settings as any)?.soniox_endpoint_sensitivity ?? 0,
  );
  const sonioxEnableLanguageIdentification = Boolean(
    (settings as any)?.soniox_enable_language_identification ?? true,
  );
  const sonioxEnableSpeakerDiarization = Boolean(
    (settings as any)?.soniox_enable_speaker_diarization ?? true,
  );
  const sonioxKeepaliveSeconds = Number(
    (settings as any)?.soniox_keepalive_interval_seconds ?? 10,
  );
  const sonioxLiveFinalizeTimeoutMs = Number(
    (settings as any)?.soniox_live_finalize_timeout_ms ?? 500,
  );
  const sonioxLiveInstantStop = Boolean(
    (settings as any)?.soniox_live_instant_stop ?? false,
  );
  const deepgramModel = (settings as any)?.deepgram_model ?? "nova-3";
  const deepgramTimeout = Number(
    (settings as any)?.deepgram_timeout_seconds ?? 3600,
  );
  const deepgramLiveEnabled = Boolean(
    (settings as any)?.deepgram_live_enabled ?? true,
  );
  const deepgramKeepaliveSeconds = Number(
    (settings as any)?.deepgram_keepalive_interval_seconds ?? 5,
  );
  const deepgramLiveFinalizeTimeoutMs = Number(
    (settings as any)?.deepgram_live_finalize_timeout_ms ?? 1200,
  );
  const deepgramLiveInstantStop = Boolean(
    (settings as any)?.deepgram_live_instant_stop ?? false,
  );
  const deepgramInterimResults = Boolean(
    (settings as any)?.deepgram_interim_results ?? true,
  );
  const deepgramSmartFormat = Boolean(
    (settings as any)?.deepgram_smart_format ?? true,
  );
  const deepgramEndpointingEnabled = Boolean(
    (settings as any)?.deepgram_endpointing_enabled ?? true,
  );
  const deepgramEndpointingMs = Number(
    (settings as any)?.deepgram_endpointing_ms ?? 400,
  );
  const openAiRealtimeWhisperDelay = String(
    (settings as any)?.openai_realtime_whisper_delay ?? "low",
  );
  const openAiRealtimeWhisperFlattenEnabled = Boolean(
    (settings as any)?.openai_realtime_whisper_flatten_enabled ?? false,
  );
  const isRemoteOpenAiProvider = provider === "remote_openai_compatible";
  const isSonioxProvider = provider === "remote_soniox";
  const isDeepgramProvider = provider === "remote_deepgram";
  const isCloudProvider =
    isRemoteOpenAiProvider || isSonioxProvider || isDeepgramProvider;
  const trimmedSonioxModel = sonioxModel.trim();
  const isKnownSonioxPreset =
    trimmedSonioxModel === SONIOX_DEFAULT_REALTIME_MODEL ||
    trimmedSonioxModel === SONIOX_DEFAULT_ASYNC_MODEL;
  const derivedSonioxModelMode = isKnownSonioxPreset
    ? trimmedSonioxModel
    : "custom";
  const isSonioxRealtimeModel = trimmedSonioxModel.startsWith("stt-rt");
  const isSonioxRealtimeV5Model =
    trimmedSonioxModel === SONIOX_DEFAULT_REALTIME_MODEL;
  const showSonioxRealtimeV5Upgrade =
    isSonioxRealtimeModel && !isSonioxRealtimeV5Model;
  const isSonioxAsyncModel = trimmedSonioxModel.startsWith("stt-async");
  const effectiveRemoteBaseUrl =
    remotePreset === "custom"
      ? remoteSettings?.base_url ?? ""
      : (REMOTE_STT_PRESETS[remotePreset]?.baseUrl ??
          remoteSettings?.base_url ??
          "");
  const currentRemoteInterface: RemoteSttInterfaceId =
    remotePreset === "groq"
      ? "groq"
      : remotePreset === "custom"
        ? "custom"
        : (remoteSettings?.model_id ?? "") === "gpt-realtime-whisper"
          ? "openai_realtime_whisper"
        : (remoteSettings?.model_id ?? "") === "gpt-realtime-translate"
          ? "openai_realtime_translate"
          : "openai_realtime_agent";
  const remoteApiKeyTitle =
    currentRemoteInterface === "groq"
      ? "Groq API Key"
      : currentRemoteInterface === "custom"
        ? "Custom Remote API Key"
        : "OpenAI API Key";
  const remoteApiKeyDescription =
    currentRemoteInterface === "groq"
      ? "Stored separately for Groq in Windows Credential Manager."
      : currentRemoteInterface === "custom"
        ? "Stored separately for the Custom remote endpoint in Windows Credential Manager."
        : "Stored separately for OpenAI in Windows Credential Manager.";
  const activeProfileId = settings?.active_profile_id ?? "default";
  const activeProfile = useMemo<TranscriptionProfile | null>(() => {
    if (activeProfileId === "default") {
      return null;
    }
    return (
      (settings?.transcription_profiles ?? []).find(
        (profile) => profile.id === activeProfileId,
      ) ?? null
    );
  }, [activeProfileId, settings?.transcription_profiles]);
  const activeProfileUsesSttPrompt = Boolean(
    activeProfile?.stt_prompt_override_enabled,
  );
  const effectiveRealtimePromptModelId =
    remoteSettings?.model_id?.trim() || "gpt-realtime-2";
  const globalRealtimeAgentPrompt =
    settings?.transcription_prompts?.[effectiveRealtimePromptModelId] ?? "";
  const storedRealtimeAgentPrompt = activeProfileUsesSttPrompt
    ? activeProfile?.system_prompt ?? ""
    : globalRealtimeAgentPrompt;
  const effectiveRealtimeAgentPrompt = resolveRealtimeAgentPrompt(
    storedRealtimeAgentPrompt,
  );
  const realtimeAgentPromptSource = activeProfileUsesSttPrompt
    ? `Profile: ${activeProfile?.name ?? activeProfileId}`
    : "Global model prompt";
  const [realtimeAgentPromptDraft, setRealtimeAgentPromptDraft] = useState(
    effectiveRealtimeAgentPrompt,
  );
  const [isSavingRealtimeAgentPrompt, setIsSavingRealtimeAgentPrompt] =
    useState(false);
  const realtimeAgentPromptDirty =
    realtimeAgentPromptDraft !== effectiveRealtimeAgentPrompt;
  const effectiveRealtimeAgentLanguage =
    activeProfile?.language ?? settings?.selected_language ?? "auto";
  const effectiveRealtimeAgentTranslateToEnglish =
    activeProfile?.translate_to_english ?? Boolean(settings?.translate_to_english);
  const realtimeLanguageSettingSource = activeProfile
    ? `Active profile: ${activeProfile.name}`
    : "Global language settings";
  const realtimeTranslateOutputTarget = effectiveRealtimeAgentTranslateToEnglish
    ? "English (en)"
    : effectiveRealtimeAgentLanguage === "auto"
      ? "OS input language at recording time (Auto)"
      : effectiveRealtimeAgentLanguage === "os_input"
        ? "OS input language at recording time"
        : getLanguageLabel(effectiveRealtimeAgentLanguage);
  const realtimeAgentLanguageForPreview =
    effectiveRealtimeAgentLanguage === "os_input"
      ? "OS input language at recording time"
      : effectiveRealtimeAgentLanguage;
  const resolvedRealtimeAgentPromptPreview = useMemo(
    () =>
      applyRealtimeAgentPromptVars(
        realtimeAgentPromptDraft,
        realtimeAgentLanguageForPreview,
        effectiveRealtimeAgentTranslateToEnglish,
      ),
    [
      realtimeAgentPromptDraft,
      realtimeAgentLanguageForPreview,
      effectiveRealtimeAgentTranslateToEnglish,
    ],
  );
  const realtimeAgentInstructionPreview = useMemo(() => {
    const task = effectiveRealtimeAgentTranslateToEnglish
      ? "Translate the user's spoken audio into English."
      : "Transcribe the user's spoken audio in the original language.";
    const languageHint =
      realtimeAgentLanguageForPreview.trim() &&
      realtimeAgentLanguageForPreview !== "auto"
        ? `\nLanguage hint: ${realtimeAgentLanguageForPreview}.`
        : "";
    const promptHint = resolvedRealtimeAgentPromptPreview.trim()
      ? `\nAdditional STT instructions/context: ${resolvedRealtimeAgentPromptPreview.trim()}`
      : "";

    return (
      "You are being used as a speech-to-text engine inside AivoRelay STT application. " +
      `${task} Output ONLY the final transcript text. Do not answer the speaker, ` +
      "summarize, explain, add labels, add Markdown, or mention that you are an AI. " +
      `If a word is unclear, use [⚠️inaudible⚠️].${languageHint}${promptHint}`
    );
  }, [
    effectiveRealtimeAgentTranslateToEnglish,
    realtimeAgentLanguageForPreview,
    resolvedRealtimeAgentPromptPreview,
  ]);

  const [baseUrlInput, setBaseUrlInput] = useState(
    effectiveRemoteBaseUrl,
  );
  const [modelIdInput, setModelIdInput] = useState(
    remoteSettings?.model_id ?? "",
  );
  const [customModelId, setCustomModelId] = useState(
    remotePreset === "custom" ? (remoteSettings?.model_id ?? "") : "",
  );
  const [sonioxModelMode, setSonioxModelMode] = useState(derivedSonioxModelMode);
  const [customSonioxModelInput, setCustomSonioxModelInput] = useState<string>(
    isKnownSonioxPreset ? "" : sonioxModel,
  );
  const [sonioxTimeoutInput, setSonioxTimeoutInput] = useState(
    String(sonioxTimeout),
  );
  const [sonioxLanguageHintsInput, setSonioxLanguageHintsInput] = useState(
    sonioxLanguageHints.join(", "),
  );
  const [sonioxMaxEndpointDelayMsInput, setSonioxMaxEndpointDelayMsInput] =
    useState(String(sonioxMaxEndpointDelayMs));
  const [sonioxEndpointSensitivityInput, setSonioxEndpointSensitivityInput] =
    useState(String(sonioxEndpointSensitivity));
  const [sonioxKeepaliveSecondsInput, setSonioxKeepaliveSecondsInput] =
    useState(String(sonioxKeepaliveSeconds));
  const [sonioxLiveFinalizeTimeoutInput, setSonioxLiveFinalizeTimeoutInput] =
    useState(String(sonioxLiveFinalizeTimeoutMs));
  const [deepgramModelInput, setDeepgramModelInput] = useState(deepgramModel);
  const [deepgramTimeoutInput, setDeepgramTimeoutInput] = useState(
    String(deepgramTimeout),
  );
  const [deepgramKeepaliveSecondsInput, setDeepgramKeepaliveSecondsInput] =
    useState(String(deepgramKeepaliveSeconds));
  const [deepgramLiveFinalizeTimeoutInput, setDeepgramLiveFinalizeTimeoutInput] =
    useState(String(deepgramLiveFinalizeTimeoutMs));
  const [deepgramEndpointingMsInput, setDeepgramEndpointingMsInput] = useState(
    String(deepgramEndpointingMs),
  );

  const [apiKeyInput, setApiKeyInput] = useState("");
  const [hasApiKey, setHasApiKey] = useState(false);
  const [apiKeyLoading, setApiKeyLoading] = useState(false);
  const [isEditingKey, setIsEditingKey] = useState(false);
  const [hasKeyStatusLoaded, setHasKeyStatusLoaded] = useState(false);
  const [isUpgradingSonioxModel, setIsUpgradingSonioxModel] = useState(false);
  const sonioxRealtimeControlsEnabled =
    sonioxModelMode === "custom" || isSonioxRealtimeModel;
  const sonioxEndpointSensitivityEnabled =
    sonioxRealtimeControlsEnabled && isSonioxRealtimeV5Model;

  const [debugLines, setDebugLines] = useState<string[]>([]);
  const [connectionStatus, setConnectionStatus] = useState<
    "idle" | "checking" | "success" | "error"
  >("idle");
  const [connectionMessage, setConnectionMessage] = useState<string | null>(
    null,
  );

  const debugCapture = remoteSettings?.debug_capture ?? false;
  const debugMode = remoteSettings?.debug_mode ?? "normal";
  const debugCap = debugMode === "verbose" ? 300 : 50;

  useEffect(() => {
    setBaseUrlInput(effectiveRemoteBaseUrl);
  }, [effectiveRemoteBaseUrl]);

  useEffect(() => {
    setRealtimeAgentPromptDraft(effectiveRealtimeAgentPrompt);
  }, [effectiveRealtimeAgentPrompt]);

  useEffect(() => {
    setModelIdInput(remoteSettings?.model_id ?? "");
  }, [remoteSettings?.model_id]);

  useEffect(() => {
    if (remotePreset === "custom") {
      setCustomModelId(remoteSettings?.model_id ?? "");
    }
  }, [remotePreset, remoteSettings?.model_id]);

  useEffect(() => {
    setSonioxModelMode(derivedSonioxModelMode);
    if (!isKnownSonioxPreset) {
      setCustomSonioxModelInput(sonioxModel);
    }
  }, [sonioxModel, isKnownSonioxPreset, derivedSonioxModelMode]);

  useEffect(() => {
    setSonioxTimeoutInput(String(sonioxTimeout));
  }, [sonioxTimeout]);

  useEffect(() => {
    setSonioxLanguageHintsInput(sonioxLanguageHints.join(", "));
  }, [sonioxLanguageHints]);

  useEffect(() => {
    setSonioxMaxEndpointDelayMsInput(String(sonioxMaxEndpointDelayMs));
  }, [sonioxMaxEndpointDelayMs]);

  useEffect(() => {
    setSonioxEndpointSensitivityInput(String(sonioxEndpointSensitivity));
  }, [sonioxEndpointSensitivity]);

  useEffect(() => {
    setSonioxKeepaliveSecondsInput(String(sonioxKeepaliveSeconds));
  }, [sonioxKeepaliveSeconds]);

  useEffect(() => {
    setSonioxLiveFinalizeTimeoutInput(String(sonioxLiveFinalizeTimeoutMs));
  }, [sonioxLiveFinalizeTimeoutMs]);

  useEffect(() => {
    setDeepgramModelInput(deepgramModel);
  }, [deepgramModel]);

  useEffect(() => {
    setDeepgramTimeoutInput(String(deepgramTimeout));
  }, [deepgramTimeout]);

  useEffect(() => {
    setDeepgramKeepaliveSecondsInput(String(deepgramKeepaliveSeconds));
  }, [deepgramKeepaliveSeconds]);

  useEffect(() => {
    setDeepgramLiveFinalizeTimeoutInput(String(deepgramLiveFinalizeTimeoutMs));
  }, [deepgramLiveFinalizeTimeoutMs]);

  useEffect(() => {
    setDeepgramEndpointingMsInput(String(deepgramEndpointingMs));
  }, [deepgramEndpointingMs]);

  useEffect(() => {
    setHasKeyStatusLoaded(false);
    if (!isWindows) {
      setHasApiKey(false);
      setHasKeyStatusLoaded(true);
      return;
    }

    const loadApiKeyStatus = async () => {
      try {
        if (isDeepgramProvider) {
          const hasDeepgramKey = await invoke<boolean>("deepgram_has_api_key");
          setHasApiKey(Boolean(hasDeepgramKey));
        } else {
          const result = isSonioxProvider
            ? await commands.sonioxHasApiKey()
            : await commands.remoteSttHasApiKey();
          if (result.status === "ok") {
            setHasApiKey(result.data);
          }
        }
      } catch (error) {
        console.error("Failed to check API key status:", error);
      } finally {
        setHasKeyStatusLoaded(true);
      }
    };

    loadApiKeyStatus();
  }, [isWindows, provider, remotePreset, isSonioxProvider, isDeepgramProvider]);

  useEffect(() => {
    if (!hasKeyStatusLoaded) {
      return;
    }
    if (!hasApiKey) {
      setIsEditingKey(true);
    } else {
      setIsEditingKey(false);
    }
  }, [hasApiKey, hasKeyStatusLoaded]);

  useEffect(() => {
    setConnectionStatus("idle");
    setConnectionMessage(null);
  }, [baseUrlInput, hasApiKey, provider, remotePreset, remoteSettings?.model_id]);

  useEffect(() => {
    if (!isWindows || !isRemoteOpenAiProvider) {
      setDebugLines([]);
      return;
    }

    const loadDebugDump = async () => {
      try {
        const result = await commands.remoteSttGetDebugDump();
        if (result.status === "ok") {
          setDebugLines(result.data.slice(-debugCap));
        }
      } catch (error) {
        console.error("Failed to load remote debug log:", error);
      }
    };

    loadDebugDump();
  }, [isWindows, isRemoteOpenAiProvider, debugCap]);

  useEffect(() => {
    if (!isWindows || !isRemoteOpenAiProvider) {
      return;
    }

    const unlistenPromise = listen<string>("remote-stt-debug-line", (event) => {
      setDebugLines((prev) => {
        const next = [...prev, event.payload];
        if (next.length > debugCap) {
          return next.slice(-debugCap);
        }
        return next;
      });
    });

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [isWindows, isRemoteOpenAiProvider, debugCap]);

  useEffect(() => {
    if (!debugCapture) {
      setDebugLines([]);
    }
  }, [debugCapture]);

  const providerOptions = useMemo<SelectOption[]>(() => {
    return [
      {
        value: "local",
        label: t("settings.advanced.transcriptionProvider.options.local"),
      },
      {
        value: "remote_openai_compatible",
        label: t("settings.advanced.transcriptionProvider.options.remote"),
        isDisabled: !isWindows,
      },
      {
        value: "remote_soniox",
        label: t("settings.advanced.transcriptionProvider.options.soniox"),
        isDisabled: !isWindows,
      },
      {
        value: "remote_deepgram",
        label: t(
          "settings.advanced.transcriptionProvider.options.deepgram",
          "Remote (Deepgram Cloud API)",
        ),
        isDisabled: !isWindows,
      },
    ];
  }, [t, isWindows]);

  const deepgramModelOptions = useMemo<SelectOption[]>(() => {
    const options: SelectOption[] = [
      {
        value: "nova-3",
        label: "nova-3",
      },
      {
        value: "nova-3-general",
        label: "nova-3-general",
      },
      {
        value: "nova-3-medical",
        label: "nova-3-medical",
      },
    ];
    const current = deepgramModelInput.trim();
    if (current && !options.some((option) => option.value === current)) {
      options.push({
        value: current,
        label: current,
      });
    }
    return options;
  }, [deepgramModelInput]);

  const sonioxModelOptions = useMemo<SelectOption[]>(
    () => [
      {
        value: SONIOX_DEFAULT_REALTIME_MODEL,
        label: t(
          "settings.advanced.soniox.model.options.realtime",
          "stt-rt-v5 - Real-time",
        ),
      },
      {
        value: SONIOX_DEFAULT_ASYNC_MODEL,
        label: t(
          "settings.advanced.soniox.model.options.async",
          "stt-async-v5 - Async",
        ),
      },
      {
        value: "custom",
        label: t("settings.advanced.soniox.model.options.custom", "Custom"),
      },
    ],
    [t],
  );

  const remoteInterfaceOptions = useMemo<SelectOption[]>(
    () =>
      [
        {
          value: "groq",
          label: "Groq",
        },
        {
          value: "openai_realtime_whisper",
          label: "OpenAI gpt-realtime-whisper",
        },
        {
          value: "openai_realtime_agent",
          label: "OpenAI gpt-realtime-2 STT Hack - Not actually realtime",
        },
        {
          value: "openai_realtime_translate",
          label: "OpenAI gpt-realtime-translate",
        },
        {
          value: "custom",
          label: "Custom",
        },
      ],
    [],
  );

  const remoteInterfaceHint = useMemo(() => {
    const hints: Record<RemoteSttInterfaceId, string> = {
      groq: "Classic OpenAI-compatible /audio/transcriptions endpoint.",
      openai_realtime_whisper:
        "Native Realtime transcription model. Can stream live deltas, or flatten into post-recording STT.",
      openai_realtime_agent:
        "Voice-agent model coerced into transcript-only output. Uses global/profile STT prompts.",
      openai_realtime_translate:
        "Translation session used as STT by targeting the same language.",
      custom: "Self-hosted or non-standard OpenAI-compatible endpoint.",
    };
    return hints[currentRemoteInterface];
  }, [currentRemoteInterface]);

  const showOpenAiRealtimeNotes =
    currentRemoteInterface === "openai_realtime_agent" ||
    currentRemoteInterface === "openai_realtime_translate";

  const openAiRealtimeWhisperDelayOptions = useMemo<SelectOption[]>(
    () => [
      { value: "minimal", label: "minimal - fastest, ~1.5s chunks" },
      { value: "low", label: "low - quick, ~3s chunks" },
      { value: "medium", label: "medium - balanced, ~5s chunks" },
      { value: "high", label: "high - more context, ~7s chunks" },
      { value: "xhigh", label: "xhigh - most context, ~10s chunks" },
    ],
    [],
  );

  const groqModelOptions = useMemo<SelectOption[]>(() => {
    const options: SelectOption[] = [
      {
        value: "whisper-large-v3-turbo",
        label: "whisper-large-v3-turbo",
      },
      {
        value: "whisper-large-v3",
        label: "whisper-large-v3",
      },
    ];
    const current = modelIdInput.trim();
    if (current && !options.some((option) => option.value === current)) {
      options.push({
        value: current,
        label: current,
      });
    }
    return options;
  }, [modelIdInput]);

  const handleProviderChange = (value: string | null) => {
    if (!value) return;
    void setTranscriptionProvider(value);
  };

  const handleRemoteInterfaceSelect = async (interfaceId: RemoteSttInterfaceId) => {
    const nextPreset: RemoteSttPreset =
      interfaceId === "groq"
        ? "groq"
        : interfaceId === "custom"
          ? "custom"
          : "openai";
    const nextModel =
      interfaceId === "groq"
        ? REMOTE_STT_PRESETS.groq.defaultModel
        : interfaceId === "openai_realtime_whisper"
          ? "gpt-realtime-whisper"
        : interfaceId === "openai_realtime_translate"
          ? "gpt-realtime-translate"
          : interfaceId === "openai_realtime_agent"
            ? "gpt-realtime-2"
            : remotePreset === "custom"
              ? customModelId.trim() || modelIdInput.trim()
              : REMOTE_STT_PRESETS.custom.defaultModel;

    try {
      if (remotePreset === "custom") {
        setCustomModelId(modelIdInput.trim());
      }

      if (nextPreset !== remotePreset) {
        await invoke("change_remote_stt_provider_preset_setting", {
          preset: nextPreset,
        });
      }

      if (nextModel && nextModel !== (remoteSettings?.model_id ?? "")) {
        await updateRemoteSttModelId(nextModel);
        setModelIdInput(nextModel);
      }

      await refreshSettings();
    } catch (error) {
      toast.error(String(error));
    }
  };

  const handleRemoteHttpOverrideChange = async (enabled: boolean) => {
    try {
      await invoke("change_remote_stt_allow_insecure_http_setting", { enabled });
      await refreshSettings();
    } catch (error) {
      toast.error(String(error));
    }
  };

  const handleBaseUrlBlur = async () => {
    const trimmed = baseUrlInput.trim();
    if (trimmed !== (remoteSettings?.base_url ?? "")) {
      try {
        await updateRemoteSttBaseUrl(trimmed);
      } catch (error) {
        toast.error(String(error));
        setBaseUrlInput(remoteSettings?.base_url ?? "");
      }
    }
  };

  const handleModelIdBlur = () => {
    const trimmed = modelIdInput.trim();
    if (remotePreset === "custom") {
      setCustomModelId(trimmed);
    }
    if (trimmed !== (remoteSettings?.model_id ?? "")) {
      void updateRemoteSttModelId(trimmed);
    }
  };

  const handleGroqModelChange = (value: string | null) => {
    if (!value) return;
    setModelIdInput(value);
    if (value !== (remoteSettings?.model_id ?? "")) {
      void updateRemoteSttModelId(value);
    }
  };

  const handleOpenAiRealtimeWhisperDelayChange = (value: string | null) => {
    if (!value) return;
    void updateSetting("openai_realtime_whisper_delay" as any, value as any);
  };

  const handleSaveRealtimeAgentPrompt = async () => {
    setIsSavingRealtimeAgentPrompt(true);
    try {
      if (activeProfileUsesSttPrompt && activeProfile) {
        const result = await commands.updateTranscriptionProfile(
          buildProfileUpdatePayload(activeProfile, {
            systemPrompt: realtimeAgentPromptDraft,
            sttPromptOverrideEnabled: true,
          }),
        );
        if (result.status === "error") throw new Error(result.error);
      } else {
        const result = await commands.changeTranscriptionPromptSetting(
          effectiveRealtimePromptModelId,
          realtimeAgentPromptDraft,
        );
        if (result.status === "error") throw new Error(result.error);
      }
      await refreshSettings();
      toast.success("Realtime 2 STT prompt saved.");
    } catch (error) {
      toast.error(String(error));
    } finally {
      setIsSavingRealtimeAgentPrompt(false);
    }
  };

  const handleUseProfileRealtimeAgentPrompt = async () => {
    if (!activeProfile) return;
    setIsSavingRealtimeAgentPrompt(true);
    try {
      const result = await commands.updateTranscriptionProfile(
        buildProfileUpdatePayload(activeProfile, {
          systemPrompt: realtimeAgentPromptDraft,
          sttPromptOverrideEnabled: true,
        }),
      );
      if (result.status === "error") throw new Error(result.error);
      await refreshSettings();
      toast.success("Active profile now overrides the Realtime 2 STT prompt.");
    } catch (error) {
      toast.error(String(error));
    } finally {
      setIsSavingRealtimeAgentPrompt(false);
    }
  };

  const handleResetRealtimeAgentPrompt = async () => {
    setIsSavingRealtimeAgentPrompt(true);
    try {
      if (activeProfileUsesSttPrompt && activeProfile) {
        const result = await commands.updateTranscriptionProfile(
          buildProfileUpdatePayload(activeProfile, {
            systemPrompt: REALTIME_AGENT_PROMPT_TEMPLATE,
            sttPromptOverrideEnabled: true,
          }),
        );
        if (result.status === "error") throw new Error(result.error);
        setRealtimeAgentPromptDraft(REALTIME_AGENT_PROMPT_TEMPLATE);
        toast.success("Profile Realtime 2 STT prompt reset to default.");
      } else {
        const result = await commands.changeTranscriptionPromptSetting(
          effectiveRealtimePromptModelId,
          REALTIME_AGENT_PROMPT_TEMPLATE,
        );
        if (result.status === "error") throw new Error(result.error);
        setRealtimeAgentPromptDraft(REALTIME_AGENT_PROMPT_TEMPLATE);
        toast.success("Global Realtime 2 STT prompt reset to default.");
      }
      await refreshSettings();
    } catch (error) {
      toast.error(String(error));
    } finally {
      setIsSavingRealtimeAgentPrompt(false);
    }
  };

  const handleSonioxModelChange = (value: string | null) => {
    const nextMode = value || SONIOX_DEFAULT_REALTIME_MODEL;
    setSonioxModelMode(nextMode);
    if (nextMode === "custom") {
      setCustomSonioxModelInput((current) => current || sonioxModel);
      return;
    }

    const nextModel = nextMode;
    if (nextModel !== sonioxModel) {
      void updateSetting("soniox_model" as any, nextModel as any);
    }
  };

  const handleCustomSonioxModelBlur = () => {
    const trimmed = customSonioxModelInput.trim();
    if (!trimmed) {
      setCustomSonioxModelInput(sonioxModelMode === "custom" ? sonioxModel : "");
      return;
    }
    if (trimmed !== sonioxModel) {
      void updateSetting("soniox_model" as any, trimmed as any);
    }
  };

  const handleUpgradeSonioxRealtimeModel = async () => {
    setIsUpgradingSonioxModel(true);
    setSonioxModelMode(SONIOX_DEFAULT_REALTIME_MODEL);
    setCustomSonioxModelInput("");

    try {
      const result = await commands.changeSonioxModelSetting(
        SONIOX_DEFAULT_REALTIME_MODEL,
      );
      if (result.status === "error") {
        throw new Error(result.error);
      }
      await refreshSettings();
      toast.success(
        t(
          "settings.advanced.soniox.model.upgradeSuccess",
          "Soniox realtime model updated to stt-rt-v5.",
        ),
      );
    } catch (error) {
      setSonioxModelMode(derivedSonioxModelMode);
      if (!isKnownSonioxPreset) {
        setCustomSonioxModelInput(sonioxModel);
      }
      const message = error instanceof Error ? error.message : String(error);
      toast.error(
        t("settings.advanced.soniox.model.upgradeFailed", {
          error: message,
          defaultValue: "Failed to update Soniox realtime model: {{error}}",
        }),
      );
    } finally {
      setIsUpgradingSonioxModel(false);
    }
  };

  const handleSonioxTimeoutBlur = () => {
    const parsed = Number.parseInt(sonioxTimeoutInput, 10);
    if (Number.isNaN(parsed)) {
      setSonioxTimeoutInput(String(sonioxTimeout));
      return;
    }
    if (parsed !== sonioxTimeout) {
      void updateSetting("soniox_timeout_seconds" as any, parsed as any);
    }
  };

  const handleSonioxLanguageHintsBlur = () => {
    const parsed = parseAndNormalizeSonioxLanguageHints(sonioxLanguageHintsInput);
    const current = parseAndNormalizeSonioxLanguageHints(
      sonioxLanguageHints.join(","),
    );

    if (parsed.rejected.length > 0) {
      toast.warning(
        `Ignored unsupported Soniox language hints: ${parsed.rejected.join(", ")}`,
      );
      setSonioxLanguageHintsInput(parsed.normalized.join(", "));
    }

    if (
      JSON.stringify(parsed.normalized) !== JSON.stringify(current.normalized) ||
      current.rejected.length > 0
    ) {
      void updateSetting(
        "soniox_language_hints" as any,
        parsed.normalized as any,
      );
    }
  };

  const handleSonioxMaxEndpointDelayBlur = () => {
    const parsed = Number.parseInt(sonioxMaxEndpointDelayMsInput, 10);
    if (Number.isNaN(parsed)) {
      setSonioxMaxEndpointDelayMsInput(String(sonioxMaxEndpointDelayMs));
      return;
    }
    if (parsed !== sonioxMaxEndpointDelayMs) {
      void updateSetting("soniox_max_endpoint_delay_ms" as any, parsed as any);
    }
  };

  const handleSonioxEndpointSensitivityBlur = () => {
    const parsed = Number.parseFloat(sonioxEndpointSensitivityInput);
    if (!Number.isFinite(parsed)) {
      setSonioxEndpointSensitivityInput(String(sonioxEndpointSensitivity));
      return;
    }

    if (parsed < -1 || parsed > 1) {
      toast.warning(
        t(
          "settings.advanced.soniox.endpointSensitivity.outOfRange",
          "Soniox endpoint sensitivity must be between -1.0 and 1.0.",
        ),
      );
      setSonioxEndpointSensitivityInput(String(sonioxEndpointSensitivity));
      return;
    }

    if (parsed !== sonioxEndpointSensitivity) {
      void updateSetting("soniox_endpoint_sensitivity" as any, parsed as any);
    }
  };

  const handleSonioxKeepaliveBlur = () => {
    const parsed = Number.parseInt(sonioxKeepaliveSecondsInput, 10);
    if (Number.isNaN(parsed)) {
      setSonioxKeepaliveSecondsInput(String(sonioxKeepaliveSeconds));
      return;
    }
    if (parsed !== sonioxKeepaliveSeconds) {
      void updateSetting("soniox_keepalive_interval_seconds" as any, parsed as any);
    }
  };

  const handleSonioxLiveFinalizeTimeoutBlur = () => {
    const parsed = Number.parseInt(sonioxLiveFinalizeTimeoutInput, 10);
    if (Number.isNaN(parsed)) {
      setSonioxLiveFinalizeTimeoutInput(String(sonioxLiveFinalizeTimeoutMs));
      return;
    }
    if (parsed !== sonioxLiveFinalizeTimeoutMs) {
      void updateSetting("soniox_live_finalize_timeout_ms" as any, parsed as any);
    }
  };

  const handleResetSonioxDefaults = async () => {
    try {
      await invoke("reset_soniox_settings_to_defaults");
      await refreshSettings();
      toast.success(t("settings.advanced.soniox.reset.success"));
    } catch (error) {
      toast.error(
        t("settings.advanced.soniox.reset.failed", { error: String(error) }),
      );
    }
  };

  const handleDeepgramModelChange = (value: string | null) => {
    if (!value) return;
    setDeepgramModelInput(value);
    if (value !== deepgramModel) {
      void updateSetting("deepgram_model" as any, value as any);
    }
  };

  const handleDeepgramTimeoutBlur = () => {
    const parsed = Number.parseInt(deepgramTimeoutInput, 10);
    if (Number.isNaN(parsed)) {
      setDeepgramTimeoutInput(String(deepgramTimeout));
      return;
    }
    if (parsed !== deepgramTimeout) {
      void updateSetting("deepgram_timeout_seconds" as any, parsed as any);
    }
  };

  const handleDeepgramKeepaliveBlur = () => {
    const parsed = Number.parseInt(deepgramKeepaliveSecondsInput, 10);
    if (Number.isNaN(parsed)) {
      setDeepgramKeepaliveSecondsInput(String(deepgramKeepaliveSeconds));
      return;
    }
    if (parsed !== deepgramKeepaliveSeconds) {
      void updateSetting(
        "deepgram_keepalive_interval_seconds" as any,
        parsed as any,
      );
    }
  };

  const handleDeepgramLiveFinalizeTimeoutBlur = () => {
    const parsed = Number.parseInt(deepgramLiveFinalizeTimeoutInput, 10);
    if (Number.isNaN(parsed)) {
      setDeepgramLiveFinalizeTimeoutInput(String(deepgramLiveFinalizeTimeoutMs));
      return;
    }
    if (parsed !== deepgramLiveFinalizeTimeoutMs) {
      void updateSetting("deepgram_live_finalize_timeout_ms" as any, parsed as any);
    }
  };

  const handleDeepgramEndpointingMsBlur = () => {
    const parsed = Number.parseInt(deepgramEndpointingMsInput, 10);
    if (Number.isNaN(parsed)) {
      setDeepgramEndpointingMsInput(String(deepgramEndpointingMs));
      return;
    }
    if (parsed !== deepgramEndpointingMs) {
      void updateSetting("deepgram_endpointing_ms" as any, parsed as any);
    }
  };

  const handleResetDeepgramDefaults = async () => {
    try {
      await invoke("reset_deepgram_settings_to_defaults");
      await refreshSettings();
      toast.success(
        t("settings.advanced.deepgram.reset.success", "Deepgram settings were reset to defaults."),
      );
    } catch (error) {
      toast.error(
        t("settings.advanced.deepgram.reset.failed", {
          error: String(error),
          defaultValue: `Failed to reset Deepgram settings: ${String(error)}`,
        }),
      );
    }
  };

  const handleSaveApiKey = async () => {
    if (!apiKeyInput.trim()) return;
    setApiKeyLoading(true);
    try {
      if (isDeepgramProvider) {
        await invoke("deepgram_set_api_key", { apiKey: apiKeyInput.trim() });
        setApiKeyInput("");
        setHasApiKey(true);
        setIsEditingKey(false);
      } else {
        const result = isSonioxProvider
          ? await commands.sonioxSetApiKey(apiKeyInput.trim())
          : await commands.remoteSttSetApiKey(apiKeyInput.trim());
        if (result.status === "ok") {
          setApiKeyInput("");
          setHasApiKey(true);
          setIsEditingKey(false);
        } else {
          toast.error(result.error);
        }
      }
    } catch (error) {
      toast.error(String(error));
    } finally {
      setApiKeyLoading(false);
    }
  };

  const handleClearApiKey = async () => {
    setApiKeyLoading(true);
    try {
      if (isDeepgramProvider) {
        await invoke("deepgram_clear_api_key");
        setHasApiKey(false);
        setApiKeyInput("");
      } else {
        const result = isSonioxProvider
          ? await commands.sonioxClearApiKey()
          : await commands.remoteSttClearApiKey();
        if (result.status === "ok") {
          setHasApiKey(false);
          setApiKeyInput("");
        } else {
          toast.error(result.error);
        }
      }
    } catch (error) {
      toast.error(String(error));
    } finally {
      setApiKeyLoading(false);
    }
  };

  const handleStartReplaceKey = () => {
    setApiKeyInput("");
    setIsEditingKey(true);
  };

  const handleCancelReplaceKey = () => {
    setApiKeyInput("");
    setIsEditingKey(false);
  };

  const handleTestConnection = async () => {
    const baseUrl = baseUrlInput.trim();
    if (!baseUrl || !hasApiKey) return;

    setConnectionStatus("checking");
    setConnectionMessage(null);
    try {
      const result = await commands.remoteSttTestConnection(baseUrl);
      if (result.status === "ok") {
        setConnectionStatus("success");
        setConnectionMessage(
          t("settings.advanced.remoteStt.connection.success"),
        );
      } else {
        setConnectionStatus("error");
        setConnectionMessage(
          t("settings.advanced.remoteStt.connection.failed", {
            error: result.error,
          }),
        );
      }
    } catch (error) {
      setConnectionStatus("error");
      setConnectionMessage(
        t("settings.advanced.remoteStt.connection.failed", {
          error: String(error),
        }),
      );
    }
  };

  const handleClearDebug = async () => {
    try {
      await commands.remoteSttClearDebug();
    } catch (error) {
      console.error("Failed to clear remote debug log:", error);
    } finally {
      setDebugLines([]);
    }
  };

  const showRemoteFields =
    isWindows && isCloudProvider;
  const showOpenAiFields = isWindows && isRemoteOpenAiProvider;
  const showSonioxFields = isWindows && isSonioxProvider;
  const showDeepgramFields = isWindows && isDeepgramProvider;
  const canTestConnection =
    isRemoteOpenAiProvider &&
    baseUrlInput.trim().length > 0 &&
    !apiKeyLoading;

  return (
    <div className="space-y-2">
      {!hideProviderSelector && (
        <SettingContainer
          title={t("settings.advanced.transcriptionProvider.title")}
          description={t("settings.advanced.transcriptionProvider.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        >
          <div className="flex flex-col gap-2 min-w-[220px]">
            <Select
              value={provider}
              options={providerOptions}
              onChange={(value) => handleProviderChange(value)}
              placeholder={t("settings.advanced.transcriptionProvider.placeholder")}
              isClearable={false}
            />
            {!isWindows && (
              <p className="text-xs text-text/60">
                {t("settings.advanced.transcriptionProvider.windowsOnly")}
              </p>
            )}
          </div>
        </SettingContainer>
      )}

      {showRemoteFields && (
        <>
          {showOpenAiFields && (
            <>
              {!hideRemoteInterfaceSelector && (
                <SettingContainer
                  title="Remote STT Interface"
                  description="Choose the exact cloud interface AivoRelay should use for this remote provider."
                  descriptionMode={descriptionMode}
                  grouped={grouped}
                  layout="stacked"
                >
                  <div className="flex flex-col gap-2">
                    <Select
                      value={currentRemoteInterface}
                      options={remoteInterfaceOptions}
                      onChange={(value) =>
                        value &&
                        void handleRemoteInterfaceSelect(
                          value as RemoteSttInterfaceId,
                        )
                      }
                      isClearable={false}
                      className="w-full"
                    />
                    <p className="text-xs text-text/60">
                      {remoteInterfaceHint}
                    </p>
                  </div>
                </SettingContainer>
              )}

              <SettingContainer
                title={t("settings.advanced.remoteStt.baseUrl.title")}
                description={t("settings.advanced.remoteStt.baseUrl.description")}
                descriptionMode={descriptionMode}
                grouped={grouped}
                layout="stacked"
              >
                <Input
                  type="text"
                  value={baseUrlInput}
                  onChange={(event) => setBaseUrlInput(event.target.value)}
                  onBlur={() => void handleBaseUrlBlur()}
                  placeholder={t("settings.advanced.remoteStt.baseUrl.placeholder")}
                  className="w-full"
                  disabled={remotePreset !== "custom"}
                />
              </SettingContainer>

              {currentRemoteInterface === "openai_realtime_whisper" && (
                <>
                  <SettingContainer
                    title="Realtime Whisper Delay"
                    description="Controls how much audio context gpt-realtime-whisper gets before AivoRelay asks for a transcript chunk. Faster settings show text sooner; slower settings cut speech less often."
                    descriptionMode={descriptionMode}
                    grouped={grouped}
                    layout="stacked"
                  >
                    <Select
                      value={openAiRealtimeWhisperDelay}
                      options={openAiRealtimeWhisperDelayOptions}
                      onChange={handleOpenAiRealtimeWhisperDelayChange}
                      isClearable={false}
                      className="w-full"
                    />
                  </SettingContainer>

                  <ToggleSwitch
                    checked={openAiRealtimeWhisperFlattenEnabled}
                    onChange={(enabled) =>
                      void updateSetting(
                        "openai_realtime_whisper_flatten_enabled" as any,
                        enabled as any,
                      )
                    }
                    isUpdating={isUpdating(
                      "openai_realtime_whisper_flatten_enabled" as any,
                    )}
                    label="Flatten realtime Whisper"
                    description="Record the whole utterance first, then send it to gpt-realtime-whisper for a final transcript instead of showing live deltas."
                    descriptionMode={descriptionMode}
                    grouped={grouped}
                  />

                  <div className="mx-4 rounded-lg border border-emerald-400/20 bg-emerald-400/5 p-3 text-xs text-text/80">
                    <p className="font-medium text-text">
                      Uses OpenAI Realtime transcription sessions.
                    </p>
                    <p className="mt-1">
                      Live mode sends audio continuously, then commits the buffer
                      in small chunks so text can appear before recording stops.
                      OpenAI accepts delay as named values, not exact seconds; the
                      seconds shown above are AivoRelay's approximate chunk timing.
                      Use higher delay if words are being split or corrected too
                      aggressively.
                    </p>
                    <p className="mt-1">
                      Flatten mode keeps the same model, but behaves like
                      post-recording STT: record everything first, commit once,
                      then wait for the final transcript.
                    </p>
                    <p className="mt-1">
                      STT prompts are not sent for this model; use language and
                      delay here, then evaluate vocabulary against real audio.
                    </p>
                  </div>
                </>
              )}

              {showOpenAiRealtimeNotes && (
                <div className="mx-4 rounded-lg border border-blue-400/20 bg-blue-400/5 p-3 text-xs text-text/80">
                  {currentRemoteInterface === "openai_realtime_agent" ? (
                    <>
                      <p className="font-medium text-text">
                        How to configure: OpenAI key here, language/prompt in
                        profiles.
                      </p>
                      <p className="mt-1">
                        This mode uses the active transcription profile's
                        language, Translate to English setting, and STT prompt.
                        If no profile is active, it uses the global language,
                        global Translate to English toggle, and global/model STT
                        prompt.
                      </p>
                      <p className="mt-1">
                        The prompt is meaningful here: AivoRelay sends it as
                        Realtime instructions, so use it for vocabulary,
                        spelling, punctuation, and "output transcript only"
                        behavior.
                      </p>
                      <div className="mt-3 rounded-lg border border-blue-400/20 bg-black/20 p-3">
                        <div className="mb-2 flex flex-wrap items-center justify-between gap-2">
                          <div>
                            <p className="font-medium text-text">
                              Realtime 2 STT prompt
                            </p>
                            <p className="mt-0.5 text-[11px] text-text/60">
                              Source: {realtimeAgentPromptSource} | Language:{" "}
                              {getLanguageLabel(effectiveRealtimeAgentLanguage)}
                            </p>
                          </div>
                          <div className="flex flex-wrap gap-2">
                            {activeProfile && !activeProfileUsesSttPrompt && (
                              <Button
                                variant="ghost"
                                size="sm"
                                disabled={isSavingRealtimeAgentPrompt}
                                onClick={() =>
                                  void handleUseProfileRealtimeAgentPrompt()
                                }
                              >
                                Use Profile
                              </Button>
                            )}
                            <Button
                              variant="ghost"
                              size="sm"
                              disabled={isSavingRealtimeAgentPrompt}
                              onClick={() =>
                                setRealtimeAgentPromptDraft(
                                  REALTIME_AGENT_PROMPT_TEMPLATE,
                                )
                              }
                            >
                              Starter
                            </Button>
                            <Button
                              variant="ghost"
                              size="sm"
                              disabled={isSavingRealtimeAgentPrompt}
                              onClick={() => void handleResetRealtimeAgentPrompt()}
                            >
                              Reset
                            </Button>
                            <Button
                              variant="secondary"
                              size="sm"
                              disabled={
                                isSavingRealtimeAgentPrompt ||
                                !realtimeAgentPromptDirty
                              }
                              onClick={() => void handleSaveRealtimeAgentPrompt()}
                            >
                              Save
                            </Button>
                          </div>
                        </div>
                        <Textarea
                          value={realtimeAgentPromptDraft}
                          onChange={(event) =>
                            setRealtimeAgentPromptDraft(event.target.value)
                          }
                          placeholder="Add names, vocabulary, formatting rules, or output-language instructions. Variables: ${language}, ${translate_to_english}."
                          className="min-h-[120px] w-full resize-y border-blue-400/20 bg-[#151515] text-sm"
                        />
                        {!realtimeAgentPromptDraft.trim() && (
                          <div className="mt-2 rounded-md border border-amber-400/40 bg-amber-400/10 p-2 text-[11px] text-amber-100">
                            Empty prompt means "use the built-in default".
                            AivoRelay will still send the fixed transcript-only
                            guardrails plus the default Realtime 2 STT prompt,
                            profile/global language, and Translate to English
                            settings.
                          </div>
                        )}
                        <p className="mt-2 text-[11px] text-text/60">
                          Variables supported here:{" "}
                          <span className="font-mono">${"{language}"}</span>{" "}
                          and{" "}
                          <span className="font-mono">
                            ${"{translate_to_english}"}
                          </span>
                          . For Follow OS Input Language, AivoRelay resolves the
                          real language when recording starts. The base
                          instruction still forces transcript-only output.
                        </p>
                        <details className="mt-3">
                          <summary className="cursor-pointer text-blue-300">
                            Full instruction preview
                          </summary>
                          <Textarea
                            value={realtimeAgentInstructionPreview}
                            readOnly
                            className="mt-2 min-h-[150px] w-full resize-y border-blue-400/20 bg-[#101010] text-xs font-mono text-text/80"
                          />
                        </details>
                      </div>
                      <details className="mt-2">
                        <summary className="cursor-pointer text-blue-300">
                          What this mode actually does
                        </summary>
                        <ul className="mt-2 list-disc space-y-1 pl-5">
                          <li>
                            This is not the normal OpenAI transcription
                            endpoint. It is a voice-agent Realtime model used as
                            STT.
                          </li>
                          <li>
                            AivoRelay opens a Realtime WebSocket for
                            <span className="font-mono"> gpt-realtime-2</span>,
                            sends 24 kHz PCM audio, asks for text-only output,
                            and collects text deltas as the transcript.
                          </li>
                          <li>
                            Translation to English is prompt-driven through the
                            same Realtime agent path.
                          </li>
                          <li>
                            Base URL must stay as OpenAI. Custom URLs still use
                            the classic OpenAI-compatible path instead.
                          </li>
                        </ul>
                      </details>
                    </>
                  ) : (
                    <>
                      <div className="mb-3 rounded-lg border border-red-500/50 bg-red-500/15 p-3 text-red-100">
                        <p className="font-semibold uppercase tracking-wide">
                          Important output language warning
                        </p>
                        <p className="mt-1">
                          This model can listen to multilingual speech, but the
                          translation endpoint still needs an output language
                          target. For same-language STT, that target must match
                          what you are speaking.
                        </p>
                        <p className="mt-1">
                          If you leave language on Auto, AivoRelay does not use
                          speech auto-detection to pick the output target.
                          Auto follows the current OS keyboard/input language
                          because OpenAI requires a target language for this
                          endpoint.
                        </p>
                      </div>
                      <p className="font-medium text-text">
                        How to configure: OpenAI key here, language in
                        profiles.
                      </p>
                      <div className="mt-3 rounded-lg border border-violet-300/25 bg-black/20 p-3">
                        <div className="flex flex-wrap items-center justify-between gap-2">
                          <div>
                            <p className="font-medium text-text">
                              Output target
                            </p>
                            <p className="mt-0.5 text-[11px] text-text/60">
                              Source: {realtimeLanguageSettingSource}
                            </p>
                          </div>
                          <span className="rounded-md border border-violet-300/25 bg-violet-300/10 px-2 py-1 text-xs font-medium text-violet-100">
                            {realtimeTranslateOutputTarget}
                          </span>
                        </div>
                        <p className="mt-2 text-[11px] text-text/65">
                          Change this in the active transcription profile. If
                          the Default profile is active, change the global
                          language and Translate to English settings. This model
                          card only selects the OpenAI gpt-realtime-translate
                          interface and stores the OpenAI key.
                        </p>
                      </div>
                      <p className="mt-1">
                        This mode uses the active transcription profile's
                        language as the output target and its Translate to
                        English setting. If no profile is active, it uses the
                        global language and global Translate to English toggle.
                      </p>
                      <p className="mt-1">
                        For same-language STT, set Translate to English OFF and
                        choose the spoken language in the profile/global
                        language setting. Auto follows the current OS input
                        language for this model only.
                      </p>
                      <details className="mt-2">
                        <summary className="cursor-pointer text-blue-300">
                          What this mode actually does
                        </summary>
                        <ul className="mt-2 list-disc space-y-1 pl-5">
                          <li>
                            AivoRelay opens the Realtime translation endpoint
                            with
                            <span className="font-mono">
                              {" "}
                              gpt-realtime-translate
                            </span>.
                          </li>
                          <li>
                            With Translate to English OFF, AivoRelay sets the
                            output target to the language you selected in the
                            profile/global language setting. The incoming speech
                            can still be multilingual; the selected language is
                            the text/audio output target. If that language is
                            Auto, AivoRelay resolves it from the current OS
                            input language and reads transcript events.
                          </li>
                          <li>
                            With Translate to English ON, AivoRelay targets
                            English instead.
                          </li>
                          <li>
                            OpenAI exposes target-language configuration here,
                            not free-form prompt instructions, so profile/global
                            STT prompts do not apply to this mode.
                          </li>
                        </ul>
                      </details>
                    </>
                  )}
                </div>
              )}

              {remotePreset === "custom" ? (
                <>
                  <ToggleSwitch
                    checked={remoteAllowInsecureHttp}
                    onChange={(enabled) =>
                      void handleRemoteHttpOverrideChange(enabled)
                    }
                    isUpdating={false}
                    label={t("settings.advanced.remoteStt.customHttpOverride.title")}
                    description={t(
                      "settings.advanced.remoteStt.customHttpOverride.description",
                    )}
                    descriptionMode={descriptionMode}
                    grouped={grouped}
                  />

                  {remoteAllowInsecureHttp ? (
                    <div className="mx-4 rounded-lg border border-red-500/40 bg-red-500/10 p-3 text-sm text-red-200">
                      {t("settings.advanced.remoteStt.customHttpOverride.warning")}
                    </div>
                  ) : null}
                </>
              ) : null}

              {currentRemoteInterface === "groq" && (
                <SettingContainer
                  title="Groq Model"
                  description="Choose the Groq OpenAI-compatible transcription model."
                  descriptionMode={descriptionMode}
                  grouped={grouped}
                  layout="stacked"
                >
                  <Select
                    value={modelIdInput}
                    options={groqModelOptions}
                    onChange={handleGroqModelChange}
                    isClearable={false}
                    className="w-full"
                  />
                </SettingContainer>
              )}

              {currentRemoteInterface === "custom" && (
                <SettingContainer
                  title={t("settings.advanced.remoteStt.modelId.title")}
                  description={t("settings.advanced.remoteStt.modelId.description")}
                  descriptionMode={descriptionMode}
                  grouped={grouped}
                  layout="stacked"
                >
                  <Input
                    type="text"
                    value={modelIdInput}
                    onChange={(event) => {
                      const nextValue = event.target.value;
                      setModelIdInput(nextValue);
                      setCustomModelId(nextValue);
                    }}
                    onBlur={handleModelIdBlur}
                    placeholder={t("settings.advanced.remoteStt.modelId.placeholder")}
                    className="w-full"
                  />
                </SettingContainer>
              )}
            </>
          )}

          {showSonioxFields && (
            <>
              <div className="p-3 bg-yellow-500/10 border border-yellow-500/30 rounded-lg text-sm text-yellow-400 mx-4 mt-2">
                {t("onboarding.remoteSttWizard.sonioxRealtimeWarning")}
              </div>
              <TellMeMore
                title={t("settings.advanced.soniox.tellMeMore.title")}
              >
                <div className="space-y-3">
                  <p>
                    <strong>{t("settings.advanced.soniox.tellMeMore.headline")}</strong>
                  </p>
                  <p>{t("settings.advanced.soniox.tellMeMore.liveFlow")}</p>
                  <p>{t("settings.advanced.soniox.tellMeMore.stopFlow")}</p>
                  <div>
                    <p className="mb-1 font-medium">
                      {t("settings.advanced.soniox.tellMeMore.userStory.title")}
                    </p>
                    <ul className="list-disc space-y-1 pl-5 text-sm text-text/90">
                      <li>{t("settings.advanced.soniox.tellMeMore.userStory.item1")}</li>
                      <li>{t("settings.advanced.soniox.tellMeMore.userStory.item2")}</li>
                      <li>{t("settings.advanced.soniox.tellMeMore.userStory.item3")}</li>
                    </ul>
                  </div>
                  <p className="text-text/80">
                    {t("settings.advanced.soniox.tellMeMore.tip")}
                  </p>
                </div>
              </TellMeMore>

              <SettingContainer
                title={t("settings.advanced.soniox.reset.title")}
                description={t("settings.advanced.soniox.reset.description")}
                descriptionMode={descriptionMode}
                grouped={grouped}
              >
                <Button variant="secondary" size="sm" onClick={handleResetSonioxDefaults}>
                  {t("settings.advanced.soniox.reset.button")}
                </Button>
              </SettingContainer>

              <div className="px-4 pt-3">
                <TellMeMore
                  title={t("settings.advanced.soniox.tellMeMore.parametersTitle")}
                >
                  <ul className="list-disc space-y-1 pl-5 text-sm text-text/90">
                    {["model", "live", "timeout"].map((id) => (
                      <li key={id}>
                        {t(`settings.advanced.soniox.tellMeMore.parameters.${id}`)}
                      </li>
                    ))}
                  </ul>
                </TellMeMore>
              </div>

              <SettingContainer
                title={t("settings.advanced.soniox.model.title")}
                description={t("settings.advanced.soniox.model.description")}
                descriptionMode={descriptionMode}
                grouped={grouped}
                layout="stacked"
              >
                <Select
                  value={sonioxModelMode}
                  options={sonioxModelOptions}
                  onChange={handleSonioxModelChange}
                  placeholder={t("settings.advanced.soniox.model.placeholder")}
                  isClearable={false}
                  className="w-full"
                />
              </SettingContainer>

              {showSonioxRealtimeV5Upgrade && (
                <div className="mx-4 rounded border border-cyan-500/30 bg-cyan-500/10 p-3 text-sm text-cyan-100">
                  <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                    <div className="min-w-0">
                      <p className="font-medium">
                        {t(
                          "settings.advanced.soniox.model.upgradeTitle",
                          "Soniox real-time v5 is available",
                        )}
                      </p>
                      <p className="mt-1 text-cyan-200/90">
                        {t("settings.advanced.soniox.model.upgradeDescription", {
                          currentModel: trimmedSonioxModel || sonioxModel,
                          targetModel: SONIOX_DEFAULT_REALTIME_MODEL,
                          defaultValue:
                            "You are using {{currentModel}}. Click here to switch this field to {{targetModel}} and apply it.",
                        })}
                      </p>
                    </div>
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={handleUpgradeSonioxRealtimeModel}
                      disabled={
                        isUpgradingSonioxModel || isUpdating("soniox_model")
                      }
                      className="inline-flex shrink-0 items-center justify-center gap-2 whitespace-nowrap"
                    >
                      <RefreshCw
                        className={`h-3.5 w-3.5 ${
                          isUpgradingSonioxModel ? "animate-spin" : ""
                        }`}
                      />
                      {t(
                        "settings.advanced.soniox.model.upgradeButton",
                        "Update to stt-rt-v5",
                      )}
                    </Button>
                  </div>
                </div>
              )}

              {sonioxModelMode === "custom" && (
                <SettingContainer
                  title={t(
                    "settings.advanced.soniox.model.customTitle",
                    "Custom model id",
                  )}
                  description={t(
                    "settings.advanced.soniox.model.customDescription",
                    "Enter the exact Soniox model id. Models starting with stt-rt use the real-time path; models starting with stt-async use the async file/job path.",
                  )}
                  descriptionMode={descriptionMode}
                  grouped={grouped}
                  layout="stacked"
                >
                  <PersistentHintInput
                    type="text"
                    value={customSonioxModelInput}
                    onChange={(event) =>
                      setCustomSonioxModelInput(event.target.value)
                    }
                    onBlur={handleCustomSonioxModelBlur}
                    placeholder="stt-rt-v5"
                    hint="stt-rt-v5"
                  />
                </SettingContainer>
              )}

              {isSonioxAsyncModel && sonioxModelMode !== "custom" && (
                <div className="mx-4 rounded border border-cyan-500/30 bg-cyan-500/10 p-3 text-sm text-cyan-200">
                  {t(
                    "settings.advanced.soniox.model.asyncNotice",
                    "Soniox async models upload the finished recording as a file, create a transcription job, poll until it completes, then fetch the transcript. That makes them naturally slower for short dictation than one-request providers such as Groq Whisper Turbo. AivoRelay can reduce polling overhead, but it cannot turn the Soniox async file/job pipeline into an instant request.",
                  )}
                </div>
              )}

              <SettingContainer
                title={t("settings.advanced.soniox.timeout.title")}
                description={t("settings.advanced.soniox.timeout.description")}
                descriptionMode={descriptionMode}
                grouped={grouped}
                layout="stacked"
              >
                <PersistentHintInput
                  type="number"
                  value={sonioxTimeoutInput}
                  onChange={(event) => setSonioxTimeoutInput(event.target.value)}
                  onBlur={handleSonioxTimeoutBlur}
                  min={10}
                  max={300}
                  hint="30"
                  hintClassName="right-8"
                  inputPaddingClassName="pr-20"
                />
              </SettingContainer>

              <div className="px-4 pt-3">
                <TellMeMore
                  title={t("settings.advanced.soniox.tellMeMore.realtimeBehavior.title")}
                >
                  <ul className="list-disc space-y-1 pl-5 text-sm text-text/90">
                    {[
                      "tokenDraft",
                      "tokenFinal",
                      "manualFinalize",
                      "finMarker",
                      "keepalive",
                      "silenceRule",
                    ].map((id) => (
                      <li key={id}>
                        {t(`settings.advanced.soniox.tellMeMore.realtimeBehavior.${id}`)}
                      </li>
                    ))}
                  </ul>
                </TellMeMore>
              </div>

              <ToggleSwitch
                label={t("settings.advanced.soniox.live.title")}
                description={t("settings.advanced.soniox.live.description")}
                checked={sonioxLiveEnabled}
                onChange={(enabled) =>
                  void updateSetting("soniox_live_enabled" as any, enabled as any)
                }
                isUpdating={isUpdating("soniox_live_enabled")}
                disabled={!sonioxRealtimeControlsEnabled}
                descriptionMode={descriptionMode}
                grouped={grouped}
              />

              <ToggleSwitch
                label={t("settings.advanced.soniox.optimizeDeliveryPreconnect.title")}
                description={t(
                  "settings.advanced.soniox.optimizeDeliveryPreconnect.description",
                )}
                checked={sonioxOptimizeDeliveryPreconnectEnabled}
                onChange={(enabled) =>
                  void updateSetting(
                    "soniox_optimize_delivery_preconnect_enabled" as any,
                    enabled as any,
                  )
                }
                isUpdating={isUpdating(
                  "soniox_optimize_delivery_preconnect_enabled",
                )}
                disabled={!sonioxRealtimeControlsEnabled || sonioxLiveEnabled}
                descriptionMode={descriptionMode}
                grouped={grouped}
              />

              <div className="px-4 pt-3">
                <TellMeMore
                  title={t("settings.advanced.soniox.tellMeMore.matrix.title")}
                >
                  <div className="space-y-2 text-sm text-text/90">
                    {["live", "languageHints"].map((id) => (
                      <div
                        key={id}
                        className="rounded border border-mid-gray/25 bg-mid-gray/10 p-2"
                      >
                        <p className="font-medium">
                          {t(`settings.advanced.soniox.tellMeMore.matrix.items.${id}.title`)}
                        </p>
                        <p>
                          <strong>
                            {t("settings.advanced.soniox.tellMeMore.matrix.whenToUseLabel")}
                          </strong>{" "}
                          {t(`settings.advanced.soniox.tellMeMore.matrix.items.${id}.whenToUse`)}
                        </p>
                        <p>
                          <strong>
                            {t("settings.advanced.soniox.tellMeMore.matrix.tradeoffLabel")}
                          </strong>{" "}
                          {t(`settings.advanced.soniox.tellMeMore.matrix.items.${id}.tradeoff`)}
                        </p>
                        <p>
                          <strong>
                            {t("settings.advanced.soniox.tellMeMore.matrix.recommendedLabel")}
                          </strong>{" "}
                          {t(`settings.advanced.soniox.tellMeMore.matrix.items.${id}.recommended`)}
                        </p>
                      </div>
                    ))}
                  </div>
                </TellMeMore>
              </div>

              {!sonioxRealtimeControlsEnabled && (
                <p className="text-xs text-text/60">
                  {t("settings.advanced.soniox.live.realtimeOnly")}
                </p>
              )}

              <ToggleSwitch
                label={t("settings.advanced.soniox.profileLanguageHintOnly.title")}
                description={t(
                  "settings.advanced.soniox.profileLanguageHintOnly.description",
                )}
                checked={sonioxUseProfileLanguageHintOnly}
                onChange={(enabled) =>
                  void updateSetting(
                    "soniox_use_profile_language_hint_only" as any,
                    enabled as any,
                  )
                }
                isUpdating={isUpdating("soniox_use_profile_language_hint_only")}
                descriptionMode={descriptionMode}
                grouped={grouped}
              />

              <div className="px-4 pt-3">
                <TellMeMore
                  title={t("settings.advanced.soniox.tellMeMore.parametersTitle")}
                >
                  <ul className="list-disc space-y-1 pl-5 text-sm text-text/90">
                    {[
                      "languageHints",
                      "profileLanguageHintOnly",
                      "strict",
                      "languageIdentification",
                      "speakerDiarization",
                    ].map((id) => (
                      <li key={id}>
                        {t(`settings.advanced.soniox.tellMeMore.parameters.${id}`)}
                      </li>
                    ))}
                  </ul>
                </TellMeMore>
              </div>

              <SettingContainer
                title={t("settings.advanced.soniox.languageHints.title")}
                description={t("settings.advanced.soniox.languageHints.description")}
                descriptionMode={descriptionMode}
                grouped={grouped}
                layout="stacked"
                disabled={sonioxUseProfileLanguageHintOnly}
              >
                <div className="mb-2 flex justify-end gap-2">
                  <a
                    href="https://soniox.com/docs/stt/concepts/supported-languages"
                    target="_blank"
                    rel="noreferrer"
                    className="text-xs text-accent hover:underline"
                  >
                    {t("settings.advanced.soniox.languageHints.supportedLanguagesLink")}
                  </a>
                  <span className="text-xs text-mid-gray/40">·</span>
                  <a
                    href="https://soniox.com/docs/stt/concepts/language-hints"
                    target="_blank"
                    rel="noreferrer"
                    className="text-xs text-accent hover:underline"
                  >
                    {t("settings.advanced.soniox.languageHints.languageHintsDocsLink", "Language hints docs")}
                  </a>
                </div>
                <PersistentHintInput
                  type="text"
                  value={sonioxLanguageHintsInput}
                  onChange={(event) =>
                    setSonioxLanguageHintsInput(event.target.value)
                  }
                  onBlur={handleSonioxLanguageHintsBlur}
                  placeholder={t("settings.advanced.soniox.languageHints.placeholder")}
                  disabled={sonioxUseProfileLanguageHintOnly}
                  hint="e.g. en, fr"
                />
              </SettingContainer>

              <ToggleSwitch
                label={t("settings.advanced.soniox.languageHintsStrict.title")}
                description={t(
                  "settings.advanced.soniox.languageHintsStrict.description",
                )}
                checked={sonioxLanguageHintsStrict}
                onChange={(enabled) =>
                  void updateSetting(
                    "soniox_language_hints_strict" as any,
                    enabled as any,
                  )
                }
                isUpdating={isUpdating("soniox_language_hints_strict")}
                descriptionMode={descriptionMode}
                grouped={grouped}
              />

              <ToggleSwitch
                label={t("settings.advanced.soniox.endpointDetection.title")}
                description={t(
                  "settings.advanced.soniox.endpointDetection.description",
                )}
                checked={sonioxEnableEndpointDetection}
                onChange={(enabled) =>
                  void updateSetting(
                    "soniox_enable_endpoint_detection" as any,
                    enabled as any,
                  )
                }
                isUpdating={isUpdating("soniox_enable_endpoint_detection")}
                disabled={!sonioxRealtimeControlsEnabled}
                descriptionMode={descriptionMode}
                grouped={grouped}
              />

              <div className="px-4 pt-3">
                <TellMeMore
                  title={t("settings.advanced.soniox.tellMeMore.matrix.title")}
                >
                  <div className="space-y-2 text-sm text-text/90">
                    {["endpointDetection", "finalizeTimeout", "instantStop"].map((id) => (
                      <div
                        key={id}
                        className="rounded border border-mid-gray/25 bg-mid-gray/10 p-2"
                      >
                        <p className="font-medium">
                          {t(`settings.advanced.soniox.tellMeMore.matrix.items.${id}.title`)}
                        </p>
                        <p>
                          <strong>
                            {t("settings.advanced.soniox.tellMeMore.matrix.whenToUseLabel")}
                          </strong>{" "}
                          {t(`settings.advanced.soniox.tellMeMore.matrix.items.${id}.whenToUse`)}
                        </p>
                        <p>
                          <strong>
                            {t("settings.advanced.soniox.tellMeMore.matrix.tradeoffLabel")}
                          </strong>{" "}
                          {t(`settings.advanced.soniox.tellMeMore.matrix.items.${id}.tradeoff`)}
                        </p>
                        <p>
                          <strong>
                            {t("settings.advanced.soniox.tellMeMore.matrix.recommendedLabel")}
                          </strong>{" "}
                          {t(`settings.advanced.soniox.tellMeMore.matrix.items.${id}.recommended`)}
                        </p>
                      </div>
                    ))}
                  </div>
                  <ul className="list-disc space-y-1 pl-5 mt-3 text-sm text-text/90">
                    {[
                      "endpoint",
                      "endpointSensitivity",
                      "keepalive",
                      "finalizeTimeout",
                      "instantStop",
                    ].map((id) => (
                      <li key={id}>
                        {t(`settings.advanced.soniox.tellMeMore.parameters.${id}`)}
                      </li>
                    ))}
                  </ul>
                </TellMeMore>
              </div>

              <SettingContainer
                title={t("settings.advanced.soniox.maxEndpointDelay.title")}
                description={t("settings.advanced.soniox.maxEndpointDelay.description")}
                descriptionMode={descriptionMode}
                grouped={grouped}
                layout="stacked"
                disabled={!sonioxRealtimeControlsEnabled}
              >
                <PersistentHintInput
                  type="number"
                  value={sonioxMaxEndpointDelayMsInput}
                  onChange={(event) =>
                    setSonioxMaxEndpointDelayMsInput(event.target.value)
                  }
                  onBlur={handleSonioxMaxEndpointDelayBlur}
                  min={500}
                  max={3000}
                  disabled={!sonioxRealtimeControlsEnabled}
                  hint="2000"
                  hintClassName="right-8"
                  inputPaddingClassName="pr-20"
                />
              </SettingContainer>

              <SettingContainer
                title={t("settings.advanced.soniox.endpointSensitivity.title")}
                description={t(
                  "settings.advanced.soniox.endpointSensitivity.description",
                )}
                descriptionMode={descriptionMode}
                grouped={grouped}
                layout="stacked"
                disabled={!sonioxEndpointSensitivityEnabled}
              >
                <PersistentHintInput
                  type="number"
                  value={sonioxEndpointSensitivityInput}
                  onChange={(event) =>
                    setSonioxEndpointSensitivityInput(event.target.value)
                  }
                  onBlur={handleSonioxEndpointSensitivityBlur}
                  min={-1}
                  max={1}
                  step={0.1}
                  disabled={!sonioxEndpointSensitivityEnabled}
                  hint="0.0"
                  hintClassName="right-8"
                  inputPaddingClassName="pr-20"
                />
              </SettingContainer>

              <SettingContainer
                title={t("settings.advanced.soniox.keepalive.title")}
                description={t("settings.advanced.soniox.keepalive.description")}
                descriptionMode={descriptionMode}
                grouped={grouped}
                layout="stacked"
                disabled={!sonioxRealtimeControlsEnabled}
              >
                <PersistentHintInput
                  type="number"
                  value={sonioxKeepaliveSecondsInput}
                  onChange={(event) =>
                    setSonioxKeepaliveSecondsInput(event.target.value)
                  }
                  onBlur={handleSonioxKeepaliveBlur}
                  min={5}
                  max={20}
                  disabled={!sonioxRealtimeControlsEnabled}
                  hint="10"
                  hintClassName="right-8"
                  inputPaddingClassName="pr-20"
                />
              </SettingContainer>

              <SettingContainer
                title={t("settings.advanced.soniox.finalizeTimeout.title")}
                description={t("settings.advanced.soniox.finalizeTimeout.description")}
                descriptionMode={descriptionMode}
                grouped={grouped}
                layout="stacked"
                disabled={!sonioxRealtimeControlsEnabled}
              >
                <PersistentHintInput
                  type="number"
                  value={sonioxLiveFinalizeTimeoutInput}
                  onChange={(event) =>
                    setSonioxLiveFinalizeTimeoutInput(event.target.value)
                  }
                  onBlur={handleSonioxLiveFinalizeTimeoutBlur}
                  min={100}
                  max={20000}
                  disabled={!sonioxRealtimeControlsEnabled}
                  hint="500"
                  hintClassName="right-8"
                  inputPaddingClassName="pr-20"
                />
              </SettingContainer>

              <ToggleSwitch
                label={t("settings.advanced.soniox.instantStop.title")}
                description={t("settings.advanced.soniox.instantStop.description")}
                checked={sonioxLiveInstantStop}
                onChange={(enabled) =>
                  void updateSetting("soniox_live_instant_stop" as any, enabled as any)
                }
                isUpdating={isUpdating("soniox_live_instant_stop")}
                disabled={!sonioxRealtimeControlsEnabled}
                descriptionMode={descriptionMode}
                grouped={grouped}
              />

              <ToggleSwitch
                label={t("settings.advanced.soniox.languageIdentification.title")}
                description={t(
                  "settings.advanced.soniox.languageIdentification.description",
                )}
                checked={sonioxEnableLanguageIdentification}
                onChange={(enabled) =>
                  void updateSetting(
                    "soniox_enable_language_identification" as any,
                    enabled as any,
                  )
                }
                isUpdating={isUpdating("soniox_enable_language_identification")}
                descriptionMode={descriptionMode}
                grouped={grouped}
              />

              <ToggleSwitch
                label={t("settings.advanced.soniox.speakerDiarization.title")}
                description={t(
                  "settings.advanced.soniox.speakerDiarization.description",
                )}
                checked={sonioxEnableSpeakerDiarization}
                onChange={(enabled) =>
                  void updateSetting(
                    "soniox_enable_speaker_diarization" as any,
                    enabled as any,
                  )
                }
                isUpdating={isUpdating("soniox_enable_speaker_diarization")}
                descriptionMode={descriptionMode}
                grouped={grouped}
              />
            </>
          )}

          {showDeepgramFields && (
            <>
              <div className="p-3 bg-cyan-500/10 border border-cyan-500/30 rounded-lg text-sm text-cyan-300 mx-4 mt-2">
                {t("settings.advanced.deepgram.banner")}
              </div>

              <TellMeMore title={t("settings.advanced.deepgram.tellMeMore.title")}>
                <div className="space-y-3">
                  <p>
                    <strong>{t("settings.advanced.deepgram.tellMeMore.headline")}</strong>
                  </p>
                  <p>{t("settings.advanced.deepgram.tellMeMore.liveFlow")}</p>
                  <p>{t("settings.advanced.deepgram.tellMeMore.stopFlow")}</p>
                  <p className="text-text/80">
                    {t("settings.advanced.deepgram.tellMeMore.tip")}
                  </p>
                </div>
              </TellMeMore>

              <SettingContainer
                title={t("settings.advanced.deepgram.reset.title")}
                description={t("settings.advanced.deepgram.reset.description")}
                descriptionMode={descriptionMode}
                grouped={grouped}
              >
                <Button variant="secondary" size="sm" onClick={handleResetDeepgramDefaults}>
                  {t("settings.advanced.deepgram.reset.button")}
                </Button>
              </SettingContainer>

              <div className="px-4 pt-3">
                <TellMeMore
                  title={t("settings.advanced.deepgram.tellMeMore.parametersTitle")}
                >
                  <div className="space-y-2 mb-3 text-sm text-text/90">
                    <div className="rounded border border-mid-gray/25 bg-mid-gray/10 p-2">
                      <p className="font-medium">
                        {t("settings.advanced.deepgram.tellMeMore.matrix.items.model.title")}
                      </p>
                      <p>
                        <strong>
                          {t("settings.advanced.deepgram.tellMeMore.matrix.whenToUseLabel")}
                        </strong>{" "}
                        {t("settings.advanced.deepgram.tellMeMore.matrix.items.model.whenToUse")}
                      </p>
                      <p>
                        <strong>
                          {t("settings.advanced.deepgram.tellMeMore.matrix.tradeoffLabel")}
                        </strong>{" "}
                        {t("settings.advanced.deepgram.tellMeMore.matrix.items.model.tradeoff")}
                      </p>
                      <p>
                        <strong>
                          {t("settings.advanced.deepgram.tellMeMore.matrix.recommendedLabel")}
                        </strong>{" "}
                        {t("settings.advanced.deepgram.tellMeMore.matrix.items.model.recommended")}
                      </p>
                    </div>
                  </div>
                  <ul className="list-disc space-y-1 pl-5 text-sm text-text/90">
                    {["model", "timeout", "apiKey"].map((id) => (
                      <li key={id}>
                        {t(`settings.advanced.deepgram.tellMeMore.parameters.${id}`)}
                      </li>
                    ))}
                  </ul>
                  <p className="mt-3 mb-1 font-medium">
                    {t("settings.advanced.deepgram.tellMeMore.docs.title")}
                  </p>
                  <ul className="list-disc space-y-1 pl-5 text-sm">
                    <li>
                      <a
                        href="https://developers.deepgram.com/reference/speech-to-text/listen-streaming"
                        target="_blank"
                        rel="noreferrer"
                        className="text-accent hover:underline"
                      >
                        {t("settings.advanced.deepgram.tellMeMore.docs.liveApi")}
                      </a>
                    </li>
                    <li>
                      <a
                        href="https://developers.deepgram.com/docs/audio-keep-alive"
                        target="_blank"
                        rel="noreferrer"
                        className="text-accent hover:underline"
                      >
                        {t("settings.advanced.deepgram.tellMeMore.docs.keepalive")}
                      </a>
                    </li>
                    <li>
                      <a
                        href="https://developers.deepgram.com/docs/finalize"
                        target="_blank"
                        rel="noreferrer"
                        className="text-accent hover:underline"
                      >
                        {t("settings.advanced.deepgram.tellMeMore.docs.finalize")}
                      </a>
                    </li>
                    <li>
                      <a
                        href="https://developers.deepgram.com/docs/close-stream"
                        target="_blank"
                        rel="noreferrer"
                        className="text-accent hover:underline"
                      >
                        {t("settings.advanced.deepgram.tellMeMore.docs.closeStream")}
                      </a>
                    </li>
                    <li>
                      <a
                        href="https://developers.deepgram.com/docs/endpointing"
                        target="_blank"
                        rel="noreferrer"
                        className="text-accent hover:underline"
                      >
                        {t("settings.advanced.deepgram.tellMeMore.docs.endpointing")}
                      </a>
                    </li>
                    <li>
                      <a
                        href="https://developers.deepgram.com/docs/interim-results"
                        target="_blank"
                        rel="noreferrer"
                        className="text-accent hover:underline"
                      >
                        {t("settings.advanced.deepgram.tellMeMore.docs.interimResults")}
                      </a>
                    </li>
                    <li>
                      <a
                        href="https://developers.deepgram.com/docs/smart-format"
                        target="_blank"
                        rel="noreferrer"
                        className="text-accent hover:underline"
                      >
                        {t("settings.advanced.deepgram.tellMeMore.docs.smartFormat")}
                      </a>
                    </li>
                    <li>
                      <a
                        href="https://developers.deepgram.com/docs/model"
                        target="_blank"
                        rel="noreferrer"
                        className="text-accent hover:underline"
                      >
                        {t("settings.advanced.deepgram.tellMeMore.docs.modelOptions")}
                      </a>
                    </li>
                    <li>
                      <a
                        href="https://developers.deepgram.com/docs/stt-troubleshooting-websocket-data-and-net-errors"
                        target="_blank"
                        rel="noreferrer"
                        className="text-accent hover:underline"
                      >
                        {t("settings.advanced.deepgram.tellMeMore.docs.troubleshooting")}
                      </a>
                    </li>
                  </ul>
                </TellMeMore>
              </div>

              <SettingContainer
                title={t("settings.advanced.deepgram.model.title")}
                description={t("settings.advanced.deepgram.model.description")}
                descriptionMode={descriptionMode}
                grouped={grouped}
              >
                <Select
                  value={deepgramModelInput}
                  options={deepgramModelOptions}
                  onChange={handleDeepgramModelChange}
                  isClearable={false}
                />
              </SettingContainer>

              <SettingContainer
                title={t("settings.advanced.deepgram.timeout.title")}
                description={t("settings.advanced.deepgram.timeout.description")}
                descriptionMode={descriptionMode}
                grouped={grouped}
                layout="stacked"
              >
                <Input
                  type="number"
                  value={deepgramTimeoutInput}
                  onChange={(event) => setDeepgramTimeoutInput(event.target.value)}
                  onBlur={handleDeepgramTimeoutBlur}
                  min={10}
                  max={3600}
                  className="w-full"
                />
              </SettingContainer>

              <div className="px-4 pt-3">
                <TellMeMore
                  title={t("settings.advanced.deepgram.tellMeMore.controlMessages.title")}
                >
                  <ul className="list-disc space-y-1 pl-5 text-sm text-text/90">
                    {[
                      "keepalive",
                      "finalize",
                      "closeStream",
                      "fromFinalize",
                      "metadata",
                      "netTimeout",
                    ].map((id) => (
                      <li key={id}>
                        {t(`settings.advanced.deepgram.tellMeMore.controlMessages.${id}`)}
                      </li>
                    ))}
                  </ul>
                </TellMeMore>
              </div>

              <ToggleSwitch
                label={t("settings.advanced.deepgram.live.title")}
                description={t("settings.advanced.deepgram.live.description")}
                checked={deepgramLiveEnabled}
                onChange={(enabled) =>
                  void updateSetting("deepgram_live_enabled" as any, enabled as any)
                }
                isUpdating={isUpdating("deepgram_live_enabled")}
                descriptionMode={descriptionMode}
                grouped={grouped}
              />

              <div className="px-4 pt-3">
                <TellMeMore
                  title={t("settings.advanced.deepgram.tellMeMore.transcriptTiming.title")}
                >
                  <ul className="list-disc space-y-1 pl-5 text-sm text-text/90">
                    {["interimVsFinal", "speechFinal", "concatRule"].map((id) => (
                      <li key={id}>
                        {t(`settings.advanced.deepgram.tellMeMore.transcriptTiming.${id}`)}
                      </li>
                    ))}
                  </ul>
                </TellMeMore>
              </div>

              <ToggleSwitch
                label={t("settings.advanced.deepgram.interim.title")}
                description={t("settings.advanced.deepgram.interim.description")}
                checked={deepgramInterimResults}
                onChange={(enabled) =>
                  void updateSetting("deepgram_interim_results" as any, enabled as any)
                }
                isUpdating={isUpdating("deepgram_interim_results")}
                descriptionMode={descriptionMode}
                grouped={grouped}
              />

              <ToggleSwitch
                label={t("settings.advanced.deepgram.smartFormat.title")}
                description={t("settings.advanced.deepgram.smartFormat.description")}
                checked={deepgramSmartFormat}
                onChange={(enabled) =>
                  void updateSetting("deepgram_smart_format" as any, enabled as any)
                }
                isUpdating={isUpdating("deepgram_smart_format")}
                descriptionMode={descriptionMode}
                grouped={grouped}
              />

              <ToggleSwitch
                label={t("settings.advanced.deepgram.endpointing.title")}
                description={t("settings.advanced.deepgram.endpointing.description")}
                checked={deepgramEndpointingEnabled}
                onChange={(enabled) =>
                  void updateSetting("deepgram_endpointing_enabled" as any, enabled as any)
                }
                isUpdating={isUpdating("deepgram_endpointing_enabled")}
                descriptionMode={descriptionMode}
                grouped={grouped}
              />

              <div className="px-4 pt-3">
                <TellMeMore
                  title={t("settings.advanced.deepgram.tellMeMore.matrix.title")}
                >
                  <div className="space-y-2 text-sm text-text/90">
                    {[
                      "live",
                      "interim",
                      "endpointing",
                      "endpointingMs",
                      "keepalive",
                      "finalizeTimeout",
                      "instantStop",
                    ].map((id) => (
                      <div
                        key={id}
                        className="rounded border border-mid-gray/25 bg-mid-gray/10 p-2"
                      >
                        <p className="font-medium">
                          {t(`settings.advanced.deepgram.tellMeMore.matrix.items.${id}.title`)}
                        </p>
                        <p>
                          <strong>
                            {t("settings.advanced.deepgram.tellMeMore.matrix.whenToUseLabel")}
                          </strong>{" "}
                          {t(`settings.advanced.deepgram.tellMeMore.matrix.items.${id}.whenToUse`)}
                        </p>
                        <p>
                          <strong>
                            {t("settings.advanced.deepgram.tellMeMore.matrix.tradeoffLabel")}
                          </strong>{" "}
                          {t(`settings.advanced.deepgram.tellMeMore.matrix.items.${id}.tradeoff`)}
                        </p>
                        <p>
                          <strong>
                            {t("settings.advanced.deepgram.tellMeMore.matrix.recommendedLabel")}
                          </strong>{" "}
                          {t(
                            `settings.advanced.deepgram.tellMeMore.matrix.items.${id}.recommended`,
                          )}
                        </p>
                      </div>
                    ))}
                  </div>
                  <ul className="list-disc space-y-1 pl-5 mt-3 text-sm text-text/90">
                    {[
                      "live",
                      "interim",
                      "smartFormat",
                      "endpointing",
                      "endpointingMs",
                      "keepalive",
                      "finalizeTimeout",
                      "instantStop",
                    ].map((id) => (
                      <li key={id}>
                        {t(`settings.advanced.deepgram.tellMeMore.parameters.${id}`)}
                      </li>
                    ))}
                  </ul>
                </TellMeMore>
              </div>

              <SettingContainer
                title={t("settings.advanced.deepgram.endpointingMs.title")}
                description={t("settings.advanced.deepgram.endpointingMs.description")}
                descriptionMode={descriptionMode}
                grouped={grouped}
                layout="stacked"
                disabled={!deepgramEndpointingEnabled}
              >
                <Input
                  type="number"
                  value={deepgramEndpointingMsInput}
                  onChange={(event) => setDeepgramEndpointingMsInput(event.target.value)}
                  onBlur={handleDeepgramEndpointingMsBlur}
                  min={50}
                  max={5000}
                  className="w-full"
                  disabled={!deepgramEndpointingEnabled}
                />
              </SettingContainer>

              <SettingContainer
                title={t("settings.advanced.deepgram.keepalive.title")}
                description={t("settings.advanced.deepgram.keepalive.description")}
                descriptionMode={descriptionMode}
                grouped={grouped}
                layout="stacked"
              >
                <Input
                  type="number"
                  value={deepgramKeepaliveSecondsInput}
                  onChange={(event) => setDeepgramKeepaliveSecondsInput(event.target.value)}
                  onBlur={handleDeepgramKeepaliveBlur}
                  min={3}
                  max={5}
                  className="w-full"
                />
              </SettingContainer>

              <SettingContainer
                title={t("settings.advanced.deepgram.finalizeTimeout.title")}
                description={t("settings.advanced.deepgram.finalizeTimeout.description")}
                descriptionMode={descriptionMode}
                grouped={grouped}
                layout="stacked"
              >
                <Input
                  type="number"
                  value={deepgramLiveFinalizeTimeoutInput}
                  onChange={(event) =>
                    setDeepgramLiveFinalizeTimeoutInput(event.target.value)
                  }
                  onBlur={handleDeepgramLiveFinalizeTimeoutBlur}
                  min={100}
                  max={20000}
                  className="w-full"
                />
              </SettingContainer>

              <ToggleSwitch
                label={t("settings.advanced.deepgram.instantStop.title")}
                description={t("settings.advanced.deepgram.instantStop.description")}
                checked={deepgramLiveInstantStop}
                onChange={(enabled) =>
                  void updateSetting("deepgram_live_instant_stop" as any, enabled as any)
                }
                isUpdating={isUpdating("deepgram_live_instant_stop")}
                descriptionMode={descriptionMode}
                grouped={grouped}
              />
            </>
          )}

          <SettingContainer
            title={
              isSonioxProvider
                ? t("settings.advanced.soniox.apiKey.title")
                : isDeepgramProvider
                  ? t("settings.advanced.deepgram.apiKey.title")
                : remoteApiKeyTitle
            }
            description={
              isSonioxProvider
                ? t("settings.advanced.soniox.apiKey.description")
                : isDeepgramProvider
                  ? t("settings.advanced.deepgram.apiKey.description")
                : remoteApiKeyDescription
            }
            descriptionMode={descriptionMode}
            grouped={grouped}
            layout="stacked"
          >
            <div className="flex flex-col gap-3 rounded-lg border border-mid-gray/30 bg-mid-gray/5 p-3">
              {hasApiKey && !isEditingKey ? (
                <StoredApiKeyDisplay
                  loading={apiKeyLoading}
                  onDelete={handleClearApiKey}
                  onReplace={handleStartReplaceKey}
                />
              ) : (
                <ApiKeyEditor
                  loading={apiKeyLoading}
                  value={apiKeyInput}
                  onChange={setApiKeyInput}
                  onSave={handleSaveApiKey}
                  onCancel={handleCancelReplaceKey}
                  placeholder={
                    isSonioxProvider
                      ? t("settings.advanced.soniox.apiKey.placeholder")
                      : isDeepgramProvider
                        ? t("settings.advanced.deepgram.apiKey.placeholder")
                        : t("settings.advanced.remoteStt.apiKey.placeholder")
                  }
                  showCancel={hasApiKey}
                  hint={
                    hasApiKey
                      ? t("settings.advanced.remoteStt.apiKey.replaceHint")
                      : t("settings.advanced.remoteStt.apiKey.statusMissing")
                  }
                />
              )}

              <div className="flex flex-col gap-2">
                {showOpenAiFields && (
                  <>
                    <div className="flex items-center gap-2">
                      <Button
                        variant="secondary"
                        size="sm"
                        onClick={handleTestConnection}
                        disabled={
                          !canTestConnection || connectionStatus === "checking"
                        }
                      >
                        {connectionStatus === "checking"
                          ? t("settings.advanced.remoteStt.connection.testing")
                          : t("settings.advanced.remoteStt.connection.test")}
                      </Button>
                    </div>
                    {connectionMessage && (
                      <span
                        className={`text-xs ${
                          connectionStatus === "success"
                            ? "text-green-400"
                            : "text-red-400"
                        }`}
                      >
                        {connectionMessage}
                      </span>
                    )}
                  </>
                )}
              </div>
            </div>
          </SettingContainer>

          {showOpenAiFields && (
            <>
              <ToggleSwitch
                checked={debugCapture}
                onChange={(enabled) => updateRemoteSttDebugCapture(enabled)}
                isUpdating={isUpdating("remote_stt_debug_capture")}
                label={t("settings.advanced.remoteStt.debug.capture.title")}
                description={t(
                  "settings.advanced.remoteStt.debug.capture.description",
                )}
                descriptionMode={descriptionMode}
                grouped={grouped}
              />

              <SettingContainer
                title={t("settings.advanced.remoteStt.debug.mode.title")}
                description={t("settings.advanced.remoteStt.debug.mode.description")}
                descriptionMode={descriptionMode}
                grouped={grouped}
              >
                <Select
                  value={debugMode}
                  options={[
                    {
                      value: "normal",
                      label: t(
                        "settings.advanced.remoteStt.debug.mode.options.normal",
                      ),
                    },
                    {
                      value: "verbose",
                      label: t(
                        "settings.advanced.remoteStt.debug.mode.options.verbose",
                      ),
                    },
                  ]}
                  onChange={(value) => value && updateRemoteSttDebugMode(value)}
                  isClearable={false}
                  disabled={!debugCapture}
                />
              </SettingContainer>

              <SettingContainer
                title={t("settings.advanced.remoteStt.debug.output.title")}
                description={t("settings.advanced.remoteStt.debug.output.description")}
                descriptionMode={descriptionMode}
                grouped={grouped}
                layout="stacked"
              >
                <div className="flex flex-col gap-2">
                  <Textarea
                    value={
                      debugLines.length > 0
                        ? debugLines.join("\n")
                        : t("settings.advanced.remoteStt.debug.output.empty")
                    }
                    readOnly
                    className="min-h-[160px] max-h-[300px] overflow-y-auto font-mono text-xs"
                  />
                  <div className="flex justify-end">
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={handleClearDebug}
                      disabled={debugLines.length === 0}
                    >
                      {t("settings.advanced.remoteStt.debug.output.clear")}
                    </Button>
                  </div>
                </div>
              </SettingContainer>
            </>
          )}
        </>
      )}
    </div>
  );
};
