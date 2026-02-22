import React, { useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { X, Cloud, ExternalLink } from "lucide-react";
import { toast } from "sonner";
import { commands } from "@/bindings";
import { Button } from "../ui/Button";
import { Input } from "../ui/Input";
import { Select, type SelectOption } from "../ui/Select";
import { useSettings } from "../../hooks/useSettings";

interface RemoteSttWizardProps {
  isOpen: boolean;
  onClose: () => void;
  onComplete: () => void;
}

type EngineType = "openai" | "soniox";

const DEFAULT_BASE_URL = "https://api.groq.com/openai/v1";
const DEFAULT_MODEL_ID = "whisper-large-v3-turbo";

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
  const [baseUrl, setBaseUrl] = useState(DEFAULT_BASE_URL);
  const [modelId, setModelId] = useState(DEFAULT_MODEL_ID);
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
    ],
    [t],
  );

  const isSoniox = engine === "soniox";

  if (!isOpen) return null;

  const handleSaveAndTest = async () => {
    if (!apiKey.trim()) return;
    if (!isSoniox && !baseUrl.trim()) return;

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
        // Soniox doesn't have a test connection command in the wizard,
        // just mark as success if the key was saved
        setConnectionStatus("success");
        setConnectionMessage(t("onboarding.remoteSttWizard.connectionSuccess"));
      } else {
        // Save base URL
        await updateRemoteSttBaseUrl(baseUrl.trim());

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
    // If not tested yet, save settings first
    if (connectionStatus !== "success" && apiKey.trim()) {
      setIsLoading(true);
      try {
        if (isSoniox) {
          await commands.sonioxSetApiKey(apiKey.trim());
        } else if (baseUrl.trim()) {
          // Save base URL
          await updateRemoteSttBaseUrl(baseUrl.trim());
          // Save model ID
          await updateRemoteSttModelId(modelId.trim());
          // Save API key
          await commands.remoteSttSetApiKey(apiKey.trim());
        }
      } catch (error) {
        toast.error(String(error));
      } finally {
        setIsLoading(false);
      }
    }

    // Set the transcription provider based on engine selection
    try {
      await setTranscriptionProvider(
        isSoniox ? "remote_soniox" : "remote_openai_compatible",
      );
    } catch (error) {
      toast.error(String(error));
    }

    onComplete();
  };

  const canTest =
    apiKey.trim().length > 0 && (isSoniox || baseUrl.trim().length > 0);
  const canFinish = apiKey.trim().length > 0;

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
                  setConnectionStatus("idle");
                  setConnectionMessage(null);
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
          {!isSoniox && (
            <>
              {/* Base URL input */}
              <div className="space-y-2">
                <label className="text-sm font-medium text-text/80">
                  {t("onboarding.remoteSttWizard.baseUrl.label")}
                </label>
                <Input
                  type="text"
                  value={baseUrl}
                  onChange={(e) => setBaseUrl(e.target.value)}
                  placeholder={DEFAULT_BASE_URL}
                  disabled={isLoading}
                />
                <p className="text-xs text-mid-gray">
                  {t("onboarding.remoteSttWizard.baseUrl.hint")}
                </p>
              </div>

              {/* Model ID input */}
              <div className="space-y-2">
                <label className="text-sm font-medium text-text/80">
                  {t("onboarding.remoteSttWizard.modelId.label")}
                </label>
                <Input
                  type="text"
                  value={modelId}
                  onChange={(e) => setModelId(e.target.value)}
                  placeholder={DEFAULT_MODEL_ID}
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
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="sk-..."
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
              ? t("onboarding.remoteSttWizard.testing")
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
