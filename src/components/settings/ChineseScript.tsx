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

export interface ChineseScriptFieldsProps {
  scopeKey: string;
  value: ChineseScript | null;
  allowInherit: boolean;
  disabled?: boolean;
  onChange: (value: ChineseScript | null) => void | Promise<void>;
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const ChineseScriptFields: React.FC<ChineseScriptFieldsProps> = ({
  scopeKey, value, allowInherit, disabled = false, onChange,
  descriptionMode = "tooltip", grouped = false,
}) => {
  const { t } = useTranslation();
  const options = [
    ...(allowInherit ? [{ value: "inherit", label: t("settings.advanced.chineseScript.options.inherit", "Use global setting") }] : []),
    { value: "as_transcribed", label: t("settings.advanced.chineseScript.options.asTranscribed") },
    { value: "simplified", label: t("settings.advanced.chineseScript.options.simplified") },
    { value: "traditional", label: t("settings.advanced.chineseScript.options.traditionalAutomatic", "Traditional (based on spoken language)") },
    { value: "traditional_tw", label: t("settings.advanced.chineseScript.options.traditionalTaiwan", "Traditional (Taiwan)") },
    { value: "traditional_hk", label: t("settings.advanced.chineseScript.options.traditionalHongKong", "Traditional (Hong Kong)") },
  ];
  const change = async (nextValue: string) => {
    if (disabled) return;
    try {
      await onChange(nextValue === "inherit" ? null : nextValue as ChineseScript);
    } catch (error) {
      toast.error(t("settings.advanced.chineseScript.saveFailed", "Could not save Chinese output script."), {
        description: String(error),
      });
    }
  };
  return (
    <SettingContainer
      id={`settings-chinese-script-${scopeKey}`}
      title={t("settings.advanced.chineseScript.title")}
      description={t("settings.advanced.chineseScript.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
    >
      <Dropdown options={options} selectedValue={value ?? (allowInherit ? "inherit" : "as_transcribed")}
        onSelect={(nextValue) => void change(nextValue)}
        disabled={disabled} />
    </SettingContainer>
  );
};

export const ChineseScriptSetting: React.FC<ChineseScriptProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false, profileId }) => {
    const { t } = useTranslation();
    const { settings, updateSetting, isUpdating, refreshSettings } = useSettings();
    const [saving, setSaving] = useState(false);
    const profile = settings?.transcription_profiles?.find((item) => item.id === profileId) as
      | { chinese_script?: ChineseScript | null }
      | undefined;
    const selectedScript = profileId ? profile?.chinese_script ?? null : settings?.chinese_script ?? "as_transcribed";
    const save = async (value: ChineseScript | null) => {
      if (saving || isUpdating("chinese_script")) return;
      if (!profileId) {
        // The checked-in generated bindings are refreshed by the debug app.
        await updateSetting("chinese_script", (value ?? "as_transcribed") as NonNullable<typeof settings>["chinese_script"]);
        return;
      }
      setSaving(true);
      try {
        await invoke("change_chinese_script_setting", {
          script: value,
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
      <ChineseScriptFields
        scopeKey={profileId ?? "default"}
        value={selectedScript}
        allowInherit={!!profileId}
        descriptionMode={descriptionMode}
        grouped={grouped}
        onChange={save}
        disabled={saving || isUpdating("chinese_script")}
      />
    );
  },
);
