import React, { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  commands,
  type ModelInfo,
  type OrtAcceleratorSetting,
  type WhisperAcceleratorSetting,
} from "@/bindings";
import { useSettings } from "@/hooks/useSettings";

const WHISPER_ENGINE_TYPES = new Set<ModelInfo["engine_type"]>(["Whisper"]);

const FooterCpuWarning: React.FC = () => {
  const { t } = useTranslation();
  const { settings } = useSettings();
  const [models, setModels] = useState<ModelInfo[]>([]);

  const transcriptionProvider = settings?.transcription_provider ?? "local";
  const selectedModelId = settings?.selected_model ?? "";
  const whisperAccelerator =
    (settings?.whisper_accelerator as WhisperAcceleratorSetting | undefined) ??
    "auto";
  const ortAccelerator =
    (settings?.ort_accelerator as OrtAcceleratorSetting | undefined) ?? "auto";

  useEffect(() => {
    const loadModels = async () => {
      try {
        const result = await commands.getAvailableModels();
        if (result.status === "ok") {
          setModels(result.data);
        }
      } catch (error) {
        console.error("Failed to load models for footer CPU warning:", error);
      }
    };

    void loadModels();
  }, [selectedModelId]);

  const selectedModel = useMemo(
    () => models.find((model) => model.id === selectedModelId) ?? null,
    [models, selectedModelId],
  );

  const cpuBackends = useMemo(() => {
    const backends: string[] = [];

    if (whisperAccelerator === "cpu") {
      backends.push(
        t("footer.cpuWarning.whisperBackend", {
          defaultValue: "Whisper backend",
        }),
      );
    }

    if (ortAccelerator === "cpu") {
      backends.push(
        t("footer.cpuWarning.onnxBackend", {
          defaultValue: "ONNX backend",
        }),
      );
    }

    return backends;
  }, [ortAccelerator, t, whisperAccelerator]);

  const selectedModelBackendLabel = useMemo(() => {
    if (!selectedModel) {
      return null;
    }

    return WHISPER_ENGINE_TYPES.has(selectedModel.engine_type)
      ? t("footer.cpuWarning.whisperBackend", {
          defaultValue: "Whisper backend",
        })
      : t("footer.cpuWarning.onnxBackend", {
          defaultValue: "ONNX backend",
        });
  }, [selectedModel, t]);

  if (transcriptionProvider !== "local" || cpuBackends.length === 0) {
    return null;
  }

  const tooltipText = [
    t("footer.cpuWarning.title", {
      defaultValue: "Local CPU acceleration warning",
    }),
    cpuBackends.length === 1
      ? t("footer.cpuWarning.singleBackend", {
          defaultValue:
            "{{backend}} is set to CPU only. Local transcription may be slower until you switch acceleration in Advanced > Acceleration.",
          backend: cpuBackends[0],
        })
      : t("footer.cpuWarning.multipleBackends", {
          defaultValue:
            "{{backends}} are set to CPU only. Local transcription may be slower until you switch acceleration in Advanced > Acceleration.",
          backends: cpuBackends.join(" + "),
        }),
    selectedModelBackendLabel
      ? t("footer.cpuWarning.selectedModel", {
          defaultValue: "Current local model uses: {{backend}}.",
          backend: selectedModelBackendLabel,
        })
      : t("footer.cpuWarning.noModel", {
          defaultValue: "No local model is currently selected.",
        }),
  ].join("\n");

  return (
    <span
      className="text-[11px] font-semibold uppercase tracking-[0.18em] text-red-400 transition-colors hover:text-red-300"
      title={tooltipText}
    >
      {t("footer.cpuWarning.label", { defaultValue: "CPU" })}
    </span>
  );
};

export default FooterCpuWarning;
