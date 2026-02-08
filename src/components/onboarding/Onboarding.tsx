import React, { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { type } from "@tauri-apps/plugin-os";
import { commands, type ModelInfo } from "@/bindings";
import ModelCard from "./ModelCard";
import HandyTextLogo from "../icons/HandyTextLogo";
import { RemoteSttWizard } from "./RemoteSttWizard";

interface OnboardingProps {
  onModelSelected: () => void;
  onRemoteSelected: () => void;
  showFullCatalog?: boolean;
}

const Onboarding: React.FC<OnboardingProps> = ({
  onModelSelected,
  onRemoteSelected,
  showFullCatalog = false,
}) => {
  const { t } = useTranslation();
  const isWindows = type() === "windows";
  const [availableModels, setAvailableModels] = useState<ModelInfo[]>([]);
  const [downloading, setDownloading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mode, setMode] = useState<"select" | "local">("select");
  const [showRemoteWizard, setShowRemoteWizard] = useState(false);

  useEffect(() => {
    loadModels();
  }, [showFullCatalog]);

  const loadModels = async () => {
    try {
      const result = await commands.getAvailableModels();
      if (result.status === "ok") {
        // First-run onboarding shows only downloadable models.
        // Debug re-run mode shows the full catalog.
        setAvailableModels(
          showFullCatalog ? result.data : result.data.filter((m) => !m.is_downloaded),
        );
      } else {
        setError(t("onboarding.errors.loadModels"));
      }
    } catch (err) {
      console.error("Failed to load models:", err);
      setError(t("onboarding.errors.loadModels"));
    }
  };

  const handleModelSelection = async (model: ModelInfo) => {
    if (model.is_downloaded) {
      try {
        const result = await commands.setActiveModel(model.id);
        if (result.status === "ok") {
          onModelSelected();
          return;
        }
        setError(t("onboarding.errors.selectModel", { error: result.error }));
      } catch (err) {
        setError(t("onboarding.errors.selectModel", { error: String(err) }));
      }
      return;
    }

    setDownloading(true);
    setError(null);

    // Immediately transition to main app - download will continue in footer
    onModelSelected();

    try {
      const result = await commands.downloadModel(model.id);
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
      // Show wizard instead of navigating directly
      setShowRemoteWizard(true);
    } catch (err) {
      console.error("Failed to select remote mode:", err);
      setError(t("onboarding.errors.selectRemote"));
    }
  };

  const handleRemoteWizardComplete = () => {
    setShowRemoteWizard(false);
    onRemoteSelected();
  };

  const handleRemoteWizardClose = () => {
    setShowRemoteWizard(false);
  };

  return (
    <div className="min-h-screen w-screen flex flex-col p-8 gap-6 inset-0 bg-gradient-to-br from-[#1e1e1e] via-[#222222] to-[#1a1a1a]">
      <div className="flex flex-col items-center gap-3 shrink-0">
        <HandyTextLogo width={220} className="drop-shadow-[0_0_20px_rgba(255,107,157,0.4)]" />
        <p className="text-[#a0a0a0] max-w-md font-medium mx-auto text-center">
          {mode === "select"
            ? t("onboarding.mode.subtitle")
            : t("onboarding.subtitle")}
        </p>
      </div>

      <div className="max-w-[600px] w-full mx-auto text-center flex-1 flex flex-col min-h-0">
        {error && (
          <div className="bg-[#ff453a]/10 border border-[#ff453a]/30 rounded-xl p-4 mb-4 shrink-0 backdrop-blur-sm">
            <p className="text-[#ff453a] text-sm font-medium">{error}</p>
          </div>
        )}

        <div className="flex flex-col gap-4 ">
          {mode === "select" ? (
            <div className="flex flex-col gap-4">
              <button
                className="glass-panel-interactive flex justify-between items-center rounded-xl p-5 text-left group"
                onClick={handleSelectLocal}
              >
                <div>
                  <h3 className="text-lg font-semibold text-[#f5f5f5] group-hover:text-[#ff4d8d] transition-colors">
                    {t("onboarding.mode.local.title")}
                  </h3>
                  <p className="text-[#a0a0a0] text-sm mt-1">
                    {t("onboarding.mode.local.description")}
                  </p>
                </div>
                <div className="w-10 h-10 rounded-full bg-[#9b5de5] flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity">
                  <svg className="w-5 h-5 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                  </svg>
                </div>
              </button>
              <button
                className={`glass-panel-interactive flex justify-between items-center rounded-xl p-5 text-left group ${
                  !isWindows ? "opacity-40 cursor-not-allowed" : ""
                }`}
                onClick={handleSelectRemote}
                disabled={!isWindows}
              >
                <div>
                  <h3 className="text-lg font-semibold text-[#f5f5f5] group-hover:text-[#ff4d8d] transition-colors">
                    {t("onboarding.mode.remote.title")}
                  </h3>
                  <p className="text-[#a0a0a0] text-sm mt-1">
                    {t("onboarding.mode.remote.description")}
                  </p>
                  {!isWindows && (
                    <p className="text-xs text-[#6b6b6b] mt-2">
                      {t("onboarding.mode.remote.windowsOnly")}
                    </p>
                  )}
                </div>
                {isWindows && (
                  <div className="w-10 h-10 rounded-full bg-[#9b5de5] flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity">
                    <svg className="w-5 h-5 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                    </svg>
                  </div>
                )}
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
                    onSelect={handleModelSelection}
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
                    onSelect={handleModelSelection}
                  />
                ))}
            </>
          )}
        </div>
      </div>
      {/* Remote STT Configuration Wizard */}
      <RemoteSttWizard
        isOpen={showRemoteWizard}
        onClose={handleRemoteWizardClose}
        onComplete={handleRemoteWizardComplete}
      />
    </div>
  );
};

export default Onboarding;
