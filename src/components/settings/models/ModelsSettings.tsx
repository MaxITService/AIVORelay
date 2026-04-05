import React, { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { ask } from "@tauri-apps/plugin-dialog";
import { Cloud, Cpu, FolderOpen, Radio, ShieldAlert } from "lucide-react";
import { commands, type ModelInfo } from "@/bindings";
import { useModels } from "../../../hooks/useModels";
import { useSettings } from "../../../hooks/useSettings";
import { getTranslatedModelDescription, getTranslatedModelName } from "../../../lib/utils/modelTranslation";
import { EXTERNAL_MODEL_DOWNLOADS, hasExternalModelDownload } from "../../../lib/utils/externalModelDownloads";
import { formatModelSize } from "../../../lib/utils/format";
import { Button } from "../../ui/Button";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { TellMeMore } from "../../ui/TellMeMore";
import { RemoteSttSettings } from "../remote-stt/RemoteSttSettings";
import { ExternalModelDownloadModal } from "./ExternalModelDownloadModal";
import { ModelMetadataPanel } from "./ModelMetadataPanel";

type ExternalDownloadInfo = {
  sourceLabel: string;
  sourceUrl: string;
  privacyUrl: string;
  termsUrl: string;
  destinationFolder: string;
  files: string[];
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
    error,
    selectModel,
    downloadModel,
    cancelDownload,
    deleteModel,
  } = useModels();
  const { getSetting, setTranscriptionProvider } = useSettings();
  const [switchingModelId, setSwitchingModelId] = useState<string | null>(null);
  const [externalDownloadModel, setExternalDownloadModel] = useState<ModelInfo | null>(null);
  const [externalDownloadInfo, setExternalDownloadInfo] = useState<ExternalDownloadInfo | null>(
    null,
  );

  const transcriptionProvider = String(
    getSetting("transcription_provider") || "local",
  );
  const isRemoteProvider =
    transcriptionProvider === "remote_openai_compatible" ||
    transcriptionProvider === "remote_soniox" ||
    transcriptionProvider === "remote_deepgram";

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
    () =>
      models.filter(
        (model: ModelInfo) =>
          !model.is_downloaded &&
          (Boolean(model.url) || hasExternalModelDownload(model.id)),
      ),
    [models],
  );
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
    const model = models.find((entry) => entry.id === modelId) || null;
    const external = EXTERNAL_MODEL_DOWNLOADS[modelId];
    if (model && external) {
      const appDirResult = await commands.getAppDirPath();
      if (appDirResult.status === "error") {
        throw new Error(appDirResult.error);
      }
      const destinationFolder = `${appDirResult.data}\\models\\${model.filename}`;
      setExternalDownloadInfo({
        ...external,
        destinationFolder,
      });
      setExternalDownloadModel(model);
      return;
    }
    await downloadModel(modelId);
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

  return (
    <div className="max-w-3xl w-full mx-auto space-y-8 pb-12">
      {/* Help Section */}
      <TellMeMore title={t("modelSelector.tellMeMore.title")}>
        <div className="space-y-3">
          <p>
            <strong>{t("modelSelector.tellMeMore.headline")}</strong>
          </p>
          <p className="opacity-90">
            {t("modelSelector.tellMeMore.intro")}
          </p>
          <ul className="list-disc list-inside space-y-2 ml-1 opacity-90">
            <li>
              <strong>{t("modelSelector.tellMeMore.remoteApi.title")}</strong>{" "}
              {t("modelSelector.tellMeMore.remoteApi.description")}
            </li>
            <li>
              <strong>{t("modelSelector.tellMeMore.remoteSoniox.title")}</strong>{" "}
              {t("modelSelector.tellMeMore.remoteSoniox.description")}
            </li>
            <li>
              <strong>
                {t("modelSelector.tellMeMore.remoteDeepgram.title", "Remote via Deepgram")}
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
        {/* Remote via API */}
        <div
          className={`px-6 py-4 flex flex-col gap-3 transition-colors ${
            transcriptionProvider === "remote_openai_compatible" ? "bg-green-500/5" : ""
          }`}
        >
          <div className="flex items-center justify-between">
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <Cloud className="w-4 h-4 text-blue-400" />
                <p className="text-sm font-medium text-[#f5f5f5]">
                  {t("modelSelector.remoteApiMode")}
                </p>
                {transcriptionProvider === "remote_openai_compatible" && (
                  <span className="text-xs text-blue-400">
                    {t("modelSelector.active")}
                  </span>
                )}
              </div>
              {transcriptionProvider === "remote_openai_compatible" && (
                <p className="text-xs text-[#a0a0a0] mt-1">
                  {t("modelSelector.remoteApiModeDescription")}
                </p>
              )}
            </div>
            {transcriptionProvider !== "remote_openai_compatible" && (
              <Button
                variant="secondary"
                size="sm"
                onClick={() => setTranscriptionProvider("remote_openai_compatible")}
              >
                {t("modelSelector.chooseModel")}
              </Button>
            )}
          </div>
          {transcriptionProvider === "remote_openai_compatible" && (
            <div className="border-t border-[#3d3d3d] pt-3">
              <RemoteSttSettings descriptionMode="tooltip" grouped={true} hideProviderSelector />
            </div>
          )}
        </div>

        <div className="border-t border-[#3d3d3d]" />

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
              <RemoteSttSettings descriptionMode="tooltip" grouped={true} hideProviderSelector />
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
              <RemoteSttSettings descriptionMode="tooltip" grouped={true} hideProviderSelector />
            </div>
          )}
        </div>
      </SettingsGroup>
      <ExternalModelDownloadModal
        isOpen={Boolean(externalDownloadModel && externalDownloadInfo)}
        modelName={
          externalDownloadModel
            ? getTranslatedModelName(externalDownloadModel, t)
            : ""
        }
        sourceLabel={externalDownloadInfo?.sourceLabel || ""}
        sourceUrl={externalDownloadInfo?.sourceUrl || ""}
        privacyUrl={externalDownloadInfo?.privacyUrl || ""}
        termsUrl={externalDownloadInfo?.termsUrl || ""}
        destinationPath={externalDownloadInfo?.destinationFolder || ""}
        files={externalDownloadInfo?.files || []}
        onAccept={async () => {
          if (externalDownloadModel) {
            await downloadModel(externalDownloadModel.id);
          }
        }}
        onOpenFolder={async () => {
          await commands.openAppDataDir();
        }}
        onClose={() => {
          setExternalDownloadModel(null);
          setExternalDownloadInfo(null);
        }}
      />

      <div className="glass-panel-subtle border border-[#3d3d3d] rounded-xl p-4">
        <p className="text-sm text-[#f5f5f5]">
          {t("modelSelector.customModelsHelpTitle")}
        </p>
        <p className="text-xs text-[#a0a0a0] mt-1">
          {t("modelSelector.customModelsHelpDescription")}
        </p>
        <p className="text-xs text-[#8a8a8a] mt-2">
          {customModelsCount > 0
            ? t("modelSelector.customModelsDetected", { count: customModelsCount })
            : t("modelSelector.customModelsHelpHint")}
        </p>
      </div>

      {error && (
        <div className="border border-[#5c2f35] rounded-xl bg-[#2a1618]/70 p-4 space-y-3">
          <div className="flex items-start gap-3">
            <ShieldAlert className="w-5 h-5 text-[#ff8ea1] mt-0.5 shrink-0" />
            <div className="min-w-0">
              <p className="text-sm font-medium text-[#ffd6dd]">
                {t("modelSelector.errorPanel.title")}
              </p>
              <p className="text-xs text-[#f1b7c1] mt-1 break-words">{error}</p>
            </div>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button variant="secondary" size="sm" onClick={() => commands.openLogDir()}>
              <FolderOpen className="w-3.5 h-3.5 mr-1.5" />
              {t("modelSelector.errorPanel.openLogs")}
            </Button>
            <Button variant="ghost" size="sm" onClick={() => commands.openAppDataDir()}>
              <FolderOpen className="w-3.5 h-3.5 mr-1.5" />
              {t("modelSelector.errorPanel.openAppData")}
            </Button>
          </div>
        </div>
      )}

      <SettingsGroup title={t("modelSelector.availableModels")}>
        {loading && (
          <div className="px-6 py-4 text-sm text-[#a0a0a0]">{t("common.loading")}</div>
        )}

        {!loading && downloadedModels.length === 0 && (
          <div className="px-6 py-4 text-sm text-[#a0a0a0]">
            {t("modelSelector.noModelsAvailable")}
          </div>
        )}

        {!loading &&
          downloadedModels.map((model) => {
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
                    <p className="text-sm font-medium text-[#f5f5f5]">{modelName}</p>
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
                      {isSwitching ? t("modelSelector.loadingGeneric") : t("modelSelector.chooseModel")}
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

        {downloadableModels.map((model) => {
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
                  {t("modelSelector.downloadSize")} · {formatModelSize(Number(model.size_mb))}
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
