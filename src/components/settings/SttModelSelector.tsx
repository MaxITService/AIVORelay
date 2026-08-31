import React, { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle } from "lucide-react";
import { useTranslation } from "react-i18next";
import { commands, type ModelInfo } from "@/bindings";
import { Button } from "@/components/ui/Button";
import { Dropdown } from "@/components/ui/Dropdown";
import { useNavigationStore } from "@/stores/navigationStore";
import {
  sttCatalog,
  sttModelDropdownOptions,
  sttProviderId,
  sttSelectionKey,
  type SttCatalogOption,
  type SttModelSelection,
  type SttWorkflow,
} from "@/lib/sttModelSelection";

type Readiness = {
  selection: SttModelSelection;
  ready: boolean;
  reason?: string | null;
};

type ReadinessProblemKind =
  | "missingApiKey"
  | "downloadOnUse"
  | "notConfigured";

type Props = {
  workflow: SttWorkflow;
  selection: SttModelSelection;
  localModels?: ModelInfo[];
  onChange: (selection: SttModelSelection) => void | Promise<void>;
  disabled?: boolean;
};

const EMPTY_LOCAL_MODELS: ModelInfo[] = [];

export const SttModelSelector: React.FC<Props> = ({
  workflow,
  selection,
  localModels = EMPTY_LOCAL_MODELS,
  onChange,
  disabled = false,
}) => {
  const { t } = useTranslation();
  const catalog = useMemo(
    () => sttCatalog(workflow, localModels),
    [localModels, workflow],
  );
  const currentKey = sttSelectionKey(selection);
  const currentProviderId = sttProviderId(selection);
  const [readiness, setReadiness] = useState<Map<string, string | null>>(
    new Map(),
  );
  const [problemKinds, setProblemKinds] = useState<
    Map<string, ReadinessProblemKind>
  >(new Map());
  const [readinessRefreshToken, setReadinessRefreshToken] = useState(0);
  const [readinessCheckFailed, setReadinessCheckFailed] = useState(false);
  const [downloadStarting, setDownloadStarting] = useState(false);
  const [selectionChanging, setSelectionChanging] = useState(false);
  const [downloadedModelIds, setDownloadedModelIds] = useState<Set<string>>(
    new Set(),
  );
  const [actionError, setActionError] = useState<string | null>(null);

  const options = useMemo<SttCatalogOption[]>(() => {
    if (catalog.some((option) => sttSelectionKey(option.selection) === currentKey)) {
      return catalog;
    }
    return [
      ...catalog,
      {
        id: `current:${currentKey}`,
        providerId: currentProviderId,
        providerLabel:
          currentProviderId.charAt(0).toUpperCase() + currentProviderId.slice(1),
        modelLabel: selection.model_id || "Current model",
        selection,
        capabilities: catalog[0]?.capabilities ?? {
          workflows: [workflow],
          diarization: [],
          languageHints: [],
          vocabulary: [],
          chunking: [],
          timestamps: [],
        },
      },
    ];
  }, [catalog, currentKey, currentProviderId]);

  useEffect(() => {
    let active = true;
    setReadinessCheckFailed(false);
    void invoke<Readiness[]>("stt_model_selections_readiness", {
      selections: options.map((option) => option.selection),
    })
      .then((items) => {
        if (!active) return;
        const next = new Map<string, string | null>();
        const nextKinds = new Map<string, ReadinessProblemKind>();
        for (const item of items) {
          const key = sttSelectionKey(item.selection);
          next.set(
            key,
            item.ready
              ? null
              : item.reason === "API key is not configured in Models."
                ? t("settings.sttModelSelector.missingApiKey")
                : item.reason || t("settings.sttModelSelector.notConfigured"),
          );
          if (!item.ready) {
            nextKinds.set(
              key,
              item.reason === "API key is not configured in Models."
                ? "missingApiKey"
                : "notConfigured",
            );
          }
        }
        for (const option of options) {
          if (
            option.selection.provider === "local" &&
            option.localModel &&
            !option.localModel.is_downloaded &&
            !downloadedModelIds.has(option.localModel.id)
          ) {
            next.set(
              sttSelectionKey(option.selection),
              t("settings.sttModelSelector.downloadLocalModel"),
            );
            nextKinds.set(
              sttSelectionKey(option.selection),
              "downloadOnUse",
            );
          }
        }
        setReadiness(next);
        setProblemKinds(nextKinds);
      })
      .catch(() => {
        if (active) {
          setReadiness(new Map());
          setProblemKinds(new Map());
          setReadinessCheckFailed(true);
        }
      });
    return () => {
      active = false;
    };
  }, [downloadedModelIds, options, readinessRefreshToken, t]);

  const providerOptions = useMemo(() => {
    const providers = new Map<string, string>();
    for (const option of options) {
      providers.set(option.providerId, option.providerLabel);
    }
    return [...providers].map(([value, label]) => ({ value, label }));
  }, [options]);
  const modelOptions = options.filter(
    (option) => option.providerId === currentProviderId,
  );
  const currentCatalogOption = catalog.find(
    (option) => sttSelectionKey(option.selection) === currentKey,
  );
  const currentProblemKind = problemKinds.get(currentKey) ?? null;
  const currentProblem = !currentCatalogOption
    ? t("settings.sttModelSelector.incompatibleModel")
    : readinessCheckFailed
      ? t("settings.sttModelSelector.readinessCheckFailed")
      : readiness.get(currentKey) ?? null;

  const retryReadiness = () => {
    setActionError(null);
    setReadinessRefreshToken((value) => value + 1);
  };

  const downloadCurrentModel = async () => {
    const modelId = currentCatalogOption?.localModel?.id;
    if (!modelId || downloadStarting) return;
    setDownloadStarting(true);
    setActionError(null);
    try {
      const result = await commands.downloadModel(modelId);
      if (result.status === "error") {
        setActionError(result.error);
      } else {
        setDownloadedModelIds((previous) => new Set(previous).add(modelId));
      }
    } catch (error) {
      setActionError(error instanceof Error ? error.message : String(error));
    } finally {
      setDownloadStarting(false);
      setReadinessRefreshToken((value) => value + 1);
    }
  };

  const selectModel = async (nextSelection: SttModelSelection) => {
    if (selectionChanging) return;
    setSelectionChanging(true);
    setActionError(null);
    try {
      await onChange(nextSelection);
    } catch (error) {
      setActionError(error instanceof Error ? error.message : String(error));
    } finally {
      setSelectionChanging(false);
    }
  };

  const selectProvider = (providerId: string) => {
    const first = options.find((option) => option.providerId === providerId);
    if (first) void selectModel(first.selection);
  };

  return (
    <div className="grid gap-3 md:grid-cols-2">
      {catalog.length === 0 && (
        <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-amber-500/25 bg-amber-500/10 p-3 md:col-span-2">
          <div>
            <p className="text-sm font-medium text-amber-100">
              {t("settings.sttModelSelector.noCompatibleModelsTitle")}
            </p>
            <p className="mt-1 text-xs text-amber-200/70">
              {t("settings.sttModelSelector.noCompatibleModelsHint")}
            </p>
          </div>
          <Button
            type="button"
            variant="secondary"
            size="sm"
            onClick={() => useNavigationStore.getState().setSection("models")}
          >
            {t("settings.sttModelSelector.openModels")}
          </Button>
        </div>
      )}
      <div className="space-y-1.5">
        <p className="text-[11px] uppercase tracking-[0.18em] text-[#8a8a8a]">
          {t("settings.sttModelSelector.provider")}
        </p>
        <Dropdown
          className="w-full"
          ariaLabel={t("settings.sttModelSelector.provider")}
          selectedValue={currentProviderId}
          options={providerOptions}
          onSelect={selectProvider}
          disabled={disabled || selectionChanging}
          dropUp={false}
        />
      </div>
      <div className="space-y-1.5" title={currentProblem || undefined}>
        <p
          className={`text-[11px] uppercase tracking-[0.18em] ${
            currentProblem ? "text-red-400" : "text-[#8a8a8a]"
          }`}
        >
          {t("settings.sttModelSelector.model")}
        </p>
        <Dropdown
          className="w-full"
          ariaLabel={t("settings.sttModelSelector.model")}
          selectedValue={currentKey}
          options={sttModelDropdownOptions(modelOptions, readiness)}
          onSelect={(value) => {
            const option = modelOptions.find(
              (item) => sttSelectionKey(item.selection) === value,
            );
            if (option) void selectModel(option.selection);
          }}
          disabled={disabled || selectionChanging}
          dropUp={false}
        />
        {currentProblem && (
          <div
            role="alert"
            className="space-y-2 rounded-lg border border-red-500/25 bg-red-500/5 p-2.5"
          >
            <p className="flex items-start gap-1.5 text-xs text-red-400">
              <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
              <span>{currentProblem}</span>
            </p>
            <div className="flex flex-wrap items-center gap-2 pl-5">
              {!currentCatalogOption ? (
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  disabled={disabled || catalog.length === 0}
                  onClick={() => {
                    const compatible = catalog[0];
                    if (compatible) void selectModel(compatible.selection);
                  }}
                >
                  {t("settings.sttModelSelector.selectCompatibleModel")}
                </Button>
              ) : currentProblemKind === "missingApiKey" ? (
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  onClick={() => useNavigationStore.getState().setSection("models")}
                >
                  {t("settings.sttModelSelector.configureApiKey")}
                </Button>
              ) : currentProblemKind === "downloadOnUse" ? (
                <>
                  <span className="text-xs text-amber-300">
                    {t("settings.sttModelSelector.downloadWhenStarted")}
                  </span>
                  <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    disabled={disabled || downloadStarting}
                    onClick={() => void downloadCurrentModel()}
                  >
                    {downloadStarting
                      ? t("settings.sttModelSelector.downloading")
                      : t("settings.sttModelSelector.downloadNow")}
                  </Button>
                </>
              ) : (
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  disabled={disabled}
                  onClick={retryReadiness}
                >
                  {t("common.retry", "Retry")}
                </Button>
              )}
            </div>
          </div>
        )}
        {actionError && (
          <p role="alert" className="text-xs text-red-300">
            {actionError}
          </p>
        )}
      </div>
    </div>
  );
};
