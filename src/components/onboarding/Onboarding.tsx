import React, { useState, useEffect, useRef, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { type } from "@tauri-apps/plugin-os";
import { ArrowRight, Sparkles } from "lucide-react";
import { commands, type ModelInfo } from "@/bindings";
import { invoke } from "@tauri-apps/api/core";
import {
  beginModelDownloadActivationIntent,
  cancelModelDownloadActivationIntent,
  invalidateModelDownloadActivationIntent,
} from "@/lib/modelDownloadActivation";
import ModelCard from "./ModelCard";
import HandyTextLogo from "../icons/HandyTextLogo";
import { RemoteSttWizard } from "./RemoteSttWizard";
import AccessibilityOnboarding from "./AccessibilityOnboarding";
import MacPermissionsOnboarding from "./MacPermissionsOnboarding";
import { clearOnboardingProgress, readOnboardingProgress, resolveOnboardingProgress, saveOnboardingProgress } from "./onboardingProgress";
import { HandyShortcut } from "../settings/HandyShortcut";
import { AiAssistTip } from "../shared/AiAssistTip";
import { useSettingsStore } from "@/stores/settingsStore";
import { getShortcutGuideMode } from "./shortcutGuide";
import { MODEL_CAPABILITY_LANGUAGES } from "@/lib/constants/languages";
import { readModelLanguagePreference, saveModelLanguagePreference, suggestedModelLanguages } from "./modelLanguages";
import { languageIdentity } from "@handy-computer/models";
import { recommendedModels } from "./modelRecommendations";

type WelcomeVariant = "local" | "remote";
type OnboardingMode = "permissions" | "select" | "local" | "welcome";

interface OnboardingProps {
  onModelSelected: () => void;
  onRemoteSelected: () => void;
  onSkipWizard: () => Promise<void>;
  showFullCatalog?: boolean;
  startWithPermissionStep?: boolean;
  onPermissionResolved?: () => void;
}

const Onboarding: React.FC<OnboardingProps> = ({
  onModelSelected,
  onRemoteSelected,
  onSkipWizard,
  showFullCatalog = false,
  startWithPermissionStep = false,
  onPermissionResolved,
}) => {
  const { t } = useTranslation();
  const settings = useSettingsStore((state) => state.settings);
  const shortcutMode = getShortcutGuideMode(settings);
  const isWindows = type() === "windows";
  const isMacOS = type() === "macos";
  const [savedProgress] = useState(() => showFullCatalog ? null : readOnboardingProgress());
  const needsPermissions = (startWithPermissionStep && isWindows) || isMacOS;
  const initialProgress = resolveOnboardingProgress(savedProgress, needsPermissions);
  const [availableModels, setAvailableModels] = useState<ModelInfo[]>([]);
  const [downloading, setDownloading] = useState(false);
  const downloadAttempt = useRef<{ dispatched: boolean } | null>(null);
  const [downloadingModelId, setDownloadingModelId] = useState<string | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);
  const [mode, setMode] = useState<OnboardingMode>(
    initialProgress.mode,
  );
  const [selectedModelId, setSelectedModelId] = useState<string | null>(savedProgress?.step === "local" ? savedProgress.modelId : null);
  const [welcomeVariant, setWelcomeVariant] = useState<WelcomeVariant>(initialProgress.welcomeVariant);
  const [showRemoteWizard, setShowRemoteWizard] = useState(initialProgress.showRemoteWizard);
  const [isSkipping, setIsSkipping] = useState(false);
  const [storedModelLanguages] = useState(readModelLanguagePreference);
  const [modelLanguages, setModelLanguages] = useState<string[]>(() =>
    storedModelLanguages ?? suggestedModelLanguages(typeof navigator === "undefined" ? [] : navigator.languages),
  );
  const modelLanguagesEdited = useRef(storedModelLanguages !== null);
  const changeModelLanguages = (next: string[]) => {
    modelLanguagesEdited.current = true;
    setModelLanguages(next);
    saveModelLanguagePreference(next);
  };
  const recommendations = useMemo(() => recommendedModels(availableModels, modelLanguages), [availableModels, modelLanguages]);

  useEffect(() => {
    let active = true;
    void invoke<string[]>("get_system_preferred_languages").then((locales) => {
      const suggestions = suggestedModelLanguages(locales);
      if (active && !modelLanguagesEdited.current && suggestions.length > 0) setModelLanguages(suggestions);
    }).catch((error) => console.warn("Could not read system language suggestions", error));
    return () => { active = false; };
  }, []);

  useEffect(() => {
    if (showFullCatalog || mode === "permissions") return;
    if (showRemoteWizard) saveOnboardingProgress({ step: "remote" });
    else if (mode === "welcome" && welcomeVariant === "remote") saveOnboardingProgress({ step: "remote_ready" });
    else if (mode === "local" || mode === "welcome") saveOnboardingProgress({ step: "local", modelId: selectedModelId });
    else saveOnboardingProgress({ step: "select" });
  }, [mode, showFullCatalog, showRemoteWizard, welcomeVariant, selectedModelId]);

  useEffect(() => {
    loadModels();
  }, [showFullCatalog]);

  useEffect(() => {
    if (!isWindows) return;
    if (startWithPermissionStep && isWindows) {
      setMode("permissions");
    } else if (mode === "permissions") {
      setMode("select");
    }
  }, [isWindows, mode, startWithPermissionStep]);

  const loadModels = async () => {
    try {
      const result = await commands.getAvailableModels();
      if (result.status === "ok") {
        // First-run onboarding shows only downloadable models.
        // Debug re-run mode shows the full catalog.
        setAvailableModels(
          showFullCatalog
            ? result.data
            : result.data.filter((m) => !m.is_downloaded || m.id === selectedModelId),
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
    setSelectedModelId(model.id);
    if (model.is_downloaded) {
      try {
        invalidateModelDownloadActivationIntent();
        const result = await commands.setActiveModel(model.id);
        if (result.status === "ok") {
          setWelcomeVariant("local");
          setMode("welcome");
          return;
        }
        setError(t("onboarding.errors.selectModel", { error: result.error }));
      } catch (err) {
        setError(t("onboarding.errors.selectModel", { error: String(err) }));
      }
      return;
    }

    const attempt = { dispatched: false };
    downloadAttempt.current = attempt;
    setDownloading(true);
    setDownloadingModelId(model.id);
    setError(null);

    // Start download in background, show welcome screen
    setWelcomeVariant("local");
    setMode("welcome");

    try {
      await beginModelDownloadActivationIntent(model.id);
      if (downloadAttempt.current !== attempt) return;
      attempt.dispatched = true;
      const result = await commands.downloadModel(model.id);
      if (downloadAttempt.current !== attempt) return;
      if (result.status === "error") {
        cancelModelDownloadActivationIntent(model.id);
        console.error("Download failed:", result.error);
        setDownloading(false);
        setDownloadingModelId(null);
        setMode("local");
      } else {
        setDownloading(false);
        setDownloadingModelId(null);
      }
    } catch (err) {
      if (downloadAttempt.current !== attempt) return;
      cancelModelDownloadActivationIntent(model.id);
      console.error("Download failed:", err);
      setDownloading(false);
      setDownloadingModelId(null);
      setMode("local");
    }
  };

  const handleCancelDownload = async () => {
    if (!downloadingModelId) return;

    if (!downloadAttempt.current?.dispatched) {
      downloadAttempt.current = null;
      invalidateModelDownloadActivationIntent();
      setDownloading(false);
      setDownloadingModelId(null);
      setSelectedModelId(null);
      setMode("local");
      return;
    }

    const result = await commands.cancelDownload(downloadingModelId);
    if (result.status === "error") {
      setError(result.error);
      return;
    }

    downloadAttempt.current = null;
    cancelModelDownloadActivationIntent(downloadingModelId);
    setDownloading(false);
    setDownloadingModelId(null);
    setSelectedModelId(null);
    setMode("local");
  };

  const getRecommendedBadge = (model: ModelInfo): boolean => {
    return recommendations.has(model.id);
  };

  const handleSelectLocal = async () => {
    try {
      invalidateModelDownloadActivationIntent();
      await commands.changeTranscriptionProviderSetting("local");
      setMode("local");
    } catch (err) {
      console.error("Failed to select local mode:", err);
      setError(t("onboarding.errors.selectLocal"));
    }
  };

  const handleSelectRemote = async () => {
    try {
      invalidateModelDownloadActivationIntent();
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
    setWelcomeVariant("remote");
    setMode("welcome");
  };

  const handleRemoteWizardClose = () => {
    setShowRemoteWizard(false);
  };

  const handlePermissionsComplete = () => {
    onPermissionResolved?.();
    const restored = resolveOnboardingProgress(savedProgress, false);
    setMode(restored.mode);
    setShowRemoteWizard(restored.showRemoteWizard);
  };

  const handleGetStarted = () => {
    if (!showFullCatalog) clearOnboardingProgress();
    if (welcomeVariant === "remote") {
      onRemoteSelected();
    } else {
      onModelSelected();
    }
  };

  const handleSkipWizard = async () => {
    if (isSkipping) return;
    setIsSkipping(true);
    try {
      await onSkipWizard();
    } finally {
      setIsSkipping(false);
    }
  };

  const getSubtitle = () => {
    if (mode === "welcome") return null;
    if (mode === "permissions") {
      if (isMacOS) return t("onboarding.macPermissions.subtitle", "Allow microphone and Accessibility access for dictation.");
      return t(
        "onboarding.permissions.subtitle",
        "Windows microphone access is needed before recording can work.",
      );
    }
    if (mode === "select") return t("onboarding.mode.subtitle");
    return t("onboarding.subtitle");
  };

  return (
    <div className="h-screen w-full flex flex-col inset-0 bg-gradient-to-br from-[#121212] via-[#161616] to-[#0f0f0f]">
      {/* Kept outside the scrolling area so the exit stays reachable on every step */}
      <div className="flex w-full shrink-0 justify-center px-8 pt-8">
        <button
          type="button"
          onClick={() => void handleSkipWizard()}
          disabled={isSkipping}
          className="group relative inline-flex items-center gap-2 overflow-hidden rounded-full border border-fuchsia-300/40 bg-gradient-to-r from-[#ff4d8d] via-[#b14cff] to-[#6d5dfc] px-6 py-3 text-sm font-semibold text-white shadow-[0_0_30px_rgba(177,76,255,0.35)] transition-all duration-200 hover:-translate-y-0.5 hover:shadow-[0_0_38px_rgba(255,77,141,0.5)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#ff8fba] focus-visible:ring-offset-2 focus-visible:ring-offset-[#121212] disabled:cursor-wait disabled:opacity-60"
        >
          <span className="absolute inset-0 translate-x-[-120%] bg-gradient-to-r from-transparent via-white/25 to-transparent transition-transform duration-700 group-hover:translate-x-[120%]" />
          <Sparkles className="relative h-4 w-4" aria-hidden="true" />
          <span className="relative">
            {t(
              "onboarding.skipWizard",
              "Skip wizard and open main menu",
            )}
          </span>
          <ArrowRight
            className="relative h-4 w-4 transition-transform group-hover:translate-x-1"
            aria-hidden="true"
          />
        </button>
      </div>

      <div className="flex-1 min-h-0 flex flex-col gap-6 overflow-y-auto p-8 pt-6">
        <div className="flex flex-col items-center gap-3 shrink-0">
          <HandyTextLogo
            width={220}
            className="drop-shadow-[0_0_20px_rgba(255,107,157,0.4)]"
          />
          {getSubtitle() && (
            <p className="text-[#a0a0a0] max-w-md font-medium mx-auto text-center">
              {getSubtitle()}
            </p>
          )}
        </div>

        <div className="max-w-[600px] w-full mx-auto text-center flex-1 flex flex-col min-h-0">
          {error && (
            <div className="bg-[#ff453a]/10 border border-[#ff453a]/30 rounded-xl p-4 mb-4 shrink-0 backdrop-blur-sm">
              <p className="text-[#ff453a] text-sm font-medium">{error}</p>
            </div>
          )}

          <div className="flex flex-col gap-4 ">
            {mode === "permissions" ? (
              isMacOS ? (
                <MacPermissionsOnboarding onComplete={handlePermissionsComplete} />
              ) : (
                <AccessibilityOnboarding onComplete={handlePermissionsComplete} />
              )
            ) : mode === "welcome" ? (
              <div className="flex flex-col items-center gap-6 mt-4">
                <h2 className="text-2xl font-bold text-[#f5f5f5]">
                  {t("onboarding.welcome.title")}
                </h2>

                {/* Status message */}
                <p className="text-[#a0a0a0] text-sm">
                  {welcomeVariant === "local"
                    ? downloading
                      ? t("onboarding.welcome.modelDownloading")
                      : t("onboarding.welcome.modelReady", "Your model is ready.")
                    : t("onboarding.welcome.providerReady")}
                </p>

                {welcomeVariant === "local" && downloadingModelId && (
                  <button
                    className="px-5 py-2 rounded-lg border border-[#4a4a4a] text-[#d0d0d0] text-sm hover:border-[#ff4d8d] hover:text-white transition-colors"
                    onClick={() => void handleCancelDownload()}
                    type="button"
                  >
                    {t("common.cancel")}
                  </button>
                )}

                {/* Shortcut selector */}
                <div className="w-full text-left space-y-2">
                  <p className="text-sm text-[#a0a0a0]">
                    {t("onboarding.welcome.shortcutChoose", "Choose your dictation shortcut:")}
                  </p>
                  <HandyShortcut
                    shortcutId="transcribe"
                    descriptionMode="inline"
                  />
                  {shortcutMode && (
                    <p className="text-sm text-[#d0d0d0]" aria-live="polite">
                      {shortcutMode === "auto"
                        ? t("onboarding.welcome.shortcutAuto", "Tap the shortcut to start and tap again to stop, or hold it while speaking and release to stop.")
                        : shortcutMode === "hold"
                          ? t("onboarding.welcome.shortcutHold", "Hold the shortcut while speaking, then release it to finish.")
                          : t("onboarding.welcome.shortcutToggle", "Press the shortcut to start speaking, then press it again to finish.")}
                    </p>
                  )}
                  {settings?.automatic_app_profiles_enabled && (
                    <p className="text-xs text-[#a0a0a0]">{t("onboarding.welcome.shortcutProfiles", "Other application profiles may use a different shortcut mode.")}</p>
                  )}
                </div>

                <div className="w-full text-left space-y-2">
                  <label htmlFor="onboarding-practice" className="text-sm font-medium text-[#f5f5f5]">
                    {t("onboarding.welcome.practiceTitle", "Try dictation here")}
                  </label>
                  <p className="text-sm text-[#a0a0a0]">
                    {downloading
                      ? t("onboarding.welcome.practiceDownloading", "Once the model finishes downloading, click the field and try your shortcut. You can also continue now.")
                      : t("onboarding.welcome.practiceHint", "Click the field, use your shortcut, and say a few words. This step is optional.")}
                  </p>
                  <textarea
                    id="onboarding-practice"
                    rows={3}
                    className="w-full resize-y rounded-lg border border-[#4a4a4a] bg-[#202020] p-3 text-sm text-[#f5f5f5] focus:border-[#ff4d8d] focus:outline-none"
                    placeholder={t("onboarding.welcome.practicePlaceholder", "Your words can appear here…")}
                  />
                  <p className="text-xs text-[#a0a0a0]">
                    {t("onboarding.welcome.practiceOutput", "The result follows your current output settings; preview-only or clipboard modes may show it elsewhere.")}
                  </p>
                </div>

                {/* Settings note */}
                <p className="text-xs text-[#6b6b6b] max-w-md">
                  {t("onboarding.welcome.settingsNote")}
                </p>

                {/* Get Started button */}
                <button
                  className="px-8 py-3 rounded-xl bg-[#ff4d8d] hover:bg-[#ff3377] text-white font-semibold transition-colors"
                  onClick={handleGetStarted}
                >
                  {t("onboarding.welcome.getStarted")}
                </button>
              </div>
            ) : mode === "select" ? (
              <div className="flex flex-col gap-4">
                <button
                  className="glass-panel-interactive flex justify-between items-center gap-4 rounded-xl p-5 text-left group"
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
                  <div className="w-10 h-10 shrink-0 rounded-full bg-[#9b5de5] flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity">
                    <svg
                      className="w-5 h-5 text-white"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M9 5l7 7-7 7"
                      />
                    </svg>
                  </div>
                </button>
                <button
                  className="glass-panel-interactive flex justify-between items-center gap-4 rounded-xl p-5 text-left group"
                  onClick={handleSelectRemote}
                >
                  <div>
                    <h3 className="text-lg font-semibold text-[#f5f5f5] group-hover:text-[#ff4d8d] transition-colors">
                      {t("onboarding.mode.remote.title")}
                    </h3>
                    <p className="text-[#a0a0a0] text-sm mt-1">
                      {t("onboarding.mode.remote.description")}
                    </p>
                  </div>
                  {(
                    <div className="w-10 h-10 shrink-0 rounded-full bg-[#9b5de5] flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity">
                      <svg
                        className="w-5 h-5 text-white"
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          strokeWidth={2}
                          d="M9 5l7 7-7 7"
                        />
                      </svg>
                    </div>
                  )}
                </button>
                <p className="text-xs text-[#6b6b6b] mt-2">
                  {t("onboarding.mode.changeProviderLater")}
                </p>
                <AiAssistTip className="mt-4 text-left" />
              </div>
            ) : (
              <>
                {savedProgress?.step === "local" && (
                  <p className="text-sm text-[#b8b8b8]">{t("onboarding.resumeLocal", "Choose a model to continue. An interrupted download can be resumed.")}</p>
                )}
                <div className="w-full space-y-2 text-left">
                  <label htmlFor="onboarding-model-language" className="text-sm font-medium text-[#f5f5f5]">
                    {t("onboarding.modelLanguages.title", "Languages to compare")}
                  </label>
                  <p className="text-xs text-[#a0a0a0]">{t("onboarding.modelLanguages.hint", "Suggestions use your system or interface languages. Add or remove languages to compare models; this does not change your dictation settings.")}</p>
                  <select
                    id="onboarding-model-language"
                    value=""
                    onChange={(event) => {
                      const code = languageIdentity(event.target.value);
                      if (code) {
                        changeModelLanguages(modelLanguages.includes(code) ? modelLanguages : [...modelLanguages, code]);
                      }
                    }}
                    className="w-full rounded-lg border border-[#4a4a4a] bg-[#202020] p-2 text-sm text-[#f5f5f5]"
                  >
                    <option value="">{t("onboarding.modelLanguages.add", "Add a language…")}</option>
                    {MODEL_CAPABILITY_LANGUAGES.filter((language) => !modelLanguages.includes(languageIdentity(language.value))).map((language) => (
                      <option key={language.value} value={language.value}>{language.label}</option>
                    ))}
                  </select>
                  <div className="flex flex-wrap gap-2">
                    {modelLanguages.map((code) => (
                      <button key={code} type="button" onClick={() => changeModelLanguages(modelLanguages.filter((item) => item !== code))} className="rounded-full border border-[#4a4a4a] px-3 py-1 text-xs text-[#d0d0d0] hover:border-[#ff4d8d]" aria-label={t("onboarding.modelLanguages.remove", { defaultValue: "Remove {{language}}", language: MODEL_CAPABILITY_LANGUAGES.find((item) => languageIdentity(item.value) === code)?.label ?? code })}>
                        {MODEL_CAPABILITY_LANGUAGES.find((item) => languageIdentity(item.value) === code)?.label ?? code} ×
                      </button>
                    ))}
                  </div>
                  {modelLanguages.length > 0 && recommendations.size === 0 && (
                    <p className="text-xs text-[#a0a0a0]">{t("onboarding.modelLanguages.noRecommendations", "No measured recommendation is available for all chosen languages. You can still choose from the models below.")}</p>
                  )}
                  {modelLanguages.length > 0 && (
                    <p className="text-xs text-[#a0a0a0]">{t("onboarding.modelLanguages.benchmarkNote", "Grades use FLEURS benchmark data for the catalog's default model files. Real-world accuracy can differ.")}</p>
                  )}
                </div>
                {availableModels
                  .filter((model) => getRecommendedBadge(model))
                  .map((model) => (
                    <ModelCard
                      key={model.id}
                      model={model}
                      variant="featured"
                      recommendationRole={recommendations.get(model.id)}
                      modelLanguages={modelLanguages}
                      disabled={downloading}
                      onSelect={handleModelSelection}
                    />
                  ))}

                {availableModels
                  .filter((model) => !getRecommendedBadge(model))
                  .sort((a, b) => Number(a.size_mb) - Number(b.size_mb))
                  .map((model) => (
                    <ModelCard
                      key={model.id}
                      model={model}
                      modelLanguages={modelLanguages}
                      disabled={downloading}
                      onSelect={handleModelSelection}
                    />
                  ))}
              </>
            )}
          </div>
        </div>
      </div>
      {/* Remote STT Configuration Wizard */}
      <RemoteSttWizard
        isOpen={showRemoteWizard}
        onClose={handleRemoteWizardClose}
        onComplete={handleRemoteWizardComplete}
        onSkipWizard={() => void handleSkipWizard()}
        skipDisabled={isSkipping}
      />
    </div>
  );
};

export default Onboarding;
