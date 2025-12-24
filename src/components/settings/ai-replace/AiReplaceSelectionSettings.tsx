import React from "react";
import { useTranslation, Trans } from "react-i18next";
import { Info, Wand2 } from "lucide-react";
import { useSettings } from "../../../hooks/useSettings";
import { HandyShortcut } from "../HandyShortcut";
import { Input } from "../../ui/Input";
import { SettingContainer } from "../../ui/SettingContainer";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { Textarea } from "../../ui/Textarea";
import { ToggleSwitch } from "../../ui/ToggleSwitch";

export const AiReplaceSelectionSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, getSetting, updateSetting, isUpdating } = useSettings();

  const systemPrompt = getSetting("ai_replace_system_prompt") ?? "";
  const userPrompt = getSetting("ai_replace_user_prompt") ?? "";
  const maxChars = getSetting("ai_replace_max_chars") ?? 20000;
  const allowNoSelection = getSetting("ai_replace_allow_no_selection") ?? true;
  const noSelectionSystemPrompt = getSetting("ai_replace_no_selection_system_prompt") ?? "";

  const handleSystemPromptChange = (event: React.ChangeEvent<HTMLTextAreaElement>) => {
    void updateSetting("ai_replace_system_prompt", event.target.value);
  };

  const handleUserPromptChange = (event: React.ChangeEvent<HTMLTextAreaElement>) => {
    void updateSetting("ai_replace_user_prompt", event.target.value);
  };

  const handleMaxCharsChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const value = parseInt(event.target.value, 10);
    if (!isNaN(value) && value > 0) {
      void updateSetting("ai_replace_max_chars", value);
    }
  };

  const handleAllowNoSelectionChange = (checked: boolean) => {
    void updateSetting("ai_replace_allow_no_selection", checked);
  };

  const handleNoSelectionSystemPromptChange = (event: React.ChangeEvent<HTMLTextAreaElement>) => {
    void updateSetting("ai_replace_no_selection_system_prompt", event.target.value);
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      {/* Help Banner */}
      <div className="rounded-lg border border-purple-500/30 bg-purple-500/10 p-4">
        <div className="flex items-start gap-3">
          <Wand2 className="w-5 h-5 text-purple-400 mt-0.5 flex-shrink-0" />
          <div className="space-y-2 text-sm text-text/80">
            <p className="font-medium text-text">
              {t("settings.aiReplace.help.title")}
            </p>
            <p>{t("settings.aiReplace.help.description")}</p>
            <ul className="list-disc list-inside space-y-1 ml-1">
              <li>{t("settings.aiReplace.help.step1")}</li>
              <li>{t("settings.aiReplace.help.step2")}</li>
              <li>{t("settings.aiReplace.help.step3")}</li>
            </ul>
          </div>
        </div>
      </div>

      <SettingsGroup title={t("settings.aiReplace.shortcuts.title")}>
        <HandyShortcut shortcutId="ai_replace_selection" grouped={true} />
        <SettingContainer
          title={t("settings.general.shortcut.bindings.ai_replace_selection.pushToTalk.label")}
          description={t("settings.general.shortcut.bindings.ai_replace_selection.pushToTalk.description")}
          descriptionMode="tooltip"
          grouped={true}
        >
          <ToggleSwitch
            checked={settings?.ai_replace_selection_push_to_talk ?? true}
            onChange={(enabled) => void updateSetting("ai_replace_selection_push_to_talk", enabled)}
            disabled={isUpdating("ai_replace_selection_push_to_talk")}
          />
        </SettingContainer>
      </SettingsGroup>

      <SettingsGroup title={t("settings.aiReplace.noSelection.title")}>
        <div className="text-sm text-text/60 mb-2 px-1">
          {t("settings.aiReplace.noSelection.description")}
        </div>
        <ToggleSwitch
          label={t("settings.aiReplace.noSelection.allowToggle.label")}
          description={t("settings.aiReplace.noSelection.allowToggle.description")}
          descriptionMode="tooltip"
          grouped={true}
          checked={allowNoSelection}
          onChange={handleAllowNoSelectionChange}
          disabled={isUpdating("ai_replace_allow_no_selection")}
        />
        {allowNoSelection && (
          <SettingContainer
            title={t("settings.aiReplace.noSelection.systemPrompt.title")}
            description={t("settings.aiReplace.noSelection.systemPrompt.description")}
            descriptionMode="inline"
            grouped={true}
            layout="stacked"
          >
            <Textarea
              value={noSelectionSystemPrompt}
              onChange={handleNoSelectionSystemPromptChange}
              disabled={isUpdating("ai_replace_no_selection_system_prompt")}
              className="w-full"
              rows={4}
            />
          </SettingContainer>
        )}
      </SettingsGroup>

      <SettingsGroup title={t("settings.aiReplace.withSelection.title")}>
        <div className="text-sm text-text/60 mb-2 px-1">
          {t("settings.aiReplace.withSelection.description")}
        </div>
        <SettingContainer
          title={t("settings.aiReplace.withSelection.systemPrompt.title")}
          description={t("settings.aiReplace.withSelection.systemPrompt.description")}
          descriptionMode="inline"
          grouped={true}
          layout="stacked"
        >
          <Textarea
            value={systemPrompt}
            onChange={handleSystemPromptChange}
            disabled={isUpdating("ai_replace_system_prompt")}
            className="w-full"
            rows={5}
          />
        </SettingContainer>
        <SettingContainer
          title={t("settings.aiReplace.withSelection.userPrompt.title")}
          description={t("settings.aiReplace.withSelection.userPrompt.description")}
          descriptionMode="inline"
          grouped={true}
          layout="stacked"
        >
          <Textarea
            value={userPrompt}
            onChange={handleUserPromptChange}
            disabled={isUpdating("ai_replace_user_prompt")}
            className="w-full"
            rows={3}
          />
          <div className="text-xs text-text/50 mt-1">
            {t("settings.aiReplace.withSelection.variables")}
          </div>
        </SettingContainer>
        <SettingContainer
          title={t("settings.aiReplace.withSelection.maxChars.title")}
          description={t("settings.aiReplace.withSelection.maxChars.description")}
          descriptionMode="tooltip"
          grouped={true}
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
              {t("settings.aiReplace.withSelection.maxChars.suffix")}
            </span>
          </div>
        </SettingContainer>
      </SettingsGroup>
    </div>
  );
};
