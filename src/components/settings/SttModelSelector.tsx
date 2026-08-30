import React, { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ModelInfo } from "@/bindings";
import { Dropdown } from "@/components/ui/Dropdown";
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
    void invoke<Readiness[]>("stt_model_selections_readiness", {
      selections: options.map((option) => option.selection),
    })
      .then((items) => {
        if (!active) return;
        const next = new Map<string, string | null>();
        for (const item of items) {
          next.set(
            sttSelectionKey(item.selection),
            item.ready
              ? null
              : item.reason === "API key is not configured in Models."
                ? t("settings.sttModelSelector.missingApiKey")
                : item.reason || t("settings.sttModelSelector.notConfigured"),
          );
        }
        for (const option of options) {
          if (
            option.selection.provider === "local" &&
            option.localModel &&
            !option.localModel.is_downloaded
          ) {
            next.set(
              sttSelectionKey(option.selection),
              t("settings.sttModelSelector.downloadLocalModel"),
            );
          }
        }
        setReadiness(next);
      })
      .catch(() => {
        if (active) setReadiness(new Map());
      });
    return () => {
      active = false;
    };
  }, [options, t]);

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
  const currentProblem = readiness.get(currentKey) ?? null;

  const selectProvider = (providerId: string) => {
    const first = options.find((option) => option.providerId === providerId);
    if (first) void onChange(first.selection);
  };

  return (
    <div className="grid gap-3 md:grid-cols-2">
      <div className="space-y-1.5">
        <p className="text-[11px] uppercase tracking-[0.18em] text-[#8a8a8a]">
          {t("settings.sttModelSelector.provider")}
        </p>
        <Dropdown
          className="w-full"
          selectedValue={currentProviderId}
          options={providerOptions}
          onSelect={selectProvider}
          disabled={disabled}
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
          selectedValue={currentKey}
          options={sttModelDropdownOptions(modelOptions, readiness)}
          onSelect={(value) => {
            const option = modelOptions.find(
              (item) => sttSelectionKey(item.selection) === value,
            );
            if (option) void onChange(option.selection);
          }}
          disabled={disabled}
          dropUp={false}
        />
        {currentProblem && (
          <p className="flex items-start gap-1.5 text-xs text-red-400">
            <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
            <span>{currentProblem}</span>
          </p>
        )}
      </div>
    </div>
  );
};
