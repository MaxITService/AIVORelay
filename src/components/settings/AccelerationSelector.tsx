import { type FC, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { Dropdown, type DropdownOption } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";

type WhisperAcceleratorSetting = "auto" | "cpu" | "gpu";
type OrtAcceleratorSetting = "auto" | "cpu" | "cuda" | "directml" | "rocm";

interface AvailableAccelerators {
  whisper: string[];
  ort: string[];
}

const WHISPER_LABELS: Record<WhisperAcceleratorSetting, string> = {
  auto: "Auto",
  cpu: "CPU",
  gpu: "GPU",
};

const ORT_LABELS: Record<OrtAcceleratorSetting, string> = {
  auto: "Auto",
  cpu: "CPU",
  cuda: "CUDA",
  directml: "DirectML",
  rocm: "ROCm",
};

interface AccelerationSelectorProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

export const AccelerationSelector: FC<AccelerationSelectorProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const [whisperOptions, setWhisperOptions] = useState<DropdownOption[]>([]);
  const [ortOptions, setOrtOptions] = useState<DropdownOption[]>([]);

  useEffect(() => {
    void invoke<AvailableAccelerators>("get_available_accelerators").then(
      (available) => {
        setWhisperOptions(
          available.whisper.map((value) => ({
            value,
            label: WHISPER_LABELS[value as WhisperAcceleratorSetting] ?? value,
          })),
        );

        const ortValues = available.ort.includes("auto")
          ? available.ort
          : ["auto", ...available.ort];
        setOrtOptions(
          ortValues.map((value) => ({
            value,
            label: ORT_LABELS[value as OrtAcceleratorSetting] ?? value,
          })),
        );
      },
    );
  }, []);

  const currentWhisper =
    (getSetting("whisper_accelerator" as any) as WhisperAcceleratorSetting) ??
    "auto";
  const currentOrt =
    (getSetting("ort_accelerator" as any) as OrtAcceleratorSetting) ?? "auto";

  return (
    <>
      <SettingContainer
        title={t("settings.advanced.acceleration.whisper.title")}
        description={t("settings.advanced.acceleration.whisper.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
        layout="horizontal"
      >
        <Dropdown
          options={whisperOptions}
          selectedValue={currentWhisper}
          onSelect={(value) =>
            void updateSetting(
              "whisper_accelerator" as any,
              value as WhisperAcceleratorSetting as any,
            )
          }
          disabled={isUpdating("whisper_accelerator")}
        />
      </SettingContainer>
      {ortOptions.length > 2 && (
        <SettingContainer
          title={t("settings.advanced.acceleration.ort.title")}
          description={t("settings.advanced.acceleration.ort.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
          layout="horizontal"
        >
          <Dropdown
            options={ortOptions}
            selectedValue={currentOrt}
            onSelect={(value) =>
              void updateSetting(
                "ort_accelerator" as any,
                value as OrtAcceleratorSetting as any,
              )
            }
            disabled={isUpdating("ort_accelerator")}
          />
        </SettingContainer>
      )}
    </>
  );
};
