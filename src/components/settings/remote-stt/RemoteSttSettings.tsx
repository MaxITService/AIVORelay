import React, { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { type } from "@tauri-apps/plugin-os";
import { toast } from "sonner";
import { commands } from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import {
  REMOTE_STT_PRESETS,
  type RemoteSttPreset,
} from "../../../lib/constants/remoteSttProviders";
import { parseAndNormalizeSonioxLanguageHints } from "../../../lib/constants/sonioxLanguages";
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
}

export const RemoteSttSettings: React.FC<RemoteSttSettingsProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
  hideProviderSelector = false,
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
  const sonioxModel = (settings as any)?.soniox_model ?? "stt-rt-v4";
  const sonioxTimeout = Number((settings as any)?.soniox_timeout_seconds ?? 30);
  const sonioxLiveEnabled = Boolean(
    (settings as any)?.soniox_live_enabled ?? true,
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
  const isRemoteOpenAiProvider = provider === "remote_openai_compatible";
  const isSonioxProvider = provider === "remote_soniox";
  const isDeepgramProvider = provider === "remote_deepgram";
  const isCloudProvider =
    isRemoteOpenAiProvider || isSonioxProvider || isDeepgramProvider;
  const isKnownSonioxPreset =
    sonioxModel.trim() === "stt-rt-v4" || sonioxModel.trim() === "stt-async-v4";
  const derivedSonioxModelMode = isKnownSonioxPreset
    ? sonioxModel.trim()
    : "custom";
  const isSonioxRealtimeModel = sonioxModel.trim().startsWith("stt-rt");
  const isSonioxAsyncModel = sonioxModel.trim().startsWith("stt-async");
  const effectiveRemoteBaseUrl =
    remotePreset === "custom"
      ? remoteSettings?.base_url ?? ""
      : (REMOTE_STT_PRESETS[remotePreset]?.baseUrl ??
          remoteSettings?.base_url ??
          "");

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
  const sonioxRealtimeControlsEnabled =
    sonioxModelMode === "custom" || isSonioxRealtimeModel;

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
  }, [isWindows, provider, isSonioxProvider, isDeepgramProvider]);

  useEffect(() => {
    if (!hasKeyStatusLoaded) {
      return;
    }
    if (!hasApiKey) {
      setIsEditingKey(true);
    }
  }, [hasApiKey, hasKeyStatusLoaded]);

  useEffect(() => {
    setConnectionStatus("idle");
    setConnectionMessage(null);
  }, [baseUrlInput, hasApiKey, provider]);

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
        value: "stt-rt-v4",
        label: t(
          "settings.advanced.soniox.model.options.realtime",
          "stt-rt-v4 - Real-time",
        ),
      },
      {
        value: "stt-async-v4",
        label: t(
          "settings.advanced.soniox.model.options.async",
          "stt-async-v4 - Async",
        ),
      },
      {
        value: "custom",
        label: t("settings.advanced.soniox.model.options.custom", "Custom"),
      },
    ],
    [t],
  );

  const remotePresetOptions = useMemo<SelectOption[]>(
    () => [
      {
        value: "groq",
        label: t("settings.advanced.remoteStt.providerPreset.options.groq"),
      },
      {
        value: "openai",
        label: t("settings.advanced.remoteStt.providerPreset.options.openai"),
      },
      {
        value: "custom",
        label: t("settings.advanced.remoteStt.providerPreset.options.custom"),
      },
    ],
    [t],
  );

  const handleProviderChange = (value: string | null) => {
    if (!value) return;
    void setTranscriptionProvider(value);
  };

  const handleRemotePresetChange = async (value: string | null) => {
    if (!value) return;
    const nextPreset = value as RemoteSttPreset;
    const savedCustomModelId =
      remotePreset === "custom" ? modelIdInput.trim() : customModelId.trim();
    try {
      if (remotePreset === "custom") {
        setCustomModelId(modelIdInput.trim());
      }

      await invoke("change_remote_stt_provider_preset_setting", {
        preset: nextPreset,
      });

      if (nextPreset === "custom" && savedCustomModelId.length > 0) {
        await updateRemoteSttModelId(savedCustomModelId);
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

  const handleSonioxModelChange = (value: string | null) => {
    const nextMode = value || "stt-rt-v4";
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
    hasApiKey &&
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
              <SettingContainer
                title={t("settings.advanced.remoteStt.providerPreset.title")}
                description={t(
                  "settings.advanced.remoteStt.providerPreset.description",
                )}
                descriptionMode={descriptionMode}
                grouped={grouped}
              >
                <Select
                  value={remotePreset}
                  options={remotePresetOptions}
                  onChange={handleRemotePresetChange}
                  isClearable={false}
                />
              </SettingContainer>

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
                    if (remotePreset === "custom") {
                      setCustomModelId(nextValue);
                    }
                  }}
                  onBlur={handleModelIdBlur}
                  placeholder={t("settings.advanced.remoteStt.modelId.placeholder")}
                  className="w-full"
                />
              </SettingContainer>
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
                  <Input
                    type="text"
                    value={customSonioxModelInput}
                    onChange={(event) =>
                      setCustomSonioxModelInput(event.target.value)
                    }
                    onBlur={handleCustomSonioxModelBlur}
                    placeholder="stt-rt-v5"
                    className="w-full"
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
                <Input
                  type="number"
                  value={sonioxTimeoutInput}
                  onChange={(event) => setSonioxTimeoutInput(event.target.value)}
                  onBlur={handleSonioxTimeoutBlur}
                  min={10}
                  max={300}
                  className="w-full"
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
                <Input
                  type="text"
                  value={sonioxLanguageHintsInput}
                  onChange={(event) =>
                    setSonioxLanguageHintsInput(event.target.value)
                  }
                  onBlur={handleSonioxLanguageHintsBlur}
                  placeholder={t("settings.advanced.soniox.languageHints.placeholder")}
                  className="w-full"
                  disabled={sonioxUseProfileLanguageHintOnly}
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
                    {["endpoint", "keepalive", "finalizeTimeout", "instantStop"].map((id) => (
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
                <Input
                  type="number"
                  value={sonioxMaxEndpointDelayMsInput}
                  onChange={(event) =>
                    setSonioxMaxEndpointDelayMsInput(event.target.value)
                  }
                  onBlur={handleSonioxMaxEndpointDelayBlur}
                  min={500}
                  max={3000}
                  className="w-full"
                  disabled={!sonioxRealtimeControlsEnabled}
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
                <Input
                  type="number"
                  value={sonioxKeepaliveSecondsInput}
                  onChange={(event) =>
                    setSonioxKeepaliveSecondsInput(event.target.value)
                  }
                  onBlur={handleSonioxKeepaliveBlur}
                  min={5}
                  max={20}
                  className="w-full"
                  disabled={!sonioxRealtimeControlsEnabled}
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
                <Input
                  type="number"
                  value={sonioxLiveFinalizeTimeoutInput}
                  onChange={(event) =>
                    setSonioxLiveFinalizeTimeoutInput(event.target.value)
                  }
                  onBlur={handleSonioxLiveFinalizeTimeoutBlur}
                  min={100}
                  max={20000}
                  className="w-full"
                  disabled={!sonioxRealtimeControlsEnabled}
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
                : t("settings.advanced.remoteStt.apiKey.title")
            }
            description={
              isSonioxProvider
                ? t("settings.advanced.soniox.apiKey.description")
                : isDeepgramProvider
                  ? t("settings.advanced.deepgram.apiKey.description")
                : t("settings.advanced.remoteStt.apiKey.description")
            }
            descriptionMode={descriptionMode}
            grouped={grouped}
            layout="stacked"
          >
            <div className="flex flex-col gap-3 rounded-lg border border-mid-gray/30 bg-mid-gray/5 p-3">
              {hasApiKey && !isEditingKey ? (
                <div className="flex flex-col gap-2">
                  <Input
                    type="text"
                    value="************************************************"
                    readOnly
                    className="text-green-400"
                  />
                  <div className="flex items-center gap-2 text-sm text-green-400">
                    <span className="inline-flex h-2 w-2 rounded-full bg-green-400" />
                    <span className="text-green-400">
                      {t("settings.advanced.remoteStt.apiKey.statusStored")}
                    </span>
                  </div>
                  <p className="text-xs text-text/60">
                    {t("settings.advanced.remoteStt.apiKey.statusStoredHint")}
                  </p>
                  <div className="flex items-center gap-2">
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={handleStartReplaceKey}
                      disabled={apiKeyLoading}
                    >
                      {t("settings.advanced.remoteStt.apiKey.replace")}
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={handleClearApiKey}
                      disabled={apiKeyLoading}
                    >
                      {t("settings.advanced.remoteStt.apiKey.clear")}
                    </Button>
                  </div>
                </div>
              ) : (
                <div className="flex flex-col gap-2">
                  <div className="flex items-center gap-2">
                    <Input
                      type="password"
                      value={apiKeyInput}
                      onChange={(event) => setApiKeyInput(event.target.value)}
                      placeholder={
                        isSonioxProvider
                          ? t("settings.advanced.soniox.apiKey.placeholder")
                          : isDeepgramProvider
                            ? t("settings.advanced.deepgram.apiKey.placeholder")
                          : t("settings.advanced.remoteStt.apiKey.placeholder")
                      }
                      disabled={apiKeyLoading}
                    />
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={handleSaveApiKey}
                      disabled={apiKeyLoading || !apiKeyInput.trim()}
                    >
                      {t("settings.advanced.remoteStt.apiKey.save")}
                    </Button>
                    {hasApiKey && (
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={handleCancelReplaceKey}
                        disabled={apiKeyLoading}
                      >
                        {t("settings.advanced.remoteStt.apiKey.cancel")}
                      </Button>
                    )}
                  </div>
                  <p className="text-xs text-text/60">
                    {hasApiKey
                      ? t("settings.advanced.remoteStt.apiKey.replaceHint")
                      : t("settings.advanced.remoteStt.apiKey.statusMissing")}
                  </p>
                </div>
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
