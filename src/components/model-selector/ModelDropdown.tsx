import React from "react";
import { useTranslation } from "react-i18next";
import type { ModelInfo, RemoteSttSettings } from "@/bindings";
import {
  getRemoteApiRows,
  resolveRemoteApiRow,
  type RemoteApiRow,
} from "@/lib/remoteApiModels";
import { formatModelSize } from "../../lib/utils/format";
import {
  getTranslatedModelName,
  getTranslatedModelDescription,
} from "../../lib/utils/modelTranslation";
import { ProgressBar } from "../shared";
import { ModelReleaseDate } from "../shared/ModelReleaseDate";
import { NameSortControl } from "../ui/NameSortControl";
import { useSortedDisplayNames } from "../../hooks/useSortedDisplayNames";
import { useListSortDirection } from "../../hooks/useListSortPreference";
import { ModelSelectionProfileNotice } from "../settings/models/ModelSelectionProfileNotice";
import { ModelDeleteAction } from "../shared/ModelDeleteAction";
import { cancelModelDeletion } from "@/lib/modelDeletion";
import type { useModelDeletions } from "../../hooks/useModelDeletions";

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
  modelDeletions: ReturnType<typeof useModelDeletions>;
  onError?: (error: string) => void;
  currentProvider?: string;
  remoteStt?: RemoteSttSettings;
  onRemoteApiSelect?: (row: RemoteApiRow) => void;
  onRemoteProviderSelect?: (provider: string) => void;
  onProfileNavigate?: () => void;
  presenceState?: "open" | "closed";
}

const ModelDropdown: React.FC<ModelDropdownProps> = ({
  models,
  currentModelId,
  downloadProgress,
  onModelSelect,
  onModelDownload,
  onModelDownloadCancel,
  onModelDelete,
  modelDeletions,
  currentProvider = "local",
  remoteStt,
  onRemoteApiSelect,
  onRemoteProviderSelect,
  onProfileNavigate,
  presenceState = "open",
}) => {
  const isRemoteProvider =
    currentProvider === "remote_openai_compatible" ||
    currentProvider === "remote_soniox" ||
    currentProvider === "remote_deepgram";
  const { t } = useTranslation();
  const [direction, setDirection] = useListSortDirection("model-popup");
  const { items: remoteApiRows } = useSortedDisplayNames(
    getRemoteApiRows(t).filter((row) => !row.notRecommended),
    (row) => row.title,
    direction,
  );
  const { items: visibleModels } = useSortedDisplayNames(
    models,
    (model) => getTranslatedModelName(model, t),
    direction,
  );
  const availableModels = visibleModels.filter((m) => m.is_downloaded);
  const downloadableModels = visibleModels.filter((m) => !m.is_downloaded);
  const isFirstRun = availableModels.length === 0 && models.length > 0;
  const nativeStreamingTitle = t(
    "modelSelector.nativeStreamingTooltip",
    "Native streaming model. Enable Live Preview Window to use it.",
  );

  const renderNativeStreamingIcon = (model: ModelInfo) =>
    model.supports_streaming ? (
      <span
        className="ml-1.5 inline-flex align-middle text-[11px] leading-none text-yellow-300 drop-shadow-[0_0_4px_rgba(250,204,21,0.35)]"
        title={nativeStreamingTitle}
        aria-label={nativeStreamingTitle}
      >
        ⚡
      </span>
    ) : null;

  const handleCancelClick = (e: React.MouseEvent, modelId: string) => {
    e.preventDefault();
    e.stopPropagation();
    onModelDownloadCancel(modelId);
  };

  const handleModelClick = (modelId: string) => {
    const deletion = modelDeletions.jobs.get(modelId);
    if (downloadProgress.has(modelId) || (deletion && (deletion.status === "deleting" || modelDeletions.secondsRemaining(modelId) === 0))) {
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
    <div
      data-state={presenceState}
      data-placement="top"
      className="app-popover absolute bottom-full left-0 mb-2 w-64 max-h-[calc(100vh-4rem)] overflow-y-auto bg-background border border-mid-gray/20 rounded-lg shadow-lg pb-2 z-50"
    >
      <div className="sticky top-0 z-10 border-b border-mid-gray/20 bg-background px-3 pb-2 pt-3">
        <ModelSelectionProfileNotice compact onNavigate={onProfileNavigate} />
        <div className="mt-3 flex items-center justify-between gap-2 px-1">
          <span className="text-[10px] font-semibold uppercase tracking-wider text-[#8a8a8a]">
            {t("sidebar.models", "Models")}
          </span>
          <NameSortControl
            direction={direction}
            onChange={setDirection}
            ariaLabel={t("listSorting.modelsLabel", "Sort models")}
          />
        </div>
      </div>
      {/* Remote STT Options */}
      {onRemoteProviderSelect && (
        <div>
          {onRemoteApiSelect && remoteApiRows.map((row) => {
            const { preset, modelId } = resolveRemoteApiRow(
              row,
              remoteStt?.provider_preset,
            );
            const isActive =
              currentProvider === "remote_openai_compatible" &&
              remoteStt?.provider_preset === preset &&
              (row.id === "custom" ||
                row.id === "groq" ||
                remoteStt?.model_id === modelId);

            return (
              <div
                key={row.id}
                onClick={() => !isActive && onRemoteApiSelect(row)}
                onKeyDown={(e) => {
                  if (!isActive && (e.key === "Enter" || e.key === " ")) {
                    e.preventDefault();
                    onRemoteApiSelect(row);
                  }
                }}
                tabIndex={isActive ? undefined : 0}
                role={isActive ? undefined : "button"}
                className={`w-full px-4 py-2 text-left transition-colors focus:outline-none ${
                  isActive
                    ? "bg-blue-500/10 text-blue-400 cursor-default"
                    : "hover:bg-mid-gray/10 cursor-pointer"
                }`}
              >
                <div className="flex items-center justify-between gap-2">
                  <div className="flex items-center gap-2">
                    <svg
                      className={`w-4 h-4 shrink-0 ${row.iconClassName}`}
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
                      <div className="text-sm">{row.title}</div>
                      <div className="text-xs text-text/40 italic">
                        {row.description}
                      </div>
                    </div>
                  </div>
                  {isActive && (
                    <div className="text-xs text-blue-400 shrink-0">
                      {t("modelSelector.active")}
                    </div>
                  )}
                </div>
              </div>
            );
          })}

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
            className={`w-full px-4 py-2 text-left hover:bg-mid-gray/10 transition-colors cursor-pointer focus:outline-none ${
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
                  <div className="text-sm">
                    {t("modelSelector.remoteSonioxMode")}
                  </div>
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

          {/* Remote via Deepgram */}
          <div
            onClick={() => onRemoteProviderSelect("remote_deepgram")}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onRemoteProviderSelect("remote_deepgram");
              }
            }}
            tabIndex={0}
            role="button"
            className={`w-full px-4 py-2 text-left hover:bg-mid-gray/10 transition-colors cursor-pointer focus:outline-none ${
              currentProvider === "remote_deepgram"
                ? "bg-cyan-500/10 text-cyan-400"
                : ""
            }`}
          >
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
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
                  <div className="text-sm">
                    {t(
                      "modelSelector.remoteDeepgramMode",
                      "Remote via Deepgram",
                    )}
                  </div>
                  <div className="text-xs text-text/40 italic">
                    {t(
                      "modelSelector.remoteDeepgramModeDescription",
                      "Deepgram Nova streaming service",
                    )}
                  </div>
                </div>
              </div>
              {currentProvider === "remote_deepgram" && (
                <div className="text-xs text-cyan-400">
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
        <div className="px-4 py-2 bg-logo-primary/10 border-b border-logo-primary/20">
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
          <div className="px-4 py-1 text-xs font-medium text-text/80 border-b border-mid-gray/10">
            {t("modelSelector.availableModels")}
          </div>
          {availableModels.map((model) => {
            const isActive = currentModelId === model.id && !isRemoteProvider;
            const deletion = modelDeletions.jobs.get(model.id);
            const secondsRemaining = modelDeletions.secondsRemaining(model.id);
            const isDeleting = deletion != null && (deletion.status === "deleting" || secondsRemaining === 0);
            const selectable = !isDeleting && (!isActive || deletion?.status === "pending");
            const modelName = getTranslatedModelName(model, t);
            const sizeLabel = `${t("modelSelector.downloadSize")} · ${formatModelSize(Number(model.size_mb))}`;

            return (
              <React.Fragment key={model.id}>
                <div
                  onClick={
                    selectable ? () => handleModelClick(model.id) : undefined
                  }
                  onKeyDown={(e) => {
                    if (e.target === e.currentTarget && selectable && (e.key === "Enter" || e.key === " ")) {
                      e.preventDefault();
                      handleModelClick(model.id);
                    }
                  }}
                  tabIndex={selectable ? 0 : undefined}
                  role={selectable ? "button" : undefined}
                  aria-disabled={isDeleting}
                  className={`w-full px-4 py-2 text-left transition-colors focus:outline-none ${
                    isActive
                      ? "bg-logo-primary/10 text-logo-primary cursor-default"
                      : isDeleting ? "text-text/50 cursor-default" : "hover:bg-mid-gray/10 cursor-pointer"
                  }`}
                >
                  <div className="min-w-0">
                    <div className="min-w-0">
                      <div className="text-sm">
                        <ModelReleaseDate
                          modelId={model.id}
                          className="mr-1.5 text-[10px] text-text/35"
                        />
                        {modelName}
                        {renderNativeStreamingIcon(model)}
                        {model.is_custom && (
                          <span className="ml-1.5 text-[10px] font-medium text-text/40 uppercase">
                            {t("modelSelector.custom")}
                          </span>
                        )}
                        {isActive && <span className="ml-2 text-xs text-logo-primary">{t("modelSelector.active")}</span>}
                      </div>
                      <div className="text-xs text-text/40 italic pr-4">
                        {getTranslatedModelDescription(model, t)}
                      </div>
                    </div>
                    <div className="mt-1 flex min-w-0 items-center justify-between gap-1 text-xs text-text/50">
                      <span className="min-w-0 truncate tabular-nums" title={sizeLabel} aria-label={sizeLabel}>{sizeLabel}</span>
                      <ModelDeleteAction compact modelName={modelName} job={deletion} secondsRemaining={secondsRemaining}
                        busy={modelDeletions.busyModels.has(model.id)} allowDelete={currentModelId !== model.id}
                        onDelete={() => onModelDelete(model.id)} onUndo={() => cancelModelDeletion(model.id, deletion?.job_id)} />
                    </div>
                  </div>
                </div>
              </React.Fragment>
            );
          })}
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
            className={`group w-full px-4 py-2 text-left hover:bg-mid-gray/10 transition-colors focus:outline-none ${
              isDownloading
                ? "bg-logo-primary/5 cursor-default"
                : "cursor-pointer"
            }`}
          >
            <div className="flex items-center justify-between">
              <div>
                <div className="text-sm">
                  <ModelReleaseDate
                    modelId={model.id}
                    className="mr-1.5 text-[10px] text-text/35"
                  />
                  {getTranslatedModelName(model, t)}
                  {renderNativeStreamingIcon(model)}
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
        <div className="px-4 py-2 text-sm text-text/60">
          {t("modelSelector.noModelsAvailable")}
        </div>
      )}
    </div>
  );
};

export default ModelDropdown;
