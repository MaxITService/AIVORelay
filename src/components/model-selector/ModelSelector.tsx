import React, { useState, useRef, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { type as getOsType } from "@tauri-apps/plugin-os";
import { commands, type ModelInfo, type RemoteSttSettings, type SttModelSelection } from "@/bindings";
import { resolveRemoteApiRow, type RemoteApiRow } from "@/lib/remoteApiModels";
import { sttSelectionKey } from "@/lib/sttModelSelection";
import type { ModelStateEvent } from "@/lib/types/events";
import { getTranslatedModelName } from "../../lib/utils/modelTranslation";
import { getRemoteApiDisplayLabel } from "../../lib/utils/remoteSttDisplay";
import ModelStatusButton from "./ModelStatusButton";
import ModelDropdown from "./ModelDropdown";
import DownloadProgressDisplay from "./DownloadProgressDisplay";
import { isMoonshineStreamingModel } from "./nativeStreamingModel";
import { useSettings } from "../../hooks/useSettings";
import { usePresence } from "../../hooks/usePresence";
import {
  beginModelDownloadActivationIntent,
  cancelModelDownloadActivationIntent,
  consumeModelDownloadAutoActivation,
  invalidateModelDownloadActivationIntent,
  prepareModelDownloadAutoActivation,
} from "@/lib/modelDownloadActivation";

interface DownloadProgress {
  model_id: string;
  downloaded: number;
  total: number;
  percentage: number;
}

interface ModelDownloadFailedEvent {
  model_id: string;
  error: string;
}

type ModelStatus =
  | "ready"
  | "loading"
  | "downloading"
  | "extracting"
  | "error"
  | "unloaded"
  | "none";

interface DownloadStats {
  startTime: number;
  lastUpdate: number;
  totalDownloaded: number;
  speed: number;
}

interface ModelSelectorProps {
  onError?: (error: string) => void;
  onInteraction?: () => void;
}

const ModelSelector: React.FC<ModelSelectorProps> = ({
  onError,
  onInteraction,
}) => {
  const { t } = useTranslation();
  const { settings, getSetting, setTranscriptionProvider, refreshSettings } = useSettings();
  const supportsRemoteProviders = getOsType() === "windows";
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [currentModelId, setCurrentModelId] = useState<string>("");
  const [modelStatus, setModelStatus] = useState<ModelStatus>("unloaded");
  const [modelError, setModelError] = useState<string | null>(null);
  const [modelDownloadProgress, setModelDownloadProgress] = useState<
    Map<string, DownloadProgress>
  >(new Map());
  const [showModelDropdown, setShowModelDropdown] = useState(false);
  const modelDropdownPresence = usePresence(showModelDropdown);
  const [downloadStats, setDownloadStats] = useState<
    Map<string, DownloadStats>
  >(new Map());
  const [extractingModels, setExtractingModels] = useState<Set<string>>(
    new Set(),
  );

  const activeProfileId = settings?.active_profile_id ?? "default";
  const activeProfile = activeProfileId === "default"
    ? undefined
    : settings?.transcription_profiles?.find((profile) => profile.id === activeProfileId);
  const profileModelSelection = activeProfile?.stt_model_selection_override;
  const profileModelSelectionKey = profileModelSelection
    ? sttSelectionKey(profileModelSelection)
    : null;
  const transcriptionProvider = String(
    profileModelSelection?.provider ?? getSetting("transcription_provider") ?? "local",
  );
  const globalRemoteStt = getSetting("remote_stt") as RemoteSttSettings | undefined;
  const remoteStt = profileModelSelection?.provider === "remote_openai_compatible"
    ? {
        ...globalRemoteStt,
        provider_preset: profileModelSelection.provider_preset || globalRemoteStt?.provider_preset,
        model_id: profileModelSelection.model_id || globalRemoteStt?.model_id,
      } as RemoteSttSettings
    : globalRemoteStt;
  const effectiveModelId = profileModelSelection?.provider === "local"
    ? profileModelSelection.model_id || getSetting("selected_model") || currentModelId
    : getSetting("selected_model") || currentModelId;
  const displayModelStatus = effectiveModelId !== currentModelId &&
    (modelStatus === "ready" || modelStatus === "loading")
    ? "unloaded"
    : modelStatus;
  const remoteApiDisplayLabel = getRemoteApiDisplayLabel(remoteStt);
  const isRemoteProvider =
    transcriptionProvider === "remote_openai_compatible" ||
    transcriptionProvider === "remote_soniox" ||
    transcriptionProvider === "remote_deepgram";

  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setModelError(null);
    loadCurrentModel();
  }, [activeProfileId, profileModelSelectionKey]);

  useEffect(() => {
    loadModels();
    loadCurrentModel();

    // Listen for model state changes
    const modelStateUnlisten = listen<ModelStateEvent>(
      "model-state-changed",
      (event) => {
        const { event_type, model_id, model_name, error } = event.payload;

        switch (event_type) {
          case "loading_started":
            setModelStatus("loading");
            setModelError(null);
            break;
          case "selection_changed":
            setModelStatus("unloaded");
            setModelError(null);
            if (model_id) setCurrentModelId(model_id);
            break;
          case "loading_completed":
            setModelStatus("ready");
            setModelError(null);
            if (model_id) setCurrentModelId(model_id);
            break;
          case "loading_failed":
            setModelStatus("error");
            setModelError(error || "Failed to load model");
            break;
          case "unloaded":
            setModelStatus("unloaded");
            setModelError(null);
            break;
        }
      },
    );

    // Listen for model download progress
    const downloadProgressUnlisten = listen<DownloadProgress>(
      "model-download-progress",
      (event) => {
        const progress = event.payload;
        setModelDownloadProgress((prev) => {
          const newMap = new Map(prev);
          newMap.set(progress.model_id, progress);
          return newMap;
        });
        setModelStatus("downloading");

        // Update download stats for speed calculation
        const now = Date.now();
        setDownloadStats((prev) => {
          const current = prev.get(progress.model_id);
          const newStats = new Map(prev);

          if (!current) {
            // First progress update - initialize
            newStats.set(progress.model_id, {
              startTime: now,
              lastUpdate: now,
              totalDownloaded: progress.downloaded,
              speed: 0,
            });
          } else {
            // Calculate speed over last few seconds
            const timeDiff = (now - current.lastUpdate) / 1000; // seconds
            const bytesDiff = progress.downloaded - current.totalDownloaded;

            if (timeDiff > 0.5) {
              // Update speed every 500ms
              const currentSpeed = bytesDiff / (1024 * 1024) / timeDiff; // MB/s
              // Smooth the speed with exponential moving average, but ensure positive values
              const validCurrentSpeed = Math.max(0, currentSpeed);
              const smoothedSpeed =
                current.speed > 0
                  ? current.speed * 0.8 + validCurrentSpeed * 0.2
                  : validCurrentSpeed;

              newStats.set(progress.model_id, {
                startTime: current.startTime,
                lastUpdate: now,
                totalDownloaded: progress.downloaded,
                speed: Math.max(0, smoothedSpeed),
              });
            }
          }

          return newStats;
        });
      },
    );

    // Listen for model download completion
    const downloadCompleteUnlisten = listen<string>(
      "model-download-complete",
      (event) => {
        const modelId = event.payload;
        setModelDownloadProgress((prev) => {
          const newMap = new Map(prev);
          newMap.delete(modelId);
          return newMap;
        });
        setDownloadStats((prev) => {
          const newStats = new Map(prev);
          newStats.delete(modelId);
          return newStats;
        });
        loadModels(); // Refresh models list

        // Only the latest requested download may auto-select, and only while
        // the user's model/provider selection remains unchanged.
        setTimeout(async () => {
          const settingsResult = await commands.getAppSettings();
          if (settingsResult.status === "error") {
            cancelModelDownloadActivationIntent(modelId);
            return;
          }
          const isRecording = await commands.isRecording();
          if (isRecording) {
            cancelModelDownloadActivationIntent(modelId);
            return; // Skip auto-switch if recording in progress
          }
          const activationGeneration = await prepareModelDownloadAutoActivation(
            modelId,
            settingsResult.data.active_profile_id ?? "default",
          );
          if (activationGeneration === null) {
            return;
          }
          if (
            !consumeModelDownloadAutoActivation(modelId, activationGeneration)
          ) {
            return;
          }
          setModelError(null);
          setShowModelDropdown(false);
          const result = (settingsResult.data.active_profile_id ?? "default") === "default"
            ? await commands.setActiveModel(modelId)
            : await commands.changeActiveProfileSttModelSelectionOverride({
                provider: "local",
                model_id: modelId,
                provider_preset: "",
              });
          if (result.status === "error") {
            setModelError(result.error);
            setModelStatus("error");
            onError?.(result.error);
          } else {
            await refreshSettings();
          }
        }, 500);
      },
    );

    // Listen for model download cancellation
    const downloadCancelledUnlisten = listen<string>(
      "model-download-cancelled",
      (event) => {
        const modelId = event.payload;
        cancelModelDownloadActivationIntent(modelId);
        setModelDownloadProgress((prev) => {
          const newMap = new Map(prev);
          newMap.delete(modelId);
          return newMap;
        });
        setDownloadStats((prev) => {
          const newStats = new Map(prev);
          newStats.delete(modelId);
          return newStats;
        });
        setModelStatus("unloaded");
        loadModels(); // Refresh models list to show "Download" button again
      },
    );

    // Listen for extraction events
    const extractionStartedUnlisten = listen<string>(
      "model-extraction-started",
      (event) => {
        const modelId = event.payload;
        setExtractingModels((prev) => new Set(prev.add(modelId)));
        setModelStatus("extracting");
      },
    );

    const extractionCompletedUnlisten = listen<string>(
      "model-extraction-completed",
      (event) => {
        const modelId = event.payload;
        setExtractingModels((prev) => {
          const next = new Set(prev);
          next.delete(modelId);
          return next;
        });
        loadModels(); // Completion event performs the single auto-activation.
      },
    );

    const extractionFailedUnlisten = listen<{
      model_id: string;
      error: string;
    }>("model-extraction-failed", (event) => {
      const modelId = event.payload.model_id;
      setExtractingModels((prev) => {
        const next = new Set(prev);
        next.delete(modelId);
        return next;
      });
      setModelError(`Failed to extract model: ${event.payload.error}`);
      setModelStatus("error");
    });

    const downloadFailedUnlisten = listen<ModelDownloadFailedEvent>(
      "model-download-failed",
      (event) => {
        const { model_id, error } = event.payload;
        cancelModelDownloadActivationIntent(model_id);
        setModelDownloadProgress((prev) => {
          const newMap = new Map(prev);
          newMap.delete(model_id);
          return newMap;
        });
        setDownloadStats((prev) => {
          const newStats = new Map(prev);
          newStats.delete(model_id);
          return newStats;
        });
        setExtractingModels((prev) => {
          const next = new Set(prev);
          next.delete(model_id);
          return next;
        });
        setModelError(error);
        setModelStatus("error");
        loadModels();
      },
    );

    // Listen for model deletion (e.g. from Models settings page)
    const modelDeletedUnlisten = listen<string>("model-deleted", () => {
      loadModels();
      loadCurrentModel();
    });

    // Click outside to close dropdown
    const handleClickOutside = (event: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node)
      ) {
        setShowModelDropdown(false);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);

    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
      modelStateUnlisten.then((fn) => fn());
      downloadProgressUnlisten.then((fn) => fn());
      downloadCompleteUnlisten.then((fn) => fn());
      downloadCancelledUnlisten.then((fn) => fn());
      extractionStartedUnlisten.then((fn) => fn());
      extractionCompletedUnlisten.then((fn) => fn());
      extractionFailedUnlisten.then((fn) => fn());
      downloadFailedUnlisten.then((fn) => fn());
      modelDeletedUnlisten.then((fn) => fn());
    };
  }, []);

  // Escape closes the dropdown and returns focus to the status button.
  useEffect(() => {
    if (!showModelDropdown) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      const container = dropdownRef.current;
      const focusWasInside =
        !!container && container.contains(document.activeElement);
      setShowModelDropdown(false);
      if (focusWasInside) container.querySelector("button")?.focus();
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [showModelDropdown]);

  const loadModels = async () => {
    try {
      const result = await commands.getAvailableModels();
      if (result.status === "ok") {
        setModels(result.data);
      }
    } catch (err) {
      console.error("Failed to load models:", err);
    }
  };

  const loadCurrentModel = async () => {
    try {
      const result = await commands.getCurrentModel();
      if (result.status === "ok") {
        const current = result.data;
        setCurrentModelId(current);

        if (current) {
          // Check if model is actually loaded
          const statusResult = await commands.getTranscriptionModelStatus();
          if (statusResult.status === "ok") {
            const transcriptionStatus = statusResult.data;
            if (transcriptionStatus === current) {
              setModelStatus("ready");
            } else {
              setModelStatus("unloaded");
            }
          }
        } else {
          setModelStatus("none");
        }
      }
    } catch (err) {
      console.error("Failed to load current model:", err);
      setModelStatus("error");
      setModelError("Failed to check model status");
    }
  };

  const setActiveProfileModelSelection = useCallback(async (selection: SttModelSelection) => {
    if (activeProfileId === "default") return false;
    const result = await commands.changeActiveProfileSttModelSelectionOverride(selection);
    if (result.status === "error") throw new Error(result.error);
    await refreshSettings();
    setModelError(null);
    setModelStatus("unloaded");
    return true;
  }, [activeProfileId, refreshSettings]);

  const handleModelSelect = async (modelId: string) => {
    try {
      invalidateModelDownloadActivationIntent();
      setShowModelDropdown(false);
      if (await setActiveProfileModelSelection({
        provider: "local",
        model_id: modelId,
        provider_preset: "",
      })) return;
      // If we're in remote mode and selecting a local model, switch to local provider first
      if (isRemoteProvider) {
        await setTranscriptionProvider("local");
      }
      setCurrentModelId(modelId); // Set optimistically so loading text shows correct model
      setModelError(null);
      setShowModelDropdown(false);
      const result = await commands.setActiveModel(modelId);
      if (result.status === "error") {
        const errorMsg = result.error;
        setModelError(errorMsg);
        setModelStatus("error");
        onError?.(errorMsg);
      } else {
        await refreshSettings();
      }
    } catch (err) {
      const errorMsg = `${err}`;
      setModelError(errorMsg);
      setModelStatus("error");
      onError?.(errorMsg);
    }
  };

  const handleModelDownload = async (modelId: string) => {
    try {
      // Custom profiles switch after download; Default keeps global selection behavior.
      if (activeProfileId === "default" && isRemoteProvider) {
        await setTranscriptionProvider("local");
      }
      setModelError(null);
      await beginModelDownloadActivationIntent(modelId);
      const result = await commands.downloadModel(modelId);
      if (result.status === "error") {
        cancelModelDownloadActivationIntent(modelId);
        const errorMsg = result.error;
        setModelError(errorMsg);
        setModelStatus("error");
        onError?.(errorMsg);
      }
    } catch (err) {
      cancelModelDownloadActivationIntent(modelId);
      const errorMsg = `${err}`;
      setModelError(errorMsg);
      setModelStatus("error");
      onError?.(errorMsg);
    }
  };

  const getCurrentModel = () => {
    return models.find((m) => m.id === effectiveModelId);
  };

  const currentModel = getCurrentModel();
  const currentModelSupportsNativeStreaming =
    !isRemoteProvider &&
    currentModel?.engine_type === "TranscribeCpp" &&
    Boolean(currentModel.supports_streaming);
  const currentModelIsMoonshineStreaming =
    isMoonshineStreamingModel(currentModel);
  const livePreviewEnabled = Boolean(
    getSetting("soniox_live_preview_enabled") ||
      getSetting("preview_output_only_enabled"),
  );
  const nativeLiveOutputEnabled = Boolean(
    !currentModelIsMoonshineStreaming &&
      currentModel?.id &&
      (
        (getSetting("native_streaming_live_output_models") ?? []) as string[]
      ).includes(currentModel.id),
  );
  let nativeStreamingTitle: string | undefined;
  if (currentModelSupportsNativeStreaming) {
    if (livePreviewEnabled) {
      nativeStreamingTitle = t(
        "modelSelector.nativeStreamingActiveTooltip",
        "Native streaming available. Live Preview Window is enabled.",
      );
    } else if (currentModelIsMoonshineStreaming) {
      nativeStreamingTitle = t(
        "modelSelector.moonshineStreamingPreviewOnlyTooltip",
        "Native streaming is available through Live Preview. Text is inserted after recording stops.",
      );
    } else if (nativeLiveOutputEnabled) {
      nativeStreamingTitle = t(
        "modelSelector.nativeStreamingLiveOutputActiveTooltip",
        "Native streaming active. Live final output is enabled.",
      );
    } else {
      nativeStreamingTitle = t(
        "modelSelector.nativeStreamingCurrentlyOff",
        "Streaming available · Currently off (either enable Live Preview or Live final output)",
      );
    }
  }

  const getModelDisplayText = (): string => {
    if (transcriptionProvider === "remote_openai_compatible") {
      return remoteApiDisplayLabel;
    }
    if (transcriptionProvider === "remote_soniox") {
      return t("modelSelector.remoteSonioxActive");
    }
    if (transcriptionProvider === "remote_deepgram") {
      return t("modelSelector.remoteDeepgramActive", "Remote Deepgram");
    }

    if (extractingModels.size > 0) {
      if (extractingModels.size === 1) {
        const [modelId] = Array.from(extractingModels);
        const model = models.find((m) => m.id === modelId);
        const modelName = model
          ? getTranslatedModelName(model, t)
          : t("modelSelector.extractingGeneric").replace("...", "");
        return t("modelSelector.extracting", { modelName });
      } else {
        return t("modelSelector.extractingMultiple", {
          count: extractingModels.size,
        });
      }
    }

    if (modelDownloadProgress.size > 0) {
      if (modelDownloadProgress.size === 1) {
        const [progress] = Array.from(modelDownloadProgress.values());
        const percentage = Math.max(
          0,
          Math.min(100, Math.round(progress.percentage)),
        );
        return t("modelSelector.downloading", { percentage });
      } else {
        return t("modelSelector.downloadingMultiple", {
          count: modelDownloadProgress.size,
        });
      }
    }

    switch (displayModelStatus) {
      case "ready":
        return currentModel
          ? getTranslatedModelName(currentModel, t)
          : t("modelSelector.modelReady");
      case "loading":
        return currentModel
          ? t("modelSelector.loading", {
              modelName: getTranslatedModelName(currentModel, t),
            })
          : t("modelSelector.loadingGeneric");
      case "extracting":
        return currentModel
          ? t("modelSelector.extracting", {
              modelName: getTranslatedModelName(currentModel, t),
            })
          : t("modelSelector.extractingGeneric");
      case "error":
        return modelError || t("modelSelector.modelError");
      case "unloaded":
        return currentModel
          ? getTranslatedModelName(currentModel, t)
          : t("modelSelector.modelUnloaded");
      case "none":
        return t("modelSelector.noModelDownloadRequired");
      default:
        return currentModel
          ? getTranslatedModelName(currentModel, t)
          : t("modelSelector.modelUnloaded");
    }
  };

  const handleRemoteApiSelect = async (row: RemoteApiRow) => {
    try {
      invalidateModelDownloadActivationIntent();
      setShowModelDropdown(false);
      const { preset, modelId } = resolveRemoteApiRow(
        row,
        remoteStt?.provider_preset,
      );
      if (await setActiveProfileModelSelection({
        provider: "remote_openai_compatible",
        model_id: modelId || (globalRemoteStt?.provider_preset === "custom"
          ? globalRemoteStt.model_id
          : "gpt-transcribe"),
        provider_preset: preset,
      })) return;
      // Reapplying a preset resets its endpoint and model, including Custom API.
      if (preset !== remoteStt?.provider_preset) {
        const result = await commands.changeRemoteSttProviderPresetSetting(preset);
        if (result.status === "error") throw new Error(result.error);
      }
      if (modelId) {
        const result = await commands.changeRemoteSttModelIdSetting(modelId);
        if (result.status === "error") throw new Error(result.error);
      }
      await setTranscriptionProvider("remote_openai_compatible");
      await refreshSettings();
      setModelError(null);
      setModelStatus("ready");
    } catch (err) {
      const errorMsg = `${err}`;
      setModelError(errorMsg);
      setModelStatus("error");
      onError?.(errorMsg);
    }
  };

  const handleRemoteProviderSelect = useCallback(
    async (provider: string) => {
      if (!supportsRemoteProviders) {
        const errorMsg = t(
          "modelSelector.remoteWindowsOnly",
          "Remote transcription providers are only available on Windows.",
        );
        setModelError(errorMsg);
        setModelStatus("error");
        onError?.(errorMsg);
        return;
      }
      try {
        invalidateModelDownloadActivationIntent();
        setShowModelDropdown(false);
        if (await setActiveProfileModelSelection({
          provider: provider as SttModelSelection["provider"],
          model_id: provider === "remote_soniox"
            ? profileModelSelection?.provider === "remote_soniox"
              ? profileModelSelection.model_id
              : getSetting("soniox_model") || "stt-rt-v5"
            : profileModelSelection?.provider === "remote_deepgram"
              ? profileModelSelection.model_id
              : getSetting("deepgram_model") || "nova-3",
          provider_preset: "",
        })) return;
        await setTranscriptionProvider(provider);
        setModelError(null);
        setModelStatus("ready");
      } catch (err) {
        const errorMsg = `Failed to switch to remote provider: ${err}`;
        setModelError(errorMsg);
        setModelStatus("error");
        onError?.(errorMsg);
      }
    },
    [onError, setTranscriptionProvider, supportsRemoteProviders, t, setActiveProfileModelSelection, profileModelSelection, getSetting],
  );

  const handleModelDelete = async (modelId: string) => {
    const result = await commands.deleteModel(modelId);
    if (result.status === "ok") {
      await loadModels();
      setModelError(null);
    }
  };

  return (
    <>
      {/* Model Status and Switcher */}
      <div className="relative" ref={dropdownRef}>
        <ModelStatusButton
          status={
            modelStatus === "error"
              ? "error"
              : isRemoteProvider
                ? "ready"
                : displayModelStatus
          }
          displayText={getModelDisplayText()}
          nativeStreaming={currentModelSupportsNativeStreaming}
          nativeStreamingTitle={nativeStreamingTitle}
          isDropdownOpen={showModelDropdown}
          onClick={() => {
            onInteraction?.();
            setShowModelDropdown(!showModelDropdown);
          }}
          disabled={false}
          isRemote={isRemoteProvider}
          remoteProvider={isRemoteProvider ? transcriptionProvider : undefined}
        />

        {/* Model Dropdown */}
        {modelDropdownPresence.isMounted && (
          <ModelDropdown
            presenceState={modelDropdownPresence.state}
            onProfileNavigate={() => setShowModelDropdown(false)}
            models={models}
            currentModelId={effectiveModelId}
            downloadProgress={modelDownloadProgress}
            onModelSelect={handleModelSelect}
            onModelDownload={handleModelDownload}
            onModelDownloadCancel={async (modelId) => {
              try {
                await commands.cancelDownload(modelId);
              } catch (err) {
                onError?.(`${err}`);
              }
            }}
            onModelDelete={handleModelDelete}
            onError={onError}
            currentProvider={transcriptionProvider}
            remoteStt={remoteStt}
            onRemoteApiSelect={
              supportsRemoteProviders ? handleRemoteApiSelect : undefined
            }
            onRemoteProviderSelect={
              supportsRemoteProviders ? handleRemoteProviderSelect : undefined
            }
          />
        )}
      </div>

      {/* Download Progress Bar for Models */}
      <DownloadProgressDisplay
        downloadProgress={modelDownloadProgress}
        downloadStats={downloadStats}
      />
    </>
  );
};

export default ModelSelector;
