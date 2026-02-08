import React, { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { type } from "@tauri-apps/plugin-os";
import { toast } from "sonner";
import { commands } from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { Button } from "../../ui/Button";
import { Input } from "../../ui/Input";
import { Select, type SelectOption } from "../../ui/Select";
import { SettingContainer } from "../../ui/SettingContainer";
import { Textarea } from "../../ui/Textarea";
import { ToggleSwitch } from "../../ui/ToggleSwitch";

interface RemoteSttSettingsProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const RemoteSttSettings: React.FC<RemoteSttSettingsProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const isWindows = type() === "windows";
  const {
    settings,
    isUpdating,
    updateSetting,
    setTranscriptionProvider,
    updateRemoteSttBaseUrl,
    updateRemoteSttModelId,
    updateRemoteSttDebugCapture,
    updateRemoteSttDebugMode,
  } = useSettings();

  const provider = settings?.transcription_provider ?? "local";
  const remoteSettings = settings?.remote_stt;
  const sonioxModel = (settings as any)?.soniox_model ?? "stt-rt-v4";
  const sonioxTimeout = Number((settings as any)?.soniox_timeout_seconds ?? 30);
  const isRemoteOpenAiProvider = provider === "remote_openai_compatible";
  const isSonioxProvider = provider === "remote_soniox";
  const isCloudProvider = isRemoteOpenAiProvider || isSonioxProvider;

  const [baseUrlInput, setBaseUrlInput] = useState(
    remoteSettings?.base_url ?? "",
  );
  const [modelIdInput, setModelIdInput] = useState(
    remoteSettings?.model_id ?? "",
  );
  const [sonioxModelInput, setSonioxModelInput] = useState(sonioxModel);
  const [sonioxTimeoutInput, setSonioxTimeoutInput] = useState(
    String(sonioxTimeout),
  );

  const [apiKeyInput, setApiKeyInput] = useState("");
  const [hasApiKey, setHasApiKey] = useState(false);
  const [apiKeyLoading, setApiKeyLoading] = useState(false);
  const [isEditingKey, setIsEditingKey] = useState(false);
  const [hasKeyStatusLoaded, setHasKeyStatusLoaded] = useState(false);

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
    setBaseUrlInput(remoteSettings?.base_url ?? "");
  }, [remoteSettings?.base_url]);

  useEffect(() => {
    setModelIdInput(remoteSettings?.model_id ?? "");
  }, [remoteSettings?.model_id]);

  useEffect(() => {
    setSonioxModelInput(sonioxModel);
  }, [sonioxModel]);

  useEffect(() => {
    setSonioxTimeoutInput(String(sonioxTimeout));
  }, [sonioxTimeout]);

  useEffect(() => {
    if (!isWindows) {
      setHasApiKey(false);
      setHasKeyStatusLoaded(true);
      return;
    }

    const loadApiKeyStatus = async () => {
      try {
        const result = isSonioxProvider
          ? await commands.sonioxHasApiKey()
          : await commands.remoteSttHasApiKey();
        if (result.status === "ok") {
          setHasApiKey(result.data);
        }
      } catch (error) {
        console.error("Failed to check API key status:", error);
      } finally {
        setHasKeyStatusLoaded(true);
      }
    };

    loadApiKeyStatus();
  }, [isWindows, provider, isSonioxProvider]);

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
    ];
  }, [t, isWindows]);

  const handleProviderChange = (value: string | null) => {
    if (!value) return;
    void setTranscriptionProvider(value);
  };

  const handleBaseUrlBlur = () => {
    const trimmed = baseUrlInput.trim();
    if (trimmed !== (remoteSettings?.base_url ?? "")) {
      void updateRemoteSttBaseUrl(trimmed);
    }
  };

  const handleModelIdBlur = () => {
    const trimmed = modelIdInput.trim();
    if (trimmed !== (remoteSettings?.model_id ?? "")) {
      void updateRemoteSttModelId(trimmed);
    }
  };

  const handleSonioxModelBlur = () => {
    const trimmed = sonioxModelInput.trim();
    if (trimmed !== sonioxModel) {
      void updateSetting("soniox_model" as any, trimmed || "stt-rt-v4");
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

  const handleSaveApiKey = async () => {
    if (!apiKeyInput.trim()) return;
    setApiKeyLoading(true);
    try {
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
    } catch (error) {
      toast.error(String(error));
    } finally {
      setApiKeyLoading(false);
    }
  };

  const handleClearApiKey = async () => {
    setApiKeyLoading(true);
    try {
      const result = isSonioxProvider
        ? await commands.sonioxClearApiKey()
        : await commands.remoteSttClearApiKey();
      if (result.status === "ok") {
        setHasApiKey(false);
        setApiKeyInput("");
      } else {
        toast.error(result.error);
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
  const canTestConnection =
    isRemoteOpenAiProvider &&
    hasApiKey &&
    baseUrlInput.trim().length > 0 &&
    !apiKeyLoading;

  return (
    <div className="space-y-2">
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

      {showRemoteFields && (
        <>
          {showOpenAiFields && (
            <>
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
                  onBlur={handleBaseUrlBlur}
                  placeholder={t("settings.advanced.remoteStt.baseUrl.placeholder")}
                  className="w-full"
                />
              </SettingContainer>

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
                  onChange={(event) => setModelIdInput(event.target.value)}
                  onBlur={handleModelIdBlur}
                  placeholder={t("settings.advanced.remoteStt.modelId.placeholder")}
                  className="w-full"
                />
              </SettingContainer>
            </>
          )}

          {showSonioxFields && (
            <>
              <SettingContainer
                title={t("settings.advanced.soniox.model.title")}
                description={t("settings.advanced.soniox.model.description")}
                descriptionMode={descriptionMode}
                grouped={grouped}
                layout="stacked"
              >
                <Input
                  type="text"
                  value={sonioxModelInput}
                  onChange={(event) => setSonioxModelInput(event.target.value)}
                  onBlur={handleSonioxModelBlur}
                  placeholder={t("settings.advanced.soniox.model.placeholder")}
                  className="w-full"
                />
              </SettingContainer>

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
            </>
          )}

          <SettingContainer
            title={
              isSonioxProvider
                ? t("settings.advanced.soniox.apiKey.title")
                : t("settings.advanced.remoteStt.apiKey.title")
            }
            description={
              isSonioxProvider
                ? t("settings.advanced.soniox.apiKey.description")
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
                      placeholder={t(
                        "settings.advanced.remoteStt.apiKey.placeholder",
                      )}
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
