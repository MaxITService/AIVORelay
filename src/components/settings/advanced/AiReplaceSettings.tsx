import React from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../../hooks/useSettings";
import { Input } from "../../ui/Input";
import { SettingContainer } from "../../ui/SettingContainer";
import { Textarea } from "../../ui/Textarea";

interface AiReplaceSettingsProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

export const AiReplaceSettings: React.FC<AiReplaceSettingsProps> = ({
  descriptionMode = "inline",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  const systemPrompt = getSetting("ai_replace_system_prompt") ?? "";
  const userPrompt = getSetting("ai_replace_user_prompt") ?? "";
  const maxChars = getSetting("ai_replace_max_chars") ?? 20000;

  const handleSystemPromptChange = (
    event: React.ChangeEvent<HTMLTextAreaElement>,
  ) => {
    updateSetting("ai_replace_system_prompt", event.target.value);
  };

  const handleUserPromptChange = (
    event: React.ChangeEvent<HTMLTextAreaElement>,
  ) => {
    updateSetting("ai_replace_user_prompt", event.target.value);
  };

  const handleMaxCharsChange = (
    event: React.ChangeEvent<HTMLInputElement>,
  ) => {
    const value = parseInt(event.target.value, 10);
    if (!isNaN(value) && value > 0) {
      updateSetting("ai_replace_max_chars", value);
    }
  };

  return (
    <>
      <SettingContainer
        title={t("settings.advanced.aiReplace.systemPrompt.title")}
        description={t("settings.advanced.aiReplace.systemPrompt.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
        layout="stacked"
      >
        <Textarea
          value={systemPrompt}
          onChange={handleSystemPromptChange}
          disabled={isUpdating("ai_replace_system_prompt")}
          className="w-full"
        />
      </SettingContainer>
      <SettingContainer
        title={t("settings.advanced.aiReplace.userPrompt.title")}
        description={t("settings.advanced.aiReplace.userPrompt.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
        layout="stacked"
      >
        <Textarea
          value={userPrompt}
          onChange={handleUserPromptChange}
          disabled={isUpdating("ai_replace_user_prompt")}
          className="w-full"
        />
      </SettingContainer>
      <SettingContainer
        title={t("settings.advanced.aiReplace.maxChars.title")}
        description={t("settings.advanced.aiReplace.maxChars.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
        layout="horizontal"
      >
        <div className="flex items-center space-x-2">
          <Input
            type="number"
            min="1"
            max="100000"
            value={maxChars}
            onChange={handleMaxCharsChange}
            disabled={isUpdating("ai_replace_max_chars")}
            className="w-24"
          />
          <span className="text-sm text-text">
            {t("settings.advanced.aiReplace.maxChars.suffix")}
          </span>
        </div>
      </SettingContainer>
    </>
  );
};
