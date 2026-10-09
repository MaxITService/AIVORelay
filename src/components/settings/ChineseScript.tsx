import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { Dropdown } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";
import { useSettings } from "../../hooks/useSettings";
import type { ChineseScript } from "../../stores/settingsStore";
import { sessionToast as toast } from "@/lib/sessionToast";

interface ChineseScriptProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  profileId?: string;
}

export const ChineseScriptSetting: React.FC<ChineseScriptProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false, profileId }) => {
    const { t } = useTranslation();
    const { settings, updateSetting, isUpdating, refreshSettings } = useSettings();
    const [saving, setSaving] = useState(false);
    const profile = settings?.transcription_profiles?.find((item) => item.id === profileId) as
      | { chinese_script?: ChineseScript | null }
      | undefined;
    const options = [
      ...(profileId ? [{ value: "inherit", label: t("settings.advanced.chineseScript.options.inherit", "Use global setting") }] : []),
      { value: "as_transcribed", label: t("settings.advanced.chineseScript.options.asTranscribed") },
      { value: "simplified", label: t("settings.advanced.chineseScript.options.simplified") },
      { value: "traditional", label: t("settings.advanced.chineseScript.options.traditional") },
    ];
    const selectedScript = profileId ? profile?.chinese_script ?? "inherit" : settings?.chinese_script ?? "as_transcribed";
    const save = async (value: string) => {
      if (!profileId) {
        await updateSetting("chinese_script", value as ChineseScript);
        return;
      }
      setSaving(true);
      try {
        await invoke("change_chinese_script_setting", {
          script: value === "inherit" ? null : value,
          profileId,
        });
        await refreshSettings();
      } catch (error) {
        toast.error(t("settings.advanced.chineseScript.saveFailed", "Could not save Chinese output script."), {
          description: String(error),
        });
      } finally {
        setSaving(false);
      }
    };
    return (
      <SettingContainer
        title={t("settings.advanced.chineseScript.title")}
        description={t("settings.advanced.chineseScript.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      >
        <Dropdown options={options} selectedValue={selectedScript}
          onSelect={(value) => void save(value)}
          disabled={saving || isUpdating("chinese_script")} />
      </SettingContainer>
    );
  },
);
