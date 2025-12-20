import React, { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { type } from "@tauri-apps/plugin-os";
import { commands, type ModelInfo } from "@/bindings";
import ModelCard from "./ModelCard";
import HandyTextLogo from "../icons/HandyTextLogo";

interface OnboardingProps {
  onModelSelected: () => void;
  onRemoteSelected: () => void;
}

const Onboarding: React.FC<OnboardingProps> = ({
  onModelSelected,
  onRemoteSelected,
}) => {
  const { t } = useTranslation();
  const isWindows = type() === "windows";
  const [availableModels, setAvailableModels] = useState<ModelInfo[]>([]);
  const [downloading, setDownloading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mode, setMode] = useState<"select" | "local">("select");

  useEffect(() => {
    loadModels();
  }, []);

  const loadModels = async () => {
    try {
      const result = await commands.getAvailableModels();
      if (result.status === "ok") {
        // Only show downloadable models for onboarding
        setAvailableModels(result.data.filter((m) => !m.is_downloaded));
      } else {
        setError(t("onboarding.errors.loadModels"));
      }
    } catch (err) {
      console.error("Failed to load models:", err);
      setError(t("onboarding.errors.loadModels"));
    }
  };

  const handleDownloadModel = async (modelId: string) => {
    setDownloading(true);
    setError(null);

    // Immediately transition to main app - download will continue in footer
    onModelSelected();

    try {
      const result = await commands.downloadModel(modelId);
      if (result.status === "error") {
        console.error("Download failed:", result.error);
        setError(t("onboarding.errors.downloadModel", { error: result.error }));
        setDownloading(false);
      }
    } catch (err) {
      console.error("Download failed:", err);
      setError(t("onboarding.errors.downloadModel", { error: String(err) }));
      setDownloading(false);
    }
  };

  const getRecommendedBadge = (modelId: string): boolean => {
    return modelId === "parakeet-tdt-0.6b-v3";
  };

  const handleSelectLocal = async () => {
    try {
      await commands.changeTranscriptionProviderSetting("local");
      setMode("local");
    } catch (err) {
      console.error("Failed to select local mode:", err);
      setError(t("onboarding.errors.selectLocal"));
    }
  };

  const handleSelectRemote = async () => {
    if (!isWindows) return;
    try {
      await commands.changeTranscriptionProviderSetting(
        "remote_openai_compatible",
      );
      onRemoteSelected();
    } catch (err) {
      console.error("Failed to select remote mode:", err);
      setError(t("onboarding.errors.selectRemote"));
    }
  };

  return (
    <div className="h-screen w-screen flex flex-col p-6 gap-4 inset-0">
      <div className="flex flex-col items-center gap-2 shrink-0">
        <HandyTextLogo width={200} />
        <p className="text-text/70 max-w-md font-medium mx-auto">
          {mode === "select"
            ? t("onboarding.mode.subtitle")
            : t("onboarding.subtitle")}
        </p>
      </div>

      <div className="max-w-[600px] w-full mx-auto text-center flex-1 flex flex-col min-h-0">
        {error && (
          <div className="bg-red-500/10 border border-red-500/20 rounded-lg p-4 mb-4 shrink-0">
            <p className="text-red-400 text-sm">{error}</p>
          </div>
        )}

        <div className="flex flex-col gap-4 ">
          {mode === "select" ? (
            <div className="flex flex-col gap-3">
              <button
                className="flex justify-between items-center rounded-xl p-4 text-left transition-all duration-200 border-2 border-mid-gray/20 hover:border-logo-primary/50 hover:bg-logo-primary/5 hover:shadow-lg hover:scale-[1.02]"
                onClick={handleSelectLocal}
              >
                <div>
                  <h3 className="text-lg font-semibold text-text">
                    {t("onboarding.mode.local.title")}
                  </h3>
                  <p className="text-text/70 text-sm">
                    {t("onboarding.mode.local.description")}
                  </p>
                </div>
              </button>
              <button
                className={`flex justify-between items-center rounded-xl p-4 text-left transition-all duration-200 border-2 ${
                  isWindows
                    ? "border-mid-gray/20 hover:border-logo-primary/50 hover:bg-logo-primary/5 hover:shadow-lg hover:scale-[1.02]"
                    : "border-mid-gray/10 opacity-60 cursor-not-allowed"
                }`}
                onClick={handleSelectRemote}
                disabled={!isWindows}
              >
                <div>
                  <h3 className="text-lg font-semibold text-text">
                    {t("onboarding.mode.remote.title")}
                  </h3>
                  <p className="text-text/70 text-sm">
                    {t("onboarding.mode.remote.description")}
                  </p>
                  {!isWindows && (
                    <p className="text-xs text-text/60 mt-1">
                      {t("onboarding.mode.remote.windowsOnly")}
                    </p>
                  )}
                </div>
              </button>
            </div>
          ) : (
            <>
              {availableModels
                .filter((model) => getRecommendedBadge(model.id))
                .map((model) => (
                  <ModelCard
                    key={model.id}
                    model={model}
                    variant="featured"
                    disabled={downloading}
                    onSelect={handleDownloadModel}
                  />
                ))}

              {availableModels
                .filter((model) => !getRecommendedBadge(model.id))
                .sort((a, b) => Number(a.size_mb) - Number(b.size_mb))
                .map((model) => (
                  <ModelCard
                    key={model.id}
                    model={model}
                    disabled={downloading}
                    onSelect={handleDownloadModel}
                  />
                ))}
            </>
          )}
        </div>
      </div>
    </div>
  );
};

export default Onboarding;
