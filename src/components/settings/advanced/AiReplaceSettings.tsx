import React from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../../hooks/useSettings";
import { Input } from "../../ui/Input";
import { SettingContainer } from "../../ui/SettingContainer";
import { Textarea } from "../../ui/Textarea";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { TellMeMore } from "../../ui/TellMeMore";

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
  const allowNoSelection = getSetting("ai_replace_allow_no_selection") ?? true;
  const noSelectionSystemPrompt =
    getSetting("ai_replace_no_selection_system_prompt") ?? "";

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

  const handleAllowNoSelectionChange = (checked: boolean) => {
    updateSetting("ai_replace_allow_no_selection", checked);
  };

  const handleNoSelectionSystemPromptChange = (
    event: React.ChangeEvent<HTMLTextAreaElement>,
  ) => {
    updateSetting("ai_replace_no_selection_system_prompt", event.target.value);
  };

  return (
    <>
      <TellMeMore title={t("settings.advanced.aiReplace.tellMeMore.title", "Tell me more: How to use AI Replace")}>
        <div className="space-y-3">
          <p>
            <strong>Think of this as your magic editing wand.</strong>
          </p>
          <ol className="list-decimal list-inside space-y-1 ml-1 opacity-90">
            <li><strong>Select Text:</strong> Highlight any text in any app (Word, Email, Browser).</li>
            <li><strong>Trigger:</strong> Press your AI Replace shortcut (Default: <code>Ctrl+Shift+Insert</code>).</li>
            <li><strong>Speak:</strong> Tell the AI what to change.
              <ul className="list-disc list-inside ml-5 mt-1 text-text/80 text-xs">
                <li><em>"Fix the grammar"</em></li>
                <li><em>"Make this sound more professional"</em></li>
                <li><em>"Translate to French"</em></li>
              </ul>
            </li>
            <li><strong>Watch:</strong> The text disappears and is re-typed with the improvements!</li>
          </ol>
          
          <div className="mt-4 p-3 bg-red-500/10 border border-red-500/20 rounded-md">
            <p className="font-semibold text-red-300 mb-1">⚠️ Configuration Required</p>
            <p className="text-xs">
              This feature requires an active <strong>LLM API</strong> connection to process instructions. Local speech models only handle speech-to-text conversion.<br/><br/>
              Please configure an API Key (OpenAI, Groq, Anthropic) in the <span className="font-bold text-accent">LLM API Relay</span> settings.
            </p>
          </div>

          <p className="pt-2">
            <strong>💡 Pro Tip: Generating New Text</strong><br/>
            If you <strong>don't</strong> select any text, you can just ask the AI to write something from scratch (e.g., <em>"Write a friendly out-of-office email"</em>).
          </p>
        </div>
      </TellMeMore>

      <ToggleSwitch
        label={t("settings.advanced.aiReplace.allowNoSelection.label")}
        description={t(
          "settings.advanced.aiReplace.allowNoSelection.description",
        )}
        descriptionMode={descriptionMode}
        grouped={grouped}
        checked={allowNoSelection}
        onChange={handleAllowNoSelectionChange}
        disabled={isUpdating("ai_replace_allow_no_selection")}
      />

      <ToggleSwitch
        label={t("settings.advanced.aiReplace.allowQuickTap.label")}
        description={t("settings.advanced.aiReplace.allowQuickTap.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
        checked={getSetting("ai_replace_allow_quick_tap") ?? true}
        onChange={(checked) => updateSetting("ai_replace_allow_quick_tap", checked)}
        disabled={isUpdating("ai_replace_allow_quick_tap")}
      />
      
      {getSetting("ai_replace_allow_quick_tap") && (
        <SettingContainer
          title={t("settings.advanced.aiReplace.quickTapSystemPrompt.title")}
          description={t("settings.advanced.aiReplace.quickTapSystemPrompt.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
          layout="stacked"
        >
          <Textarea
            value={getSetting("ai_replace_quick_tap_system_prompt") ?? ""}
            onChange={(e) => updateSetting("ai_replace_quick_tap_system_prompt", e.target.value)}
            disabled={isUpdating("ai_replace_quick_tap_system_prompt")}
            className="w-full"
            rows={3}
          />
          <div className="mt-4 flex items-center space-x-2">
            <div className="flex-1">
              <div className="text-sm font-medium text-text">
                {t("settings.advanced.aiReplace.quickTapThreshold.title")}
              </div>
              <div className="text-xs text-text-muted">
                {t("settings.advanced.aiReplace.quickTapThreshold.description")}
              </div>
            </div>
            <Input
              type="number"
              min="100"
              max="2000"
              step="50"
              value={getSetting("ai_replace_quick_tap_threshold_ms") ?? 500}
              onChange={(e) => {
                const val = parseInt(e.target.value, 10);
                if (!isNaN(val) && val > 0) {
                  updateSetting("ai_replace_quick_tap_threshold_ms", val);
                }
              }}
              disabled={isUpdating("ai_replace_quick_tap_threshold_ms")}
              className="w-24"
            />
            <span className="text-sm text-text">
              {t("settings.advanced.aiReplace.quickTapThreshold.suffix")}
            </span>
          </div>
        </SettingContainer>
      )}

      {allowNoSelection && (
        <SettingContainer
          title={t("settings.advanced.aiReplace.noSelectionSystemPrompt.title")}
          description={t(
            "settings.advanced.aiReplace.noSelectionSystemPrompt.description",
          )}
          descriptionMode={descriptionMode}
          grouped={grouped}
          layout="stacked"
        >
          <Textarea
            value={noSelectionSystemPrompt}
            onChange={handleNoSelectionSystemPromptChange}
            disabled={isUpdating("ai_replace_no_selection_system_prompt")}
            className="w-full"
          />
        </SettingContainer>
      )}

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
