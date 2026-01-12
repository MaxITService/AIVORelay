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
import { TellMeMore } from "../../ui/TellMeMore";

import { useNavigationStore } from "../../../stores/navigationStore";

export const AiReplaceSelectionSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, getSetting, updateSetting, isUpdating } = useSettings();
  const { setSection } = useNavigationStore();

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
    <div className="max-w-3xl w-full mx-auto space-y-8 pb-12">
      {/* Help Banner */}
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
              Please configure an API Key (OpenAI, Groq, Anthropic) in the <button onClick={() => setSection("postprocessing")} className="font-bold text-accent hover:underline cursor-pointer">LLM API Relay</button> settings.
            </p>
          </div>

          <p className="pt-2">
            <strong>💡 Pro Tip: Generating New Text</strong><br/>
            If you <strong>don't</strong> select any text, you can just ask the AI to write something from scratch (e.g., <em>"Write a friendly out-of-office email"</em>).
          </p>
        </div>
      </TellMeMore>

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

      <SettingsGroup 
        title={t("settings.aiReplace.noSelection.title")}
        description={t("settings.aiReplace.noSelection.description")}
      >
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

      <SettingsGroup 
        title={t("settings.aiReplace.quickTap.title")}
        description={t("settings.aiReplace.quickTap.description")}
      >
        <ToggleSwitch
          label={t("settings.aiReplace.quickTap.allowQuickTap.label")}
          description={t("settings.aiReplace.quickTap.allowQuickTap.description")}
          descriptionMode="tooltip"
          grouped={true}
          checked={getSetting("ai_replace_allow_quick_tap") ?? true}
          onChange={(checked) => void updateSetting("ai_replace_allow_quick_tap", checked)}
          disabled={isUpdating("ai_replace_allow_quick_tap")}
        />
        {(getSetting("ai_replace_allow_quick_tap") ?? true) && (
          <>
            <SettingContainer
              title={t("settings.aiReplace.quickTap.systemPrompt.title")}
              description={t("settings.aiReplace.quickTap.systemPrompt.description")}
              descriptionMode="inline"
              grouped={true}
              layout="stacked"
            >
              <Textarea
                value={getSetting("ai_replace_quick_tap_system_prompt") ?? ""}
                onChange={(e) => void updateSetting("ai_replace_quick_tap_system_prompt", e.target.value)}
                disabled={isUpdating("ai_replace_quick_tap_system_prompt")}
                className="w-full"
                rows={4}
              />
            </SettingContainer>
            <SettingContainer
              title={t("settings.aiReplace.quickTap.threshold.title")}
              description={t("settings.aiReplace.quickTap.threshold.description")}
              descriptionMode="tooltip"
              grouped={true}
              layout="horizontal"
            >
              <div className="flex items-center space-x-2">
                <Input
                  type="number"
                  min="100"
                  max="2000"
                  step="50"
                  value={getSetting("ai_replace_quick_tap_threshold_ms") ?? 500}
                  onChange={(e) => {
                    const val = parseInt(e.target.value, 10);
                    if (!isNaN(val) && val > 0) {
                      void updateSetting("ai_replace_quick_tap_threshold_ms", val);
                    }
                  }}
                  disabled={isUpdating("ai_replace_quick_tap_threshold_ms")}
                  className="w-24"
                />
                <span className="text-sm text-text">
                  {t("settings.aiReplace.quickTap.threshold.suffix")}
                </span>
              </div>
            </SettingContainer>
          </>
        )}
      </SettingsGroup>

      <SettingsGroup 
        title={t("settings.aiReplace.withSelection.title")}
        description={t("settings.aiReplace.withSelection.description")}
      >
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
