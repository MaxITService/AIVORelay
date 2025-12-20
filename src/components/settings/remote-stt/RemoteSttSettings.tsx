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
    setTranscriptionProvider,
    updateRemoteSttBaseUrl,
    updateRemoteSttModelId,
    updateRemoteSttDebugCapture,
    updateRemoteSttDebugMode,
  } = useSettings();

  const provider = settings?.transcription_provider ?? "local";
  const remoteSettings = settings?.remote_stt;

  const [baseUrlInput, setBaseUrlInput] = useState(
    remoteSettings?.base_url ?? "",
  );
  const [modelIdInput, setModelIdInput] = useState(
    remoteSettings?.model_id ?? "",
  );

  const [apiKeyInput, setApiKeyInput] = useState("");
  const [hasApiKey, setHasApiKey] = useState(false);
  const [apiKeyLoading, setApiKeyLoading] = useState(false);

  const [debugLines, setDebugLines] = useState<string[]>([]);

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
    if (!isWindows) {
      setHasApiKey(false);
      return;
    }

    const loadApiKeyStatus = async () => {
      try {
        const result = await commands.remoteSttHasApiKey();
        if (result.status === "ok") {
          setHasApiKey(result.data);
        }
      } catch (error) {
        console.error("Failed to check API key status:", error);
      }
    };

    loadApiKeyStatus();
  }, [isWindows, provider]);

  useEffect(() => {
    if (!isWindows || provider !== "remote_openai_compatible") {
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
  }, [isWindows, provider, debugCap]);

  useEffect(() => {
    if (!isWindows || provider !== "remote_openai_compatible") {
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
  }, [isWindows, provider, debugCap]);

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

  const handleSaveApiKey = async () => {
    if (!apiKeyInput.trim()) return;
    setApiKeyLoading(true);
    try {
      const result = await commands.remoteSttSetApiKey(apiKeyInput.trim());
      if (result.status === "ok") {
        setApiKeyInput("");
        setHasApiKey(true);
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
      const result = await commands.remoteSttClearApiKey();
      if (result.status === "ok") {
        setHasApiKey(false);
      } else {
        toast.error(result.error);
      }
    } catch (error) {
      toast.error(String(error));
    } finally {
      setApiKeyLoading(false);
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
    isWindows && provider === "remote_openai_compatible";

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
            />
          </SettingContainer>

          <SettingContainer
            title={t("settings.advanced.remoteStt.apiKey.title")}
            description={t("settings.advanced.remoteStt.apiKey.description")}
            descriptionMode={descriptionMode}
            grouped={grouped}
            layout="stacked"
          >
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
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={handleClearApiKey}
                  disabled={apiKeyLoading || !hasApiKey}
                >
                  {t("settings.advanced.remoteStt.apiKey.clear")}
                </Button>
              </div>
              <p className="text-xs text-text/60">
                {hasApiKey
                  ? t("settings.advanced.remoteStt.apiKey.statusSet")
                  : t("settings.advanced.remoteStt.apiKey.statusMissing")}
              </p>
            </div>
          </SettingContainer>

          <ToggleSwitch
            checked={debugCapture}
            onChange={(enabled) => updateRemoteSttDebugCapture(enabled)}
            isUpdating={isUpdating("remote_stt_debug_capture")}
            label={t("settings.advanced.remoteStt.debug.capture.title")}
            description={t("settings.advanced.remoteStt.debug.capture.description")}
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
                  label: t("settings.advanced.remoteStt.debug.mode.options.normal"),
                },
                {
                  value: "verbose",
                  label: t("settings.advanced.remoteStt.debug.mode.options.verbose"),
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
    </div>
  );
};
