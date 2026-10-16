import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { type as getOsType } from "@tauri-apps/plugin-os";
import { Dropdown } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";
import { useSettings } from "../../hooks/useSettings";
import { commands } from "@/bindings";
import type { TypingTool } from "@/bindings";

interface TypingToolProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

/** Linux-only choice of the tool used by the Direct paste method. */
export const TypingToolSetting: React.FC<TypingToolProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const isLinux = getOsType() === "linux";
    const [availableTools, setAvailableTools] = useState<string[] | null>(null);

    useEffect(() => {
      if (!isLinux) return;
      commands
        .getAvailableTypingTools()
        .then(setAvailableTools)
        .catch(() => setAvailableTools(["auto"]));
    }, [isLinux]);

    if (!isLinux) return null;

    const options = (availableTools ?? ["auto"]).map((tool) => ({
      value: tool,
      label:
        tool === "auto" ? t("settings.advanced.typingTool.options.auto") : tool,
    }));

    return (
      <SettingContainer
        title={t("settings.advanced.typingTool.title")}
        description={t("settings.advanced.typingTool.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
        tooltipPosition="bottom"
      >
        <Dropdown
          options={options}
          selectedValue={(getSetting("typing_tool") || "auto") as TypingTool}
          onSelect={(value) => updateSetting("typing_tool", value as TypingTool)}
          disabled={isUpdating("typing_tool")}
        />
      </SettingContainer>
    );
  },
);
