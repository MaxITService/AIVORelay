import React, { useMemo, useState, useRef } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { ask } from "@tauri-apps/plugin-dialog";
import { type as getOsType } from "@tauri-apps/plugin-os";
import { Cloud, Download, Filter, HardDrive, Radio, RotateCcw } from "lucide-react";
import { useModels } from "../../../hooks/useModels";
import { useSettings } from "../../../hooks/useSettings";
import { useModelFilters } from "../../../hooks/useModelFilters";
import {
  getTranslatedModelDescription,
  getTranslatedModelName,
} from "../../../lib/utils/modelTranslation";
import { formatModelSize } from "../../../lib/utils/format";
import { sessionToast as toast } from "../../../lib/sessionToast";
import { Button } from "../../ui/Button";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { TellMeMore } from "../../ui/TellMeMore";
import { RemoteSttSettings } from "../remote-stt/RemoteSttSettings";
import { ModelMetadataPanel } from "./ModelMetadataPanel";
import { ModelFilterBar } from "./ModelFilterBar";
import { ModelReleaseDate } from "../../shared/ModelReleaseDate";
import { invalidateModelDownloadActivationIntent } from "@/lib/modelDownloadActivation";
import {
  commands,
  type ModelInfo,
  type RemoteSttSettings as RemoteSttSettingsConfig,
  type SttModelSelection,
} from "@/bindings";

type RemoteApiRowId =
  | "groq"
  | "gemini_transcribe"
  | "gemini_live"
  | "openai_transcribe"
  | "openai_live_transcribe"
  | "openai_realtime_whisper"
  | "openai_realtime2"
  | "openai_realtime2_1"
  | "openai_translate"
  | "custom";

type RemoteApiRow = {
  id: RemoteApiRowId;
  title: string;
  description: string;
  notRecommended?: boolean;
  preset: "groq" | "openai" | "vercel" | "google" | "custom";
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
  const supportsRemoteProviders = getOsType() === "windows";
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
    settings,
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
    setReleasedAfter,
  } = useModelFilters();

  const filterBarRef = useRef<HTMLDivElement>(null);

  const scrollToFilter = () => {
    filterBarRef.current?.scrollIntoView({ behavior: "smooth", block: "center" });
  };

  const transcriptionProvider = String(
    getSetting("transcription_provider") || "local",
  );
  const activeProfile = useMemo(() => {
    const activeProfileId = settings?.active_profile_id ?? "default";
    if (activeProfileId === "default") return null;
    return (
      (settings?.transcription_profiles ?? []).find(
        (profile) => profile.id === activeProfileId,
      ) ?? null
    );
  }, [settings?.active_profile_id, settings?.transcription_profiles]);
  const profileModelSelection = activeProfile?.stt_model_selection_override;
  const effectiveTranscriptionProvider = String(
    profileModelSelection?.provider ?? transcriptionProvider,
  );
  const remoteStt = (getSetting("remote_stt") || {}) as RemoteSttSettingsConfig;
  const remotePreset =
    profileModelSelection?.provider === "remote_openai_compatible"
      ? profileModelSelection.provider_preset ||
        remoteStt.provider_preset ||
        "groq"
      : remoteStt.provider_preset ?? "groq";
  const remoteModelId =
    profileModelSelection?.provider === "remote_openai_compatible"
      ? profileModelSelection.model_id || remoteStt.model_id || ""
      : remoteStt.model_id ?? "";
  const isRemoteProvider =
    effectiveTranscriptionProvider === "remote_openai_compatible" ||
    effectiveTranscriptionProvider === "remote_soniox" ||
    effectiveTranscriptionProvider === "remote_deepgram";
  const activeRemoteApiId: RemoteApiRowId | null =
    effectiveTranscriptionProvider !== "remote_openai_compatible"
      ? null
      : remotePreset === "groq"
        ? "groq"
        : remotePreset === "vercel" || remotePreset === "google"
          ? (remoteModelId === "google/gemini-3.5-transcribe-live" ||
             remoteModelId === "gemini-3.5-transcribe-live"
            ? "gemini_live"
            : "gemini_transcribe")
        : remotePreset === "custom"
          ? "custom"
          : remoteModelId === "gpt-transcribe"
            ? "openai_transcribe"
          : remoteModelId === "gpt-live-transcribe"
            ? "openai_live_transcribe"
          : remoteModelId === "gpt-realtime-whisper"
            ? "openai_realtime_whisper"
          : remoteModelId === "gpt-realtime-2"
            ? "openai_realtime2"
          : remoteModelId === "gpt-realtime-2.1"
            ? "openai_realtime2_1"
          : remoteModelId === "gpt-realtime-translate"
            ? "openai_translate"
            : "openai_realtime2_1";
  const remoteApiRows: RemoteApiRow[] = [
    {
      id: "groq",
      title: t("modelSelector.remoteApiRows.groq.title"),
      description: t("modelSelector.remoteApiRows.groq.description"),
      preset: "groq",
      modelId: "whisper-large-v3-turbo",
      iconClassName: "text-sky-400",
    },
    {
      id: "gemini_transcribe",
      title: t("modelSelector.remoteApiRows.geminiTranscribe.title"),
      description: t("modelSelector.remoteApiRows.geminiTranscribe.description"),
      preset: "vercel",
      modelId: "google/gemini-3.5-transcribe",
      iconClassName: "text-amber-300",
    },
    {
      id: "gemini_live",
      title: t("modelSelector.remoteApiRows.geminiLive.title"),
      description: t("modelSelector.remoteApiRows.geminiLive.description"),
      preset: "vercel",
      modelId: "google/gemini-3.5-transcribe-live",
      iconClassName: "text-amber-400",
    },
    {
      id: "custom",
      title: t("modelSelector.remoteApiRows.custom.title"),
      description: t("modelSelector.remoteApiRows.custom.description"),
      preset: "custom",
      iconClassName: "text-slate-300",
    },
    {
      id: "openai_transcribe",
      title: t("modelSelector.remoteApiRows.openAiTranscribe.title"),
      description: t("modelSelector.remoteApiRows.openAiTranscribe.description"),
      preset: "openai",
      modelId: "gpt-transcribe",
      iconClassName: "text-emerald-400",
    },
    {
      id: "openai_live_transcribe",
      title: t("modelSelector.remoteApiRows.openAiLiveTranscribe.title"),
      description: t("modelSelector.remoteApiRows.openAiLiveTranscribe.description"),
      preset: "openai",
      modelId: "gpt-live-transcribe",
      iconClassName: "text-cyan-400",
    },
    {
      id: "openai_realtime_whisper",
      title: t("modelSelector.remoteApiRows.openAiRealtimeWhisper.title"),
      description: t("modelSelector.remoteApiRows.openAiRealtimeWhisper.description"),
      notRecommended: true,
      preset: "openai",
      modelId: "gpt-realtime-whisper",
      iconClassName: "text-emerald-400",
    },
    {
      id: "openai_realtime2",
      title: t("modelSelector.remoteApiRows.openAiRealtime2.title"),
      description: t("modelSelector.remoteApiRows.openAiRealtime2.description"),
      notRecommended: true,
      preset: "openai",
      modelId: "gpt-realtime-2",
      iconClassName: "text-blue-400",
    },
    {
      id: "openai_realtime2_1",
      title: t("modelSelector.remoteApiRows.openAiRealtime21.title"),
      description: t("modelSelector.remoteApiRows.openAiRealtime21.description"),
      notRecommended: true,
      preset: "openai",
      modelId: "gpt-realtime-2.1",
      iconClassName: "text-cyan-400",
    },
    {
      id: "openai_translate",
      title: t("modelSelector.remoteApiRows.openAiTranslate.title"),
      description: t("modelSelector.remoteApiRows.openAiTranslate.description"),
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
      (filters.recommendedOnly ? 1 : 0) +
      (filters.releasedAfter !== "" ? 1 : 0)
    );
  }, [filters]);
  const shownLocalModelsCount = filteredDownloaded.length + filteredDownloadable.length;
  const customModelsCount = useMemo(
    () => downloadedModels.filter((model) => model.is_custom).length,
    [downloadedModels],
  );

  const setActiveProfileModelSelection = async (
    selection: SttModelSelection,
    modelLabel: string,
  ) => {
    if (!activeProfile) return false;

    await invoke("change_active_profile_stt_model_selection_override", {
      selection,
    });
    await refreshSettings();
    toast.success(
      t("modelSelector.profileModelChanged", {
        profile: activeProfile.name,
        model: modelLabel,
        defaultValue: 'Profile "{{profile}}" now uses {{model}}.',
      }),
    );
    return true;
  };

  const notifyDefaultProfileModelSelection = (modelLabel: string) => {
    toast.success(
      t("modelSelector.profileModelChanged", {
        profile: t("settings.transcriptionProfiles.defaultProfile", "Default"),
        model: modelLabel,
      }),
    );
  };

  const ensureLocalProvider = async () => {
    if (isRemoteProvider) {
      await setTranscriptionProvider("local");
    }
  };

  const handleSelectModel = async (modelId: string) => {
    setSwitchingModelId(modelId);
    try {
      const selectedModel = allLocalModels.find((model) => model.id === modelId);
      if (
        await setActiveProfileModelSelection(
          {
            provider: "local",
            model_id: modelId,
            provider_preset: "",
          },
          selectedModel ? getTranslatedModelName(selectedModel, t) : modelId,
        )
      ) {
        return;
      }

      await ensureLocalProvider();
      const selected = await selectModel(modelId);
      if (!selected) return;
      notifyDefaultProfileModelSelection(
        selectedModel ? getTranslatedModelName(selectedModel, t) : modelId,
      );

      // A model can have direct final output enabled before it becomes active.
      // Re-apply the setting after selection so enabling that model cannot
      // leave the persisted Preview toggle in a conflicting state.
      const directOutputModels = (getSetting(
        "native_streaming_live_output_models",
      ) || []) as string[];
      if (
        directOutputModels.includes(modelId) &&
        selectedModel?.engine_type === "TranscribeCpp" &&
        selectedModel.supports_streaming
      ) {
        const result =
          await commands.changeNativeStreamingLiveOutputModelSetting(
            modelId,
            true,
          );
        if (result.status === "error") throw new Error(result.error);
        if (result.status === "ok" && result.data) {
          await refreshSettings();
          toast.info(t("modelSelector.nativeLiveOutput.previewDisabledTitle"), {
            description: t(
              "modelSelector.nativeLiveOutput.previewDisabledDescription",
              { model: selectedModel.name || modelId },
            ),
          });
        }
      }
    } finally {
      setSwitchingModelId(null);
    }
  };

  const handleDownloadModel = async (modelId: string) => {
    await downloadModel(modelId);
  };

  const handleRemoteApiSelect = async (row: RemoteApiRow) => {
    invalidateModelDownloadActivationIntent();
    setSwitchingRemoteApiId(row.id);
    try {
      const isGeminiRow =
        row.id === "gemini_transcribe" || row.id === "gemini_live";
      const selectedPreset =
        isGeminiRow && (remotePreset === "vercel" || remotePreset === "google")
          ? remotePreset
          : row.preset;
      const selectedModelId = isGeminiRow
        ? row.id === "gemini_live"
          ? selectedPreset === "google"
            ? "gemini-3.5-transcribe-live"
            : "google/gemini-3.5-transcribe-live"
          : selectedPreset === "google"
            ? "gemini-3.5-transcribe"
            : "google/gemini-3.5-transcribe"
        : row.modelId;
      if (
        selectedModelId &&
        (await setActiveProfileModelSelection(
          {
            provider: "remote_openai_compatible",
            model_id: selectedModelId,
            provider_preset: selectedPreset,
          },
          row.title.replace(/^Remote via /, ""),
        ))
      ) {
        return;
      }
      const presetResult = await commands.changeRemoteSttProviderPresetSetting(
        selectedPreset,
      );
      if (presetResult.status === "error") {
        throw new Error(presetResult.error);
      }
      if (selectedModelId) {
        await updateRemoteSttModelId(selectedModelId);
      }
      await setTranscriptionProvider("remote_openai_compatible");
      await refreshSettings();
      notifyDefaultProfileModelSelection(
        row.title.replace(/^Remote via /, ""),
      );
    } catch (error) {
      toast.error(String(error));
    } finally {
      setSwitchingRemoteApiId(null);
    }
  };

  const handleRemoteProviderSelect = async (
    provider: string,
    modelId: string,
    modelLabel: string,
  ) => {
    invalidateModelDownloadActivationIntent();
    try {
      if (
        await setActiveProfileModelSelection(
          {
            provider: provider as SttModelSelection["provider"],
            model_id: modelId,
            provider_preset: "",
          },
          modelLabel,
        )
      ) {
        return;
      }
      await setTranscriptionProvider(provider);
      notifyDefaultProfileModelSelection(modelLabel);
    } catch (error) {
      toast.error(String(error));
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

      {/* Remote providers depend on Windows Credential Manager. */}
      {supportsRemoteProviders && (
        <SettingsGroup title={t("modelSelector.remoteMode")}>
        {renderRemoteApiRows(primaryRemoteApiRows)}

        {/* Remote via Soniox */}
        <div
          className={`px-6 py-4 flex flex-col gap-3 transition-colors ${
            effectiveTranscriptionProvider === "remote_soniox"
              ? "bg-green-500/5"
              : ""
          }`}
        >
          <div className="flex items-center justify-between">
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <Radio className="w-4 h-4 text-teal-400" />
                <p className="text-sm font-medium text-[#f5f5f5]">
                  {t("modelSelector.remoteSonioxMode")}
                </p>
                {effectiveTranscriptionProvider === "remote_soniox" && (
                  <span className="text-xs text-teal-400">
                    {t("modelSelector.active")}
                  </span>
                )}
              </div>
              {effectiveTranscriptionProvider === "remote_soniox" && (
                <p className="text-xs text-[#a0a0a0] mt-1">
                  {t("modelSelector.remoteSonioxModeDescription")}
                </p>
              )}
            </div>
            {effectiveTranscriptionProvider !== "remote_soniox" && (
              <Button
                variant="secondary"
                size="sm"
                onClick={() =>
                  void handleRemoteProviderSelect(
                    "remote_soniox",
                    String(getSetting("soniox_model") || "stt-rt-v5"),
                    "Soniox",
                  )
                }
              >
                {t("modelSelector.chooseModel")}
              </Button>
            )}
          </div>
          {effectiveTranscriptionProvider === "remote_soniox" && (
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
            effectiveTranscriptionProvider === "remote_deepgram"
              ? "bg-green-500/5"
              : ""
          }`}
        >
          <div className="flex items-center justify-between">
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <Radio className="w-4 h-4 text-cyan-400" />
                <p className="text-sm font-medium text-[#f5f5f5]">
                  {t("modelSelector.remoteDeepgramMode", "Remote via Deepgram")}
                </p>
                {effectiveTranscriptionProvider === "remote_deepgram" && (
                  <span className="text-xs text-cyan-400">
                    {t("modelSelector.active")}
                  </span>
                )}
              </div>
              {effectiveTranscriptionProvider === "remote_deepgram" && (
                <p className="text-xs text-[#a0a0a0] mt-1">
                  {t(
                    "modelSelector.remoteDeepgramModeDescription",
                    "Deepgram Nova streaming service",
                  )}
                </p>
              )}
            </div>
            {effectiveTranscriptionProvider !== "remote_deepgram" && (
              <Button
                variant="secondary"
                size="sm"
                onClick={() =>
                  void handleRemoteProviderSelect(
                    "remote_deepgram",
                    String(getSetting("deepgram_model") || "nova-3"),
                    "Deepgram",
                  )
                }
              >
                {t("modelSelector.chooseModel")}
              </Button>
            )}
          </div>
          {effectiveTranscriptionProvider === "remote_deepgram" && (
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
      )}

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
        onReleasedAfterChange={setReleasedAfter}
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
            const effectiveLocalModelId =
              profileModelSelection?.provider === "local"
                ? profileModelSelection.model_id
                : currentModel;
            const isActive =
              model.id === effectiveLocalModelId && !isRemoteProvider;
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
                    <span
                      title={t("modelSelector.downloadedTooltip", "Downloaded")}
                      aria-label={t(
                        "modelSelector.downloadedTooltip",
                        "Downloaded",
                      )}
                    >
                      <HardDrive
                        aria-hidden="true"
                        className="h-4 w-4 text-[#a0a0a0]"
                      />
                    </span>
                    <p className="text-sm font-medium text-[#f5f5f5]">
                      <ModelReleaseDate
                        modelId={model.id}
                        className="mr-2 text-[10px] font-normal text-[#777]"
                      />
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
                    <Download aria-hidden="true" className="h-3.5 w-3.5" />
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
          const effectiveLocalModelId =
            profileModelSelection?.provider === "local"
              ? profileModelSelection.model_id
              : currentModel;
          const isActive =
            model.id === effectiveLocalModelId && !isRemoteProvider;
          const isSwitching = switchingModelId === model.id;
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
                  <ModelReleaseDate
                    modelId={model.id}
                    className="mr-2 text-[10px] font-normal text-[#777]"
                  />
                  {getTranslatedModelName(model, t)}
                  {isActive && (
                    <span className="ml-2 text-xs text-[#ff4d8d]">
                      {t("modelSelector.active")}
                    </span>
                  )}
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
                {isActive && !isDownloading && !isExtracting && (
                  <p className="mt-1 text-xs text-red-400">
                    {t("modelSelector.selectedDownloadOnUse")}
                  </p>
                )}
              </div>

              <div className="flex items-center gap-2">
                {!isActive && (
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => handleSelectModel(model.id)}
                    disabled={isSwitching || isExtracting}
                  >
                    {isSwitching
                      ? t("modelSelector.loadingGeneric")
                      : t("modelSelector.chooseModel")}
                  </Button>
                )}
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
