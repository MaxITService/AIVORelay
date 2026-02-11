import React from "react";
import { useTranslation } from "react-i18next";
import type { ModelInfo } from "@/bindings";
import { formatModelSize } from "../../lib/utils/format";
import {
  getTranslatedModelName,
  getTranslatedModelDescription,
} from "../../lib/utils/modelTranslation";
import { ProgressBar } from "../shared";

interface DownloadProgress {
  model_id: string;
  downloaded: number;
  total: number;
  percentage: number;
}

interface ModelDropdownProps {
  models: ModelInfo[];
  currentModelId: string;
  downloadProgress: Map<string, DownloadProgress>;
  onModelSelect: (modelId: string) => void;
  onModelDownload: (modelId: string) => void;
  onModelDownloadCancel: (modelId: string) => void;
  onModelDelete: (modelId: string) => Promise<void>;
  onError?: (error: string) => void;
  currentProvider?: string;
  onRemoteProviderSelect?: (provider: string) => void;
}

const ModelDropdown: React.FC<ModelDropdownProps> = ({
  models,
  currentModelId,
  downloadProgress,
  onModelSelect,
  onModelDownload,
  onModelDownloadCancel,
  onModelDelete,
  onError,
  currentProvider = "local",
  onRemoteProviderSelect,
}) => {
  const isRemoteProvider =
    currentProvider === "remote_openai_compatible" ||
    currentProvider === "remote_soniox";
  const { t } = useTranslation();
  const availableModels = models.filter((m) => m.is_downloaded);
  const downloadableModels = models.filter((m) => !m.is_downloaded);
  const isFirstRun = availableModels.length === 0 && models.length > 0;

  const handleDeleteClick = async (e: React.MouseEvent, modelId: string) => {
    e.preventDefault();
    e.stopPropagation();

    try {
      await onModelDelete(modelId);
    } catch (err) {
      const errorMsg = `Failed to delete model: ${err}`;
      onError?.(errorMsg);
    }
  };

  const handleCancelClick = (e: React.MouseEvent, modelId: string) => {
    e.preventDefault();
    e.stopPropagation();
    onModelDownloadCancel(modelId);
  };

  const handleModelClick = (modelId: string) => {
    if (downloadProgress.has(modelId)) {
      return; // Don't allow interaction while downloading
    }
    onModelSelect(modelId);
  };

  const handleDownloadClick = (modelId: string) => {
    if (downloadProgress.has(modelId)) {
      return; 
    }
    onModelDownload(modelId);
  };

  return (
    <div className="absolute bottom-full left-0 mb-2 w-64 max-h-[calc(100vh-4rem)] overflow-y-auto bg-background border border-mid-gray/20 rounded-lg shadow-lg py-2 z-50">
      {/* Remote STT Options */}
      {onRemoteProviderSelect && (
        <div>
          {/* Remote via API */}
          <div
            onClick={() => onRemoteProviderSelect("remote_openai_compatible")}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onRemoteProviderSelect("remote_openai_compatible");
              }
            }}
            tabIndex={0}
            role="button"
            className={`w-full px-3 py-2 text-left hover:bg-mid-gray/10 transition-colors cursor-pointer focus:outline-none ${
              currentProvider === "remote_openai_compatible"
                ? "bg-blue-500/10 text-blue-400"
                : ""
            }`}
          >
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                {/* Cloud icon */}
                <svg
                  className="w-4 h-4"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M3 15a4 4 0 004 4h9a5 5 0 10-.1-9.999 5.002 5.002 0 10-9.78 2.096A4.001 4.001 0 003 15z"
                  />
                </svg>
                <div>
                  <div className="text-sm">{t("modelSelector.remoteApiMode")}</div>
                  <div className="text-xs text-text/40 italic">
                    {t("modelSelector.remoteApiModeDescription")}
                  </div>
                </div>
              </div>
              {currentProvider === "remote_openai_compatible" && (
                <div className="text-xs text-blue-400">
                  {t("modelSelector.active")}
                </div>
              )}
            </div>
          </div>

          {/* Remote via Soniox */}
          <div
            onClick={() => onRemoteProviderSelect("remote_soniox")}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onRemoteProviderSelect("remote_soniox");
              }
            }}
            tabIndex={0}
            role="button"
            className={`w-full px-3 py-2 text-left hover:bg-mid-gray/10 transition-colors cursor-pointer focus:outline-none ${
              currentProvider === "remote_soniox"
                ? "bg-teal-500/10 text-teal-400"
                : ""
            }`}
          >
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                {/* Signal/wave icon */}
                <svg
                  className="w-4 h-4"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M9.348 14.651a3.75 3.75 0 010-5.303m5.304 0a3.75 3.75 0 010 5.303m-7.425 2.122a6.75 6.75 0 010-9.546m9.546 0a6.75 6.75 0 010 9.546M5.106 18.894c-3.808-3.808-3.808-9.98 0-13.789m13.788 0c3.808 3.808 3.808 9.981 0 13.79"
                  />
                </svg>
                <div>
                  <div className="text-sm">{t("modelSelector.remoteSonioxMode")}</div>
                  <div className="text-xs text-text/40 italic">
                    {t("modelSelector.remoteSonioxModeDescription")}
                  </div>
                </div>
              </div>
              {currentProvider === "remote_soniox" && (
                <div className="text-xs text-teal-400">
                  {t("modelSelector.active")}
                </div>
              )}
            </div>
          </div>
          <div className="border-t border-mid-gray/10 my-1" />
        </div>
      )}

      {/* First Run Welcome */}
      {isFirstRun && (
        <div className="px-3 py-2 bg-logo-primary/10 border-b border-logo-primary/20">
          <div className="text-xs font-medium text-logo-primary mb-1">
            {t("modelSelector.welcome")}
          </div>
          <div className="text-xs text-text/70">
            {t("modelSelector.downloadPrompt")}
          </div>
        </div>
      )}

      {/* Available Models */}
      {availableModels.length > 0 && (
        <div>
          <div className="px-3 py-1 text-xs font-medium text-text/80 border-b border-mid-gray/10">
            {t("modelSelector.availableModels")}
          </div>
          {availableModels.map((model) => (
            <div
              key={model.id}
              onClick={() => handleModelClick(model.id)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  handleModelClick(model.id);
                }
              }}
              tabIndex={0}
              role="button"
              className={`w-full px-3 py-2 text-left hover:bg-mid-gray/10 transition-colors cursor-pointer focus:outline-none ${
                currentModelId === model.id && !isRemoteProvider
                  ? "bg-logo-primary/10 text-logo-primary"
                  : ""
              }`}
            >
              <div className="flex items-center justify-between">
                <div>
                  <div className="text-sm">
                    {getTranslatedModelName(model, t)}
                    {model.is_custom && (
                      <span className="ml-1.5 text-[10px] font-medium text-text/40 uppercase">
                        {t("modelSelector.custom")}
                      </span>
                    )}
                  </div>
                  <div className="text-xs text-text/40 italic pr-4">
                    {getTranslatedModelDescription(model, t)}
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  {currentModelId === model.id && !isRemoteProvider && (
                    <div className="text-xs text-logo-primary">
                      {t("modelSelector.active")}
                    </div>
                  )}
                  {currentModelId !== model.id && (
                    <button
                      onClick={(e) => handleDeleteClick(e, model.id)}
                      className="text-red-400 hover:text-red-300 p-1 hover:bg-red-500/10 rounded transition-colors"
                      title={t("modelSelector.deleteModel", {
                        modelName: getTranslatedModelName(model, t),
                      })}
                    >
                      <svg
                        className="w-3 h-3"
                        fill="currentColor"
                        viewBox="0 0 20 20"
                      >
                        <path
                          fillRule="evenodd"
                          d="M9 2a1 1 0 00-.894.553L7.382 4H4a1 1 0 000 2v10a2 2 0 002 2h8a2 2 0 002-2V6a1 1 0 100-2h-3.382l-.724-1.447A1 1 0 0011 2H9zM7 8a1 1 0 012 0v6a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v6a1 1 0 102 0V8a1 1 0 00-1-1z"
                          clipRule="evenodd"
                        />
                      </svg>
                    </button>
                  )}
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Downloadable Models */}
      {downloadableModels.map((model) => {
        const isDownloading = downloadProgress.has(model.id);
        const progress = downloadProgress.get(model.id);

        return (
          <div
            key={model.id}
            onClick={() => !isDownloading && handleDownloadClick(model.id)}
            onKeyDown={(e) => {
              if ((e.key === "Enter" || e.key === " ") && !isDownloading) {
                e.preventDefault();
                handleDownloadClick(model.id);
              }
            }}
            tabIndex={isDownloading ? -1 : 0}
            role="button"
            aria-disabled={isDownloading}
            className={`group w-full px-3 py-2 text-left hover:bg-mid-gray/10 transition-colors focus:outline-none ${
              isDownloading
                ? "bg-logo-primary/5 cursor-default"
                : "cursor-pointer"
            }`}
          >
            <div className="flex items-center justify-between">
              <div>
                <div className="text-sm">
                  {getTranslatedModelName(model, t)}
                  {model.is_recommended && isFirstRun && (
                    <span className="ml-2 text-xs bg-logo-primary/20 text-logo-primary px-1.5 py-0.5 rounded">
                      {t("onboarding.recommended")}
                    </span>
                  )}
                </div>
                <div className="text-xs text-text/40 italic pr-4">
                  {getTranslatedModelDescription(model, t)}
                </div>
                <div className="mt-1 text-xs text-text/50 tabular-nums">
                  {t("modelSelector.downloadSize")} ·{" "}
                  {formatModelSize(Number(model.size_mb))}
                </div>
              </div>
              <div className="flex items-center gap-2">
                <div className="text-xs text-logo-primary tabular-nums">
                  {isDownloading && progress
                    ? `${Math.max(0, Math.min(100, Math.round(progress.percentage)))}%`
                    : t("modelSelector.download")}
                </div>
                {isDownloading && (
                  <button
                    onClick={(e) => handleCancelClick(e, model.id)}
                    className="p-1 text-mid-gray hover:text-red-400 hover:bg-red-500/10 rounded transition-all opacity-0 group-hover:opacity-100"
                    title={t("common.cancel")}
                  >
                    <svg
                      className="w-3.5 h-3.5"
                      fill="none"
                      stroke="currentColor"
                      viewBox="0 0 24 24"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M6 18L18 6M6 6l12 12"
                      />
                    </svg>
                  </button>
                )}
              </div>
            </div>

            {isDownloading && progress && (
              <div className="mt-2 text-logo-primary/30">
                <ProgressBar
                  progress={[
                    {
                      id: model.id,
                      percentage: progress.percentage,
                      label: model.name,
                    },
                  ]}
                  size="small"
                />
              </div>
            )}
          </div>
        );
      })}

      {/* No Models Available */}
      {availableModels.length === 0 && downloadableModels.length === 0 && (
        <div className="px-3 py-2 text-sm text-text/60">
          {t("modelSelector.noModelsAvailable")}
        </div>
      )}
    </div>
  );
};

export default ModelDropdown;
