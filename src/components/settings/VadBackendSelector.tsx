import React from "react";
import { useTranslation } from "react-i18next";
import { Dropdown, type DropdownOption } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";
import { useSettings } from "../../hooks/useSettings";
import type { VadBackend } from "../../bindings";

interface VadBackendSelectorProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const VadBackendSelector: React.FC<VadBackendSelectorProps> = ({
  descriptionMode = "inline",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const { settings, updateSetting, isUpdating } = useSettings();
  const selectedBackend = settings?.vad_backend ?? "silero";

  const options: DropdownOption[] = [
    {
      value: "silero",
      label: t("audioProcessing.vadBackendSilero", "Silero"),
    },
    {
      value: "earshot",
      label: t("audioProcessing.vadBackendEarshot", "Earshot (Experimental)"),
    },
  ];

  const handleSelect = (value: string) => {
    if (value === selectedBackend) return;
    void updateSetting("vad_backend", value as VadBackend);
  };

  return (
    <SettingContainer
      title={t("audioProcessing.vadBackend", "Filter Silence engine")}
      description={t(
        "audioProcessing.vadBackendDescription",
        "Engine used by Filter Silence. Silero is the recommended default. Earshot is experimental; switch back to Silero if speech is clipped or background noise is retained. This setting has an effect only when Filter Silence is enabled.",
      )}
      descriptionMode={descriptionMode}
      grouped={grouped}
      layout="horizontal"
    >
      <Dropdown
        options={options}
        selectedValue={selectedBackend}
        onSelect={handleSelect}
        disabled={!settings || isUpdating("vad_backend")}
      />
    </SettingContainer>
  );
};

export default VadBackendSelector;
