import React, { useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { X, Cloud, ExternalLink } from "lucide-react";
import { toast } from "sonner";
import { commands } from "@/bindings";
import { Button } from "../ui/Button";
import { Input } from "../ui/Input";
import { Select, type SelectOption } from "../ui/Select";
import { useSettings } from "../../hooks/useSettings";
import {
  REMOTE_STT_PRESETS,
  type RemoteSttPreset,
} from "../../lib/constants/remoteSttProviders";

interface RemoteSttWizardProps {
  isOpen: boolean;
  onClose: () => void;
  onComplete: () => void;
}

type EngineType = "openai" | "soniox" | "deepgram";

const resetConnectionState = (
  setConnectionStatus: React.Dispatch<
    React.SetStateAction<"idle" | "testing" | "success" | "error">
  >,
  setConnectionMessage: React.Dispatch<React.SetStateAction<string | null>>,
) => {
  setConnectionStatus("idle");
  setConnectionMessage(null);
};

export const RemoteSttWizard: React.FC<RemoteSttWizardProps> = ({
  isOpen,
  onClose,
  onComplete,
}) => {
  const { t } = useTranslation();
  const {
    updateRemoteSttBaseUrl,
    updateRemoteSttModelId,
    setTranscriptionProvider,
  } = useSettings();

  const [engine, setEngine] = useState<EngineType>("openai");
  const [remotePreset, setRemotePreset] = useState<RemoteSttPreset>("groq");
  const [baseUrl, setBaseUrl] = useState<string>(
    REMOTE_STT_PRESETS.groq.baseUrl,
  );
  const [modelId, setModelId] = useState<string>(
    REMOTE_STT_PRESETS.groq.defaultModel,
  );
  const [customBaseUrl, setCustomBaseUrl] = useState("");
  const [customModelId, setCustomModelId] = useState("");
  const [allowInsecureHttp, setAllowInsecureHttp] = useState(false);
  const [apiKey, setApiKey] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [connectionStatus, setConnectionStatus] = useState<
    "idle" | "testing" | "success" | "error"
  >("idle");
  const [connectionMessage, setConnectionMessage] = useState<string | null>(
    null,
  );

  const engineOptions = useMemo<SelectOption[]>(
    () => [
      {
        value: "openai",
        label: t("onboarding.remoteSttWizard.engineOptions.openai"),
      },
      {
        value: "soniox",
        label: t("onboarding.remoteSttWizard.engineOptions.soniox"),
      },
      {
        value: "deepgram",
        label: t("onboarding.remoteSttWizard.engineOptions.deepgram"),
      },
    ],
    [t],
  );

  const isSoniox = engine === "soniox";
  const isDeepgram = engine === "deepgram";
  const isCustomRemotePreset = remotePreset === "custom";
  const hasRequiredRemoteUrl =
    isSoniox || isDeepgram || !isCustomRemotePreset || baseUrl.trim().length > 0;

  if (!isOpen) return null;

  const handleSaveAndTest = async () => {
    if (!apiKey.trim()) return;
    if (!isSoniox && !isDeepgram && !baseUrl.trim()) return;

    setIsLoading(true);
    setConnectionStatus("testing");
    setConnectionMessage(null);

    try {
      if (isSoniox) {
        // Save Soniox API key
        const keyResult = await commands.sonioxSetApiKey(apiKey.trim());
        if (keyResult.status === "error") {
          throw new Error(keyResult.error);
        }
        // Soniox doesn't have a test connection command in the wizard.
        setConnectionStatus("success");
        setConnectionMessage(
          t("onboarding.remoteSttWizard.apiKeySaved", "API key saved."),
        );
      } else if (isDeepgram) {
        await invoke("deepgram_set_api_key", { apiKey: apiKey.trim() });
        // Deepgram wizard currently does a save-only check, not a network test.
        setConnectionStatus("success");
        setConnectionMessage(
          t("onboarding.remoteSttWizard.apiKeySaved", "API key saved."),
        );
      } else {
        await invoke("change_remote_stt_provider_preset_setting", {
          preset: remotePreset,
        });
        await invoke("change_remote_stt_allow_insecure_http_setting", {
          enabled: allowInsecureHttp,
        });

        if (isCustomRemotePreset) {
          await updateRemoteSttBaseUrl(baseUrl.trim());
        }

        // Save model ID
        await updateRemoteSttModelId(modelId.trim());

        // Save API key
        const keyResult = await commands.remoteSttSetApiKey(apiKey.trim());
        if (keyResult.status === "error") {
          throw new Error(keyResult.error);
        }

        // Test connection
        const testResult = await commands.remoteSttTestConnection(
          baseUrl.trim(),
        );
        if (testResult.status === "ok") {
          setConnectionStatus("success");
          setConnectionMessage(
            t("onboarding.remoteSttWizard.connectionSuccess"),
          );
        } else {
          setConnectionStatus("error");
          setConnectionMessage(
            t("onboarding.remoteSttWizard.connectionFailed", {
              error: testResult.error,
            }),
          );
        }
      }
    } catch (error) {
      setConnectionStatus("error");
      setConnectionMessage(String(error));
      toast.error(String(error));
    } finally {
      setIsLoading(false);
    }
  };

  const handleFinish = async () => {
    if (!hasRequiredRemoteUrl) {
      const message = t(
        "onboarding.remoteSttWizard.baseUrlRequired",
        "Enter a base URL before finishing setup.",
      );
      setConnectionStatus("error");
      setConnectionMessage(message);
      toast.error(message);
      return;
    }

    // If not tested yet, save settings first
    if (connectionStatus !== "success" && apiKey.trim()) {
      setIsLoading(true);
      try {
        if (isSoniox) {
          await commands.sonioxSetApiKey(apiKey.trim());
        } else if (isDeepgram) {
          await invoke("deepgram_set_api_key", { apiKey: apiKey.trim() });
        } else if (baseUrl.trim() || !isCustomRemotePreset) {
          await invoke("change_remote_stt_provider_preset_setting", {
            preset: remotePreset,
          });
          await invoke("change_remote_stt_allow_insecure_http_setting", {
            enabled: allowInsecureHttp,
          });
          if (isCustomRemotePreset) {
            await updateRemoteSttBaseUrl(baseUrl.trim());
          }
          // Save model ID
          await updateRemoteSttModelId(modelId.trim());
          // Save API key
          await commands.remoteSttSetApiKey(apiKey.trim());
        }
      } catch (error) {
        toast.error(String(error));
        setConnectionStatus("error");
        setConnectionMessage(String(error));
        return;
      } finally {
        setIsLoading(false);
      }
    }

    // Set the transcription provider based on engine selection
    try {
      await setTranscriptionProvider(
        isSoniox
          ? "remote_soniox"
          : isDeepgram
            ? "remote_deepgram"
            : "remote_openai_compatible",
      );
    } catch (error) {
      toast.error(String(error));
      return;
    }

    onComplete();
  };

  const canTest =
    apiKey.trim().length > 0 && hasRequiredRemoteUrl;
  const canFinish = apiKey.trim().length > 0 && hasRequiredRemoteUrl;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center overflow-y-auto py-4"
      onClick={onClose}
    >
      {/* Backdrop */}
      <div className="fixed inset-0 bg-black/50" />

      {/* Modal */}
      <div
        className="relative z-10 w-full max-w-lg mx-4 my-auto bg-gradient-to-br from-[#1e1e1e] via-[#222222] to-[#1a1a1a] border border-purple-500/30 rounded-2xl shadow-2xl animate-in fade-in zoom-in-95 duration-200"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Close button */}
        <button
          onClick={onClose}
          className="absolute top-4 right-4 p-1.5 rounded-md text-text/60 hover:text-text hover:bg-mid-gray/20 transition-colors"
        >
          <X className="w-5 h-5" />
        </button>

        {/* Header */}
        <div className="p-6 pb-4 border-b border-mid-gray/20">
          <div className="flex items-center gap-3">
            <div className="p-2.5 rounded-xl bg-purple-500/20">
              <Cloud className="w-6 h-6 text-purple-400" />
            </div>
            <div>
              <h2 className="text-xl font-semibold text-text">
                {t("onboarding.remoteSttWizard.title")}
              </h2>
              <p className="text-sm text-mid-gray mt-0.5">
                {t("onboarding.remoteSttWizard.subtitle")}
              </p>
            </div>
          </div>
        </div>

        {/* Content */}
        <div className="p-6 space-y-5">
          {/* Engine selector */}
          <div className="space-y-2">
            <label className="text-sm font-medium text-text/80">
              {t("onboarding.remoteSttWizard.engineLabel")}
            </label>
            <Select
              value={engine}
              options={engineOptions}
              onChange={(value) => {
                if (value) {
                  setEngine(value as EngineType);
                  resetConnectionState(
                    setConnectionStatus,
                    setConnectionMessage,
                  );
                }
              }}
              isClearable={false}
            />
          </div>

          {/* Info box */}
          {isSoniox ? (
            <>
              <div className="p-4 bg-purple-500/10 border border-purple-500/30 rounded-xl text-sm space-y-2">
                <p className="text-text/90">
                  {t("onboarding.remoteSttWizard.sonioxInfo")}
                </p>
                <a
                  href="https://soniox.com"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-1.5 text-purple-400 hover:text-purple-300 font-medium transition-colors"
                >
                  soniox.com
                  <ExternalLink className="w-3.5 h-3.5" />
                </a>
              </div>
              <div className="p-3 bg-yellow-500/10 border border-yellow-500/30 rounded-lg text-sm text-yellow-400">
                {t("onboarding.remoteSttWizard.sonioxRealtimeWarning")}
              </div>
            </>
          ) : isDeepgram ? (
            <>
              <div className="p-4 bg-purple-500/10 border border-purple-500/30 rounded-xl text-sm space-y-2">
                <p className="text-text/90">
                  {t("onboarding.remoteSttWizard.deepgramInfo")}
                </p>
                <a
                  href="https://console.deepgram.com"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-1.5 text-purple-400 hover:text-purple-300 font-medium transition-colors"
                >
                  console.deepgram.com
                  <ExternalLink className="w-3.5 h-3.5" />
                </a>
                <p className="text-text/60 text-xs">
                  {t("onboarding.remoteSttWizard.deepgramRecommendation")}
                </p>
              </div>
              <div className="p-3 bg-yellow-500/10 border border-yellow-500/30 rounded-lg text-sm text-yellow-400">
                {t("onboarding.remoteSttWizard.deepgramRealtimeWarning")}
              </div>
            </>
          ) : (
            <>
              <div className="p-4 bg-purple-500/10 border border-purple-500/30 rounded-xl text-sm space-y-2">
                <p className="text-text/90">
                  {t("onboarding.remoteSttWizard.recommendation")}
                </p>
                <a
                  href="https://console.groq.com"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-1.5 text-purple-400 hover:text-purple-300 font-medium transition-colors"
                >
                  console.groq.com
                  <ExternalLink className="w-3.5 h-3.5" />
                </a>
                <p className="text-text/60 text-xs">
                  {t("onboarding.remoteSttWizard.freeTier")}
                </p>
              </div>
              <div className="p-3 bg-[#ff4d8d]/10 border border-[#ff4d8d]/30 rounded-lg text-sm text-[#ffd1e6]">
                {t("onboarding.remoteSttWizard.sonioxRecommendation")}
              </div>
            </>
          )}

          {/* API Configuration section header */}
          <div className="border-t border-mid-gray/20 pt-4">
            <h3 className="text-sm font-semibold text-text/70 uppercase tracking-wide">
              {t("onboarding.remoteSttWizard.apiConfigHeader")}
            </h3>
          </div>

          {/* OpenAI-specific fields */}
          {!isSoniox && !isDeepgram && (
            <>
              <div className="space-y-2">
                <label className="text-sm font-medium text-text/80">
                  {t("onboarding.remoteSttWizard.providerPreset.label")}
                </label>
                <Select
                  value={remotePreset}
                  options={[
                    {
                      value: "groq",
                      label: t(
                        "onboarding.remoteSttWizard.providerPreset.options.groq",
                      ),
                    },
                    {
                      value: "openai",
                      label: t(
                        "onboarding.remoteSttWizard.providerPreset.options.openai",
                      ),
                    },
                    {
                      value: "custom",
                      label: t(
                        "onboarding.remoteSttWizard.providerPreset.options.custom",
                      ),
                    },
                  ]}
                  onChange={(value) => {
                    if (!value) return;
                    const preset = value as RemoteSttPreset;
                    const previousPreset = remotePreset;
                    const nextCustomBaseUrl =
                      previousPreset === "custom" ? baseUrl : customBaseUrl;
                    const nextCustomModelId =
                      previousPreset === "custom"
                        ? modelId
                        : customModelId || modelId;

                    if (previousPreset === "custom") {
                      setCustomBaseUrl(baseUrl);
                      setCustomModelId(modelId);
                    }

                    setRemotePreset(preset);
                    resetConnectionState(
                      setConnectionStatus,
                      setConnectionMessage,
                    );

                    if (preset !== "custom") {
                      setAllowInsecureHttp(false);
                      setBaseUrl(REMOTE_STT_PRESETS[preset].baseUrl);
                      setModelId(REMOTE_STT_PRESETS[preset].defaultModel);
                    } else {
                      setBaseUrl(nextCustomBaseUrl);
                      setModelId(nextCustomModelId);
                    }
                  }}
                  isClearable={false}
                />
              </div>

              {/* Base URL input */}
              <div className="space-y-2">
                <label className="text-sm font-medium text-text/80">
                  {t("onboarding.remoteSttWizard.baseUrl.label")}
                </label>
                <Input
                  type="text"
                  value={baseUrl}
                  onChange={(e) => {
                    const nextValue = e.target.value;
                    setBaseUrl(nextValue);
                    if (isCustomRemotePreset) {
                      setCustomBaseUrl(nextValue);
                    }
                    resetConnectionState(
                      setConnectionStatus,
                      setConnectionMessage,
                    );
                  }}
                  placeholder={REMOTE_STT_PRESETS.groq.baseUrl}
                  disabled={isLoading}
                  readOnly={!isCustomRemotePreset}
                />
                <p className="text-xs text-mid-gray">
                  {t("onboarding.remoteSttWizard.baseUrl.hint")}
                </p>
              </div>

              {isCustomRemotePreset ? (
                <div className="space-y-2">
                  <label className="flex items-start gap-3 rounded-lg border border-mid-gray/20 bg-mid-gray/10 p-3 text-sm text-text/90">
                    <input
                      type="checkbox"
                      className="mt-1"
                      checked={allowInsecureHttp}
                      onChange={(event) => {
                        setAllowInsecureHttp(event.target.checked);
                        resetConnectionState(
                          setConnectionStatus,
                          setConnectionMessage,
                        );
                      }}
                      disabled={isLoading}
                    />
                    <span>
                      {t("onboarding.remoteSttWizard.customHttpOverride.label")}
                    </span>
                  </label>
                  {allowInsecureHttp ? (
                    <div className="rounded-lg border border-red-500/40 bg-red-500/10 p-3 text-sm text-red-200">
                      {t("onboarding.remoteSttWizard.customHttpOverride.warning")}
                    </div>
                  ) : null}
                </div>
              ) : null}

              {/* Model ID input */}
              <div className="space-y-2">
                <label className="text-sm font-medium text-text/80">
                  {t("onboarding.remoteSttWizard.modelId.label")}
                </label>
                <Input
                  type="text"
                  value={modelId}
                  onChange={(e) => {
                    const nextValue = e.target.value;
                    setModelId(nextValue);
                    if (isCustomRemotePreset) {
                      setCustomModelId(nextValue);
                    }
                    resetConnectionState(
                      setConnectionStatus,
                      setConnectionMessage,
                    );
                  }}
                  placeholder={REMOTE_STT_PRESETS.groq.defaultModel}
                  disabled={isLoading}
                />
                <p className="text-xs text-mid-gray">
                  {t("onboarding.remoteSttWizard.modelId.hint")}
                </p>
              </div>
            </>
          )}

          {/* API Key input */}
          <div className="space-y-2">
            <label className="text-sm font-medium text-text/80">
              {t("onboarding.remoteSttWizard.apiKey.label")}
            </label>
            <Input
              type="password"
              value={apiKey}
              onChange={(e) => {
                setApiKey(e.target.value);
                resetConnectionState(
                  setConnectionStatus,
                  setConnectionMessage,
                );
              }}
              placeholder={isDeepgram ? "dg-..." : "sk-..."}
              disabled={isLoading}
            />
            <p className="text-xs text-mid-gray">
              {t("onboarding.remoteSttWizard.apiKey.hint")}
            </p>
          </div>

          {/* Connection status */}
          {connectionMessage && (
            <div
              className={`p-3 rounded-lg text-sm ${
                connectionStatus === "success"
                  ? "bg-green-500/10 border border-green-500/30 text-green-400"
                  : "bg-red-500/10 border border-red-500/30 text-red-400"
              }`}
            >
              {connectionMessage}
            </div>
          )}
        </div>

        {/* Actions */}
        <div className="p-6 pt-4 border-t border-mid-gray/20 flex items-center justify-between gap-3">
          <Button
            variant="secondary"
            onClick={handleSaveAndTest}
            disabled={!canTest || isLoading}
          >
            {connectionStatus === "testing"
              ? isSoniox || isDeepgram
                ? t(
                    "onboarding.remoteSttWizard.savingApiKey",
                    "Saving API key...",
                  )
                : t("onboarding.remoteSttWizard.testing")
              : isSoniox || isDeepgram
                ? t("onboarding.remoteSttWizard.saveApiKey")
                : t("onboarding.remoteSttWizard.testConnection")}
          </Button>

          <Button
            variant="primary"
            onClick={handleFinish}
            disabled={!canFinish || isLoading}
          >
            {t("onboarding.remoteSttWizard.finishConfig")}
          </Button>
        </div>
      </div>
    </div>
  );
};
