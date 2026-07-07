import React, { useMemo, useState, useRef } from "react";
import { useTranslation } from "react-i18next";
import { ask } from "@tauri-apps/plugin-dialog";
import { Cloud, Cpu, Filter, HardDrive, Radio, RotateCcw } from "lucide-react";
import { useModels } from "../../../hooks/useModels";
import { useSettings } from "../../../hooks/useSettings";
import { useModelFilters } from "../../../hooks/useModelFilters";
import {
  getTranslatedModelDescription,
  getTranslatedModelName,
} from "../../../lib/utils/modelTranslation";
import { formatModelSize } from "../../../lib/utils/format";
import { Button } from "../../ui/Button";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { TellMeMore } from "../../ui/TellMeMore";
import { RemoteSttSettings } from "../remote-stt/RemoteSttSettings";
import { ModelMetadataPanel } from "./ModelMetadataPanel";
import { ModelFilterBar } from "./ModelFilterBar";
import {
  commands,
  type ModelInfo,
  type RemoteSttSettings as RemoteSttSettingsConfig,
} from "@/bindings";

type RemoteApiRowId =
  | "groq"
  | "openai_realtime_whisper"
  | "openai_realtime2"
  | "openai_translate"
  | "custom";

type RemoteApiRow = {
  id: RemoteApiRowId;
  title: string;
  description: string;
  notRecommended?: boolean;
  preset: "groq" | "openai" | "custom";
  modelId?: string;
  iconClassName: string;
};

type ModelFilterSummaryBarProps = {
  activeFilterCount: number;
  shownCount: number;
  totalCount: number;
  onEdit: () => void;
  onClear: () => void;
};

const ModelFilterSummaryBar: React.FC<ModelFilterSummaryBarProps> = ({
  activeFilterCount,
  shownCount,
  totalCount,
  onEdit,
  onClear,
}) => {
  const { t } = useTranslation();
  const activeFilterLabel =
    activeFilterCount === 1
      ? t("modelSelector.filter.activeSummaryOne", "1 filter active")
      : t("modelSelector.filter.activeSummary", {
          count: activeFilterCount,
          defaultValue: "{{count}} filters active",
        });

  return (
    <div className="sticky top-3 z-30">
      <div className="flex flex-wrap items-center justify-between gap-2 rounded-xl border border-emerald-500/45 bg-[#121f1a]/90 px-3 py-2 text-xs text-emerald-100 shadow-[0_10px_30px_rgba(0,0,0,0.28),0_0_24px_rgba(52,211,153,0.18)] backdrop-blur-md">
        <button
          type="button"
          onClick={onEdit}
          className="flex min-w-0 flex-1 items-center gap-2 text-left transition-colors hover:text-white"
          title={t("modelSelector.filter.scrollToFilter", "Filter is active - click to scroll to filter")}
        >
          <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full border border-emerald-400/40 bg-emerald-400/10">
            <Filter className="h-3 w-3 text-emerald-300" />
          </span>
          <span className="min-w-0 truncate">
            <span className="font-medium">
              {activeFilterLabel}
            </span>
            <span className="mx-1.5 text-emerald-500/70">·</span>
            <span className="text-emerald-200/80">
              {t("modelSelector.filter.modelsShown", {
                shown: shownCount,
                total: totalCount,
                defaultValue: "{{shown}} / {{total}} models shown",
              })}
            </span>
          </span>
        </button>
        <div className="flex shrink-0 items-center gap-1.5">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={onEdit}
            className="!px-2.5 !py-1 !text-[11px] !text-emerald-100 hover:!border-emerald-500/35 hover:!bg-emerald-500/10"
          >
            {t("common.edit", "Edit")}
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={onClear}
            className="flex items-center gap-1.5 !px-2.5 !py-1 !text-[11px] !text-emerald-100 hover:!border-emerald-500/35 hover:!bg-emerald-500/10"
          >
            <RotateCcw className="h-3 w-3" />
            {t("modelSelector.filter.reset", "Reset")}
          </Button>
        </div>
      </div>
    </div>
  );
};

export const ModelsSettings: React.FC = () => {
  const { t } = useTranslation();
  const {
    models,
    currentModel,
    downloadProgress,
    downloadingModels,
    extractingModels,
    loading,
    selectModel,
    downloadModel,
    cancelDownload,
    deleteModel,
  } = useModels();
  const {
    getSetting,
    setTranscriptionProvider,
    updateRemoteSttModelId,
    refreshSettings,
  } = useSettings();
  const [switchingModelId, setSwitchingModelId] = useState<string | null>(null);
  const [switchingRemoteApiId, setSwitchingRemoteApiId] =
    useState<RemoteApiRowId | null>(null);
  const {
    filters,
    isAnyFilterActive,
    applyFilters,
    resetFilters,
    setSearch,
    toggleSetValue,
    toggleBoolean,
    toggleRecommended,
  } = useModelFilters();

  const filterBarRef = useRef<HTMLDivElement>(null);

  const scrollToFilter = () => {
    filterBarRef.current?.scrollIntoView({ behavior: "smooth", block: "center" });
  };

  const transcriptionProvider = String(
    getSetting("transcription_provider") || "local",
  );
  const remoteStt = (getSetting("remote_stt") || {}) as RemoteSttSettingsConfig;
  const remotePreset = remoteStt.provider_preset ?? "groq";
  const remoteModelId = remoteStt.model_id ?? "";
  const isRemoteProvider =
    transcriptionProvider === "remote_openai_compatible" ||
    transcriptionProvider === "remote_soniox" ||
    transcriptionProvider === "remote_deepgram";
  const activeRemoteApiId: RemoteApiRowId | null =
    transcriptionProvider !== "remote_openai_compatible"
      ? null
      : remotePreset === "groq"
        ? "groq"
        : remotePreset === "custom"
          ? "custom"
          : remoteModelId === "gpt-realtime-whisper"
            ? "openai_realtime_whisper"
          : remoteModelId === "gpt-realtime-translate"
            ? "openai_translate"
            : "openai_realtime2";
  const remoteApiRows: RemoteApiRow[] = [
    {
      id: "groq",
      title: "Remote via Groq",
      description: "Groq OpenAI-compatible transcription",
      preset: "groq",
      modelId: "whisper-large-v3-turbo",
      iconClassName: "text-sky-400",
    },
    {
      id: "custom",
      title: "Remote via Custom API",
      description: "Custom OpenAI-compatible transcription endpoint",
      preset: "custom",
      iconClassName: "text-slate-300",
    },
    {
      id: "openai_realtime_whisper",
      title: "Remote via OpenAI gpt-realtime-whisper",
      description: "Native Realtime transcription model with optional flattened STT mode",
      notRecommended: true,
      preset: "openai",
      modelId: "gpt-realtime-whisper",
      iconClassName: "text-emerald-400",
    },
    {
      id: "openai_realtime2",
      title: "Remote via OpenAI gpt-realtime-2 STT Hack",
      description: "Voice-agent model coerced into transcript-only output",
      notRecommended: true,
      preset: "openai",
      modelId: "gpt-realtime-2",
      iconClassName: "text-blue-400",
    },
    {
      id: "openai_translate",
      title: "Remote via OpenAI gpt-realtime-translate",
      description: "Dedicated Realtime translation model with matching input/output language",
      notRecommended: true,
      preset: "openai",
      modelId: "gpt-realtime-translate",
      iconClassName: "text-violet-400",
    },
  ];
  const primaryRemoteApiRows = remoteApiRows.filter(
    (row) => !row.notRecommended,
  );
  const discouragedRemoteApiRows = remoteApiRows.filter(
    (row) => row.notRecommended,
  );

  const downloadedModels = useMemo(
    () =>
      models
        .filter((model: ModelInfo) => model.is_downloaded)
        .sort((a, b) => {
          if (a.is_custom === b.is_custom) return 0;
          return a.is_custom ? 1 : -1;
        }),
    [models],
  );
  const downloadableModels = useMemo(
    () => models.filter((model: ModelInfo) => !model.is_downloaded),
    [models],
  );
  const allLocalModels = useMemo(
    () => [...downloadedModels, ...downloadableModels],
    [downloadedModels, downloadableModels],
  );
  const filteredDownloaded = useMemo(
    () => applyFilters(downloadedModels),
    [downloadedModels, applyFilters],
  );
  const filteredDownloadable = useMemo(
    () => applyFilters(downloadableModels),
    [downloadableModels, applyFilters],
  );
  const activeFilterCount = useMemo(() => {
    return (
      (filters.search !== "" ? 1 : 0) +
      filters.engines.size +
      filters.sizeRanges.size +
      filters.languages.size +
      (filters.supportsTranslation !== null ? 1 : 0) +
      (filters.supportsStreaming !== null ? 1 : 0) +
      (filters.recommendedOnly ? 1 : 0)
    );
  }, [filters]);
  const shownLocalModelsCount = filteredDownloaded.length + filteredDownloadable.length;
  const customModelsCount = useMemo(
    () => downloadedModels.filter((model) => model.is_custom).length,
    [downloadedModels],
  );

  const ensureLocalProvider = async () => {
    if (isRemoteProvider) {
      await setTranscriptionProvider("local");
    }
  };

  const handleSelectModel = async (modelId: string) => {
    setSwitchingModelId(modelId);
    try {
      await ensureLocalProvider();
      await selectModel(modelId);
    } finally {
      setSwitchingModelId(null);
    }
  };

  const handleDownloadModel = async (modelId: string) => {
    await ensureLocalProvider();
    await downloadModel(modelId);
  };

  const handleRemoteApiSelect = async (row: RemoteApiRow) => {
    setSwitchingRemoteApiId(row.id);
    try {
      const presetResult = await commands.changeRemoteSttProviderPresetSetting(
        row.preset,
      );
      if (presetResult.status === "error") {
        throw new Error(presetResult.error);
      }
      if (row.modelId) {
        await updateRemoteSttModelId(row.modelId);
      }
      await setTranscriptionProvider("remote_openai_compatible");
      await refreshSettings();
    } finally {
      setSwitchingRemoteApiId(null);
    }
  };

  const handleDeleteModel = async (model: ModelInfo) => {
    const modelName = getTranslatedModelName(model, t);
    const confirmed = await ask(
      `${t("modelSelector.deleteModel", { modelName })}?`,
      {
        title: t("common.delete"),
        kind: "warning",
      },
    );
    if (!confirmed) return;
    await deleteModel(model.id);
  };

  const renderRemoteApiRows = (rows: RemoteApiRow[]) =>
    rows.map((row) => {
      const isActive = activeRemoteApiId === row.id;
      return (
        <React.Fragment key={row.id}>
          <div
            className={`px-6 py-4 flex flex-col gap-3 transition-colors ${
              isActive ? "bg-green-500/5" : ""
            }`}
          >
            <div className="flex items-center justify-between gap-4">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <Cloud className={`w-4 h-4 ${row.iconClassName}`} />
                  <p className="text-sm font-medium text-[#f5f5f5]">
                    {row.title}
                  </p>
                  {isActive && (
                    <span className={`text-xs ${row.iconClassName}`}>
                      {t("modelSelector.active")}
                    </span>
                  )}
                </div>
                {(isActive || row.notRecommended) && (
                  <p className="text-xs text-[#a0a0a0] mt-1">
                    {row.notRecommended && (
                      <>
                        <span className="font-medium text-red-400">
                          (Not recommended)
                        </span>{" "}
                      </>
                    )}
                    {row.description}
                  </p>
                )}
              </div>
              {!isActive && (
                <Button
                  variant="secondary"
                  size="sm"
                  disabled={switchingRemoteApiId === row.id}
                  onClick={() => void handleRemoteApiSelect(row)}
                >
                  {t("modelSelector.chooseModel")}
                </Button>
              )}
            </div>
            {isActive && (
              <div className="border-t border-[#3d3d3d] pt-3">
                <RemoteSttSettings
                  descriptionMode="tooltip"
                  grouped={true}
                  hideProviderSelector
                  hideRemoteInterfaceSelector
                />
              </div>
            )}
          </div>
          <div className="border-t border-[#3d3d3d]" />
        </React.Fragment>
      );
    });

  return (
    <div className="max-w-3xl w-full mx-auto space-y-8 pb-12">
      {/* Help Section */}
      <TellMeMore title={t("modelSelector.tellMeMore.title")}>
        <div className="space-y-3">
          <p>
            <strong>{t("modelSelector.tellMeMore.headline")}</strong>
          </p>
          <p className="opacity-90">{t("modelSelector.tellMeMore.intro")}</p>
          <ul className="list-disc list-inside space-y-2 ml-1 opacity-90">
            <li>
              <strong>{t("modelSelector.tellMeMore.remoteApi.title")}</strong>{" "}
              {t("modelSelector.tellMeMore.remoteApi.description")}
            </li>
            <li>
              <strong>
                {t("modelSelector.tellMeMore.remoteSoniox.title")}
              </strong>{" "}
              {t("modelSelector.tellMeMore.remoteSoniox.description")}
            </li>
            <li>
              <strong>
                {t(
                  "modelSelector.tellMeMore.remoteDeepgram.title",
                  "Remote via Deepgram",
                )}
              </strong>{" "}
              {t(
                "modelSelector.tellMeMore.remoteDeepgram.description",
                "Uses Deepgram live streaming API with Nova models and control messages (Finalize, KeepAlive, CloseStream).",
              )}
            </li>
            <li>
              <strong>{t("modelSelector.tellMeMore.localModels.title")}</strong>{" "}
              {t("modelSelector.tellMeMore.localModels.description")}
            </li>
          </ul>
          <p className="pt-2 text-xs text-text/70">
            {t("modelSelector.tellMeMore.tip")}
          </p>
        </div>
      </TellMeMore>

      {/* Remote Providers */}
      <SettingsGroup title={t("modelSelector.remoteMode")}>
        {renderRemoteApiRows(primaryRemoteApiRows)}

        {/* Remote via Soniox */}
        <div
          className={`px-6 py-4 flex flex-col gap-3 transition-colors ${
            transcriptionProvider === "remote_soniox" ? "bg-green-500/5" : ""
          }`}
        >
          <div className="flex items-center justify-between">
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <Radio className="w-4 h-4 text-teal-400" />
                <p className="text-sm font-medium text-[#f5f5f5]">
                  {t("modelSelector.remoteSonioxMode")}
                </p>
                {transcriptionProvider === "remote_soniox" && (
                  <span className="text-xs text-teal-400">
                    {t("modelSelector.active")}
                  </span>
                )}
              </div>
              {transcriptionProvider === "remote_soniox" && (
                <p className="text-xs text-[#a0a0a0] mt-1">
                  {t("modelSelector.remoteSonioxModeDescription")}
                </p>
              )}
            </div>
            {transcriptionProvider !== "remote_soniox" && (
              <Button
                variant="secondary"
                size="sm"
                onClick={() => setTranscriptionProvider("remote_soniox")}
              >
                {t("modelSelector.chooseModel")}
              </Button>
            )}
          </div>
          {transcriptionProvider === "remote_soniox" && (
            <div className="border-t border-[#3d3d3d] pt-3">
              <RemoteSttSettings
                descriptionMode="tooltip"
                grouped={true}
                hideProviderSelector
              />
            </div>
          )}
        </div>

        <div className="border-t border-[#3d3d3d]" />

        {/* Remote via Deepgram */}
        <div
          className={`px-6 py-4 flex flex-col gap-3 transition-colors ${
            transcriptionProvider === "remote_deepgram" ? "bg-green-500/5" : ""
          }`}
        >
          <div className="flex items-center justify-between">
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <Radio className="w-4 h-4 text-cyan-400" />
                <p className="text-sm font-medium text-[#f5f5f5]">
                  {t("modelSelector.remoteDeepgramMode", "Remote via Deepgram")}
                </p>
                {transcriptionProvider === "remote_deepgram" && (
                  <span className="text-xs text-cyan-400">
                    {t("modelSelector.active")}
                  </span>
                )}
              </div>
              {transcriptionProvider === "remote_deepgram" && (
                <p className="text-xs text-[#a0a0a0] mt-1">
                  {t(
                    "modelSelector.remoteDeepgramModeDescription",
                    "Deepgram Nova streaming service",
                  )}
                </p>
              )}
            </div>
            {transcriptionProvider !== "remote_deepgram" && (
              <Button
                variant="secondary"
                size="sm"
                onClick={() => setTranscriptionProvider("remote_deepgram")}
              >
                {t("modelSelector.chooseModel")}
              </Button>
            )}
          </div>
          {transcriptionProvider === "remote_deepgram" && (
            <div className="border-t border-[#3d3d3d] pt-3">
              <RemoteSttSettings
                descriptionMode="tooltip"
                grouped={true}
                hideProviderSelector
              />
            </div>
          )}
        </div>

        <div className="border-t border-[#3d3d3d]" />

        {renderRemoteApiRows(discouragedRemoteApiRows)}
      </SettingsGroup>

      <div className="glass-panel-subtle border border-[#3d3d3d] rounded-xl p-4">
        <p className="text-sm text-[#f5f5f5]">
          {t("modelSelector.customModelsHelpTitle")}
        </p>
        <p className="text-xs text-[#a0a0a0] mt-1">
          {t("modelSelector.customModelsHelpDescription")}
        </p>
        <p className="text-xs text-[#8a8a8a] mt-2">
          {customModelsCount > 0
            ? t("modelSelector.customModelsDetected", {
                count: customModelsCount,
              })
            : t("modelSelector.customModelsHelpHint")}
        </p>
      </div>

      <ModelFilterBar
        filterBarRef={filterBarRef}
        allLocalModels={allLocalModels}
        filters={filters}
        isAnyFilterActive={isAnyFilterActive}
        onSearch={setSearch}
        onToggleSet={toggleSetValue}
        onToggleBoolean={toggleBoolean}
        onToggleRecommended={toggleRecommended}
        onReset={resetFilters}
      />

      {isAnyFilterActive && (
        <ModelFilterSummaryBar
          activeFilterCount={activeFilterCount}
          shownCount={shownLocalModelsCount}
          totalCount={allLocalModels.length}
          onEdit={scrollToFilter}
          onClear={resetFilters}
        />
      )}

      <SettingsGroup title={t("modelSelector.availableModels")}>
        {loading && (
          <div className="px-6 py-4 text-sm text-[#a0a0a0]">
            {t("common.loading")}
          </div>
        )}

        {!loading && downloadedModels.length === 0 && (
          <div className="px-6 py-4 text-sm text-[#a0a0a0]">
            {t("modelSelector.noModelsAvailable")}
          </div>
        )}

        {!loading && downloadedModels.length > 0 && filteredDownloaded.length === 0 && (
          <div className="px-6 py-4 text-sm text-[#a0a0a0]">
            {t("modelSelector.filter.noResults", "No models match the current filters")}
          </div>
        )}

        {!loading &&
          filteredDownloaded.map((model) => {
            const modelName = getTranslatedModelName(model, t);
            const isActive = model.id === currentModel && !isRemoteProvider;
            const isSwitching = switchingModelId === model.id;

            return (
              <div
                key={model.id}
                className={`px-6 py-4 flex flex-col gap-3 md:flex-row md:items-center md:justify-between transition-colors ${
                  isActive ? "bg-green-500/5" : ""
                }`}
              >
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <Cpu className="w-4 h-4 text-[#a0a0a0]" />
                    <p className="text-sm font-medium text-[#f5f5f5]">
                      {modelName}
                    </p>
                    {model.is_custom && (
                      <span className="text-[10px] tracking-wide uppercase text-[#a0a0a0]">
                        {t("modelSelector.custom")}
                      </span>
                    )}
                    {isActive && (
                      <span className="text-xs text-[#ff4d8d]">
                        {t("modelSelector.active")}
                      </span>
                    )}
                  </div>
                  <p className="text-xs text-[#a0a0a0] mt-1">
                    {getTranslatedModelDescription(model, t)}
                  </p>
                  <p className="mt-1 flex items-center gap-1.5 text-xs text-[#8a8a8a]">
                    <HardDrive className="h-3.5 w-3.5" />
                    <span>
                      {t("modelSelector.downloadSize")} ·{" "}
                      {formatModelSize(Number(model.size_mb))}
                    </span>
                  </p>
                  <ModelMetadataPanel model={model} />
                </div>

                <div className="flex items-center gap-2">
                  {!isActive && (
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => handleSelectModel(model.id)}
                      disabled={isSwitching}
                    >
                      {isSwitching
                        ? t("modelSelector.loadingGeneric")
                        : t("modelSelector.chooseModel")}
                    </Button>
                  )}
                  <Button
                    variant="danger"
                    size="sm"
                    onClick={() => handleDeleteModel(model)}
                  >
                    {t("common.delete")}
                  </Button>
                </div>
              </div>
            );
          })}
      </SettingsGroup>

      <SettingsGroup title={t("modelSelector.downloadModels")}>
        {downloadableModels.length === 0 && (
          <div className="px-6 py-4 text-sm text-[#a0a0a0]">
            {t("modelSelector.noModelsAvailable")}
          </div>
        )}

        {downloadableModels.length > 0 && filteredDownloadable.length === 0 && (
          <div className="px-6 py-4 text-sm text-[#a0a0a0]">
            {t("modelSelector.filter.noResults", "No models match the current filters")}
          </div>
        )}

        {filteredDownloadable.map((model) => {
          const isDownloading = downloadingModels.has(model.id);
          const isExtracting = extractingModels.has(model.id);
          const progress = downloadProgress.get(model.id);
          const percent = progress
            ? Math.max(0, Math.min(100, Math.round(progress.percentage)))
            : 0;

          return (
            <div
              key={model.id}
              className="px-6 py-4 flex flex-col gap-3 md:flex-row md:items-center md:justify-between"
            >
              <div className="min-w-0">
                <p className="text-sm font-medium text-[#f5f5f5]">
                  {getTranslatedModelName(model, t)}
                </p>
                <p className="text-xs text-[#a0a0a0] mt-1">
                  {getTranslatedModelDescription(model, t)}
                </p>
                <ModelMetadataPanel model={model} />
                <p className="text-xs text-[#8a8a8a] mt-1">
                  {t("modelSelector.downloadSize")} ·{" "}
                  {formatModelSize(Number(model.size_mb))}
                </p>
                {isDownloading && (
                  <p className="text-xs text-[#ff4d8d] mt-1">
                    {t("modelSelector.downloading", { percentage: percent })}
                  </p>
                )}
                {isExtracting && (
                  <p className="text-xs text-[#ff4d8d] mt-1">
                    {t("modelSelector.extractingGeneric")}
                  </p>
                )}
              </div>

              <div className="flex items-center gap-2">
                {isDownloading ? (
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => cancelDownload(model.id)}
                  >
                    {t("common.cancel")}
                  </Button>
                ) : (
                  <Button
                    variant="primary"
                    size="sm"
                    onClick={() => handleDownloadModel(model.id)}
                    disabled={isExtracting}
                  >
                    {t("modelSelector.download")}
                  </Button>
                )}
              </div>
            </div>
          );
        })}
      </SettingsGroup>
    </div>
  );
};
