import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { type as getOsType } from "@tauri-apps/plugin-os";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";
import type { PasteMethod } from "@/bindings";

interface ConvertLfToCrlfSettingProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const ConvertLfToCrlfSetting: React.FC<ConvertLfToCrlfSettingProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const [osType, setOsType] = useState<string>("unknown");

    useEffect(() => {
      setOsType(getOsType());
    }, []);

    const selectedMethod = (getSetting("paste_method") || "ctrl_v") as PasteMethod;
    const convertLfToCrlf = (getSetting("convert_lf_to_crlf" as any) ?? true) as boolean;
    const isClipboardMethod =
      selectedMethod === "ctrl_v" ||
      selectedMethod === "ctrl_shift_v" ||
      selectedMethod === "shift_insert";

    if (osType !== "windows" || !isClipboardMethod) {
      return null;
    }

    return (
      <ToggleSwitch
        checked={convertLfToCrlf}
        onChange={(enabled) => updateSetting("convert_lf_to_crlf" as any, enabled)}
        isUpdating={isUpdating("convert_lf_to_crlf")}
        label={t("settings.advanced.pasteMethod.convertLfToCrlf.label")}
        description={t("settings.advanced.pasteMethod.convertLfToCrlf.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  });
