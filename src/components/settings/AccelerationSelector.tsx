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
  gpu_devices: GpuDeviceOption[];
}

interface GpuDeviceOption {
  id: number;
  name: string;
  total_vram_mb: number;
}

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

function encodeWhisperValue(
  accelerator: WhisperAcceleratorSetting,
  gpuDevice: number,
): string {
  if (accelerator === "cpu") return "cpu";
  if (accelerator === "gpu" && gpuDevice >= 0) return `gpu:${gpuDevice}`;
  return "auto";
}

function decodeWhisperValue(value: string): {
  accelerator: WhisperAcceleratorSetting;
  gpuDevice: number;
} {
  if (value === "cpu") return { accelerator: "cpu", gpuDevice: -1 };
  if (value.startsWith("gpu:")) {
    return {
      accelerator: "gpu",
      gpuDevice: Number.parseInt(value.slice(4), 10),
    };
  }
  return { accelerator: "auto", gpuDevice: -1 };
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
        const nextWhisperOptions: DropdownOption[] = [
          {
            value: "auto",
            label: t("settings.advanced.acceleration.gpuDevice.auto"),
          },
        ];

        for (const device of available.gpu_devices) {
          const vramLabel =
            device.total_vram_mb >= 1024
              ? `${(device.total_vram_mb / 1024).toFixed(1)} GB`
              : `${device.total_vram_mb} MB`;
          nextWhisperOptions.push({
            value: `gpu:${device.id}`,
            label: `${device.name} (${vramLabel})`,
          });
        }

        nextWhisperOptions.push({
          value: "cpu",
          label: "CPU",
        });
        setWhisperOptions(nextWhisperOptions);

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
  }, [t]);

  const currentWhisper = encodeWhisperValue(
    ((getSetting("whisper_accelerator" as any) as WhisperAcceleratorSetting) ??
      "auto") as WhisperAcceleratorSetting,
    ((getSetting("whisper_gpu_device" as any) as number) ?? -1) as number,
  );
  const currentOrt =
    (getSetting("ort_accelerator" as any) as OrtAcceleratorSetting) ?? "auto";

  const handleWhisperChange = async (value: string) => {
    const { accelerator, gpuDevice } = decodeWhisperValue(value);
    await updateSetting("whisper_accelerator" as any, accelerator as any);
    await updateSetting("whisper_gpu_device" as any, gpuDevice as any);
  };

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
          onSelect={(value) => void handleWhisperChange(value)}
          disabled={
            isUpdating("whisper_accelerator") ||
            isUpdating("whisper_gpu_device")
          }
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
