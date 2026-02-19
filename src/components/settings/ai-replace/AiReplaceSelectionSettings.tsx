import React from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown, RotateCcw } from "lucide-react";
import { useSettings } from "../../../hooks/useSettings";
import { HandyShortcut } from "../HandyShortcut";
import { Input } from "../../ui/Input";
import { SettingContainer } from "../../ui/SettingContainer";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { Textarea } from "../../ui/Textarea";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { TellMeMore } from "../../ui/TellMeMore";
import { LlmConfigSection } from "../PostProcessingSettingsApi/LlmConfigSection";
import { useAiReplaceProviderState } from "../post-processing/useAiReplaceProviderState";
import type { AppSettings } from "../../../bindings";

const AI_REPLACE_QUICK_TAP_THRESHOLD_MIN = 100;
const AI_REPLACE_QUICK_TAP_THRESHOLD_MAX = 2000;

const Var: React.FC<{ name: string; desc: string; dim?: boolean }> = ({ name, desc, dim }) => (
  <div className="flex gap-3 items-baseline">
    <code className="text-[#ff9dbc] font-mono text-xs shrink-0 min-w-[200px]">{name}</code>
    <span className={`text-xs ${dim ? "text-[#606060]" : "text-[#a0a0a0]"}`}>{desc}</span>
  </div>
);

const ExamplePrompt: React.FC<{ label: string; children: string }> = ({ label, children }) => (
  <div className="mt-3">
    <div className="text-xs text-[#808080] mb-1">{label}</div>
    <code className="block bg-[#1a1a1a] border border-[#333] rounded px-3 py-2 text-xs text-[#c8c8c8] whitespace-pre-wrap font-mono leading-relaxed">
      {children}
    </code>
  </div>
);

/* ── Reusable prompt editor: reset button + textarea + TellMeMore ── */

interface PromptEditorProps {
  settingKey: keyof AppSettings;
  rows?: number;
}

const PromptEditor: React.FC<PromptEditorProps> = ({ settingKey, rows = 3 }) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, resetSetting, isUpdating } = useSettings();
  const value = (getSetting(settingKey) as string) ?? "";
  const busy = isUpdating(settingKey);

  return (
    <div className="relative">
      <Textarea
        value={value}
        onChange={(e) => void updateSetting(settingKey, e.target.value)}
        disabled={busy}
        variant="compact"
        rows={rows}
        className="w-full py-1.5 pr-9 font-mono text-[12.5px] leading-[1.3] resize-y"
      />
      <button
        type="button"
        onClick={() => void resetSetting(settingKey)}
        disabled={busy}
        title={t("common.reset", "Reset to default")}
        aria-label={t("common.reset", "Reset to default")}
        className="absolute top-1.5 right-1.5 flex items-center justify-center h-6 w-6 text-[#b8b8b8] hover:text-white transition-colors rounded bg-[#232323]/90 hover:bg-[#2d2d2d] border border-[#3d3d3d] hover:border-[#5a5a5a]"
      >
        <RotateCcw className="w-3 h-3" />
      </button>
    </div>
  );
};

/* ── Shared variable list snippets ── */

const CommonVars: React.FC = () => {
  const { t } = useTranslation();

  return (
    <>
      <Var name="${current_app}" desc={t("settings.aiReplace.promptHelp.variables.currentApp")} />
      <Var name="${language}" desc={t("settings.aiReplace.promptHelp.variables.language")} />
      <Var name="${profile_name}" desc={t("settings.aiReplace.promptHelp.variables.profileName")} />
      <Var name="${time_local}" desc={t("settings.aiReplace.promptHelp.variables.timeLocal")} />
      <Var name="${date_iso}" desc={t("settings.aiReplace.promptHelp.variables.dateIso")} />
      <Var
        name="${short_prev_transcript}"
        desc={t("settings.aiReplace.promptHelp.variables.shortPrevTranscript")}
      />
    </>
  );
};

type PromptHelpMode = "no-selection" | "quick-tap" | "with-selection";

const promptModeSuffixByMode: Record<PromptHelpMode, "noSelection" | "quickTap" | "withSelection"> = {
  "no-selection": "noSelection",
  "quick-tap": "quickTap",
  "with-selection": "withSelection",
};

const systemPromptTooltipKeyByMode = (mode: PromptHelpMode): string =>
  `settings.aiReplace.promptHelp.tooltips.systemPrompt.${promptModeSuffixByMode[mode]}`;

const userPromptTooltipKeyByMode = (mode: PromptHelpMode): string =>
  `settings.aiReplace.promptHelp.tooltips.userPrompt.${promptModeSuffixByMode[mode]}`;

const PromptPairHelp: React.FC<{ mode: PromptHelpMode }> = ({ mode }) => {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = React.useState(false);
  const modeSuffix = promptModeSuffixByMode[mode];
  const modeNote = t(`settings.aiReplace.promptHelp.modeNotes.${modeSuffix}`);
  const modeExample = t(`settings.aiReplace.promptHelp.examples.${modeSuffix}`);

  return (
    <div className="px-6 py-2">
      <button
        type="button"
        onClick={() => setIsOpen((prev) => !prev)}
        className="w-full flex items-center gap-1.5 text-xs text-[#c8c8c8] hover:text-white transition-colors"
      >
        <ChevronDown
          className={`w-3.5 h-3.5 text-[#9b9b9b] transition-transform ${isOpen ? "rotate-180" : ""}`}
        />
        {t("settings.aiReplace.promptHelp.toggleLabel")}
      </button>
      {isOpen && (
        <div className="mt-2 space-y-1">
          <p className="text-xs text-[#8f8f8f]">{modeNote}</p>
          <Var
            name="${instruction}"
            desc={t("settings.aiReplace.promptHelp.variables.instruction")}
            dim={mode === "quick-tap"}
          />
          <Var
            name="${output}"
            desc={t("settings.aiReplace.promptHelp.variables.output")}
            dim={mode === "no-selection"}
          />
          <Var
            name="${selection}"
            desc={t("settings.aiReplace.promptHelp.variables.selection")}
            dim={mode === "no-selection"}
          />
          <CommonVars />
          <ExamplePrompt label={t("settings.aiReplace.promptHelp.exampleLabel")}>
            {modeExample}
          </ExamplePrompt>
        </div>
      )}
    </div>
  );
};

/* ── Main component ── */

export const AiReplaceSelectionSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, getSetting, updateSetting, isUpdating } = useSettings();
  const aiReplaceState = useAiReplaceProviderState();

  return (
    <div className="max-w-3xl w-full mx-auto space-y-8 pb-12">

      {/* Help Banner */}
      <TellMeMore title={t("settings.aiReplace.help.title")}>
        <div className="space-y-3">
          <p>{t("settings.aiReplace.help.description")}</p>
          <ol className="list-decimal list-inside space-y-1 ml-1 opacity-90">
            <li>{t("settings.aiReplace.help.step1")}</li>
            <li>{t("settings.aiReplace.help.step2")}</li>
            <li>{t("settings.aiReplace.help.step3")}</li>
          </ol>

          <div className="mt-4 p-3 bg-red-500/10 border border-red-500/20 rounded-md">
            <p className="font-semibold text-red-300 mb-1">
              {t("settings.aiReplace.help.configRequiredTitle")}
            </p>
            <p className="text-xs">
              {t("settings.aiReplace.help.configRequiredBody")}
            </p>
          </div>

          <p className="pt-2">
            <strong>{t("settings.aiReplace.help.proTipTitle")}</strong>
            <br />
            {t("settings.aiReplace.help.proTipBody")}
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

      {/* ── No Selection Mode ── */}
      <SettingsGroup
        title={t("settings.aiReplace.noSelection.title")}
        description={t("settings.aiReplace.noSelection.description")}
      >
        <ToggleSwitch
          label={t("settings.aiReplace.noSelection.allowToggle.label")}
          description={t("settings.aiReplace.noSelection.allowToggle.description")}
          descriptionMode="tooltip"
          grouped={true}
          checked={getSetting("ai_replace_allow_no_selection") ?? true}
          onChange={(checked) => void updateSetting("ai_replace_allow_no_selection", checked)}
          disabled={isUpdating("ai_replace_allow_no_selection")}
        />
        {(getSetting("ai_replace_allow_no_selection") ?? true) && (
          <>
            <SettingContainer
              title={t("settings.aiReplace.noSelection.systemPrompt.title")}
              description={t(systemPromptTooltipKeyByMode("no-selection"))}
              descriptionMode="tooltip"
              grouped={true}
              layout="stacked"
              compact={true}
            >
              <PromptEditor settingKey="ai_replace_no_selection_system_prompt" rows={3} />
            </SettingContainer>
            <SettingContainer
              title={t("settings.aiReplace.noSelection.userPrompt.title")}
              description={t(userPromptTooltipKeyByMode("no-selection"))}
              descriptionMode="tooltip"
              grouped={true}
              layout="stacked"
              compact={true}
            >
              <PromptEditor settingKey="ai_replace_no_selection_user_prompt" rows={2} />
            </SettingContainer>
            <PromptPairHelp mode="no-selection" />
          </>
        )}
      </SettingsGroup>

      {/* ── Quick Tap Mode ── */}
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
              description={t(systemPromptTooltipKeyByMode("quick-tap"))}
              descriptionMode="tooltip"
              grouped={true}
              layout="stacked"
              compact={true}
            >
              <PromptEditor settingKey="ai_replace_quick_tap_system_prompt" rows={3} />
            </SettingContainer>
            <SettingContainer
              title={t("settings.aiReplace.quickTap.userPrompt.title")}
              description={t(userPromptTooltipKeyByMode("quick-tap"))}
              descriptionMode="tooltip"
              grouped={true}
              layout="stacked"
              compact={true}
            >
              <PromptEditor settingKey="ai_replace_quick_tap_user_prompt" rows={2} />
            </SettingContainer>
            <PromptPairHelp mode="quick-tap" />
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
                  min={AI_REPLACE_QUICK_TAP_THRESHOLD_MIN.toString()}
                  max={AI_REPLACE_QUICK_TAP_THRESHOLD_MAX.toString()}
                  step="50"
                  value={getSetting("ai_replace_quick_tap_threshold_ms") ?? 500}
                  onChange={(e) => {
                    const val = parseInt(e.target.value, 10);
                    if (!isNaN(val)) {
                      const clamped = Math.min(
                        AI_REPLACE_QUICK_TAP_THRESHOLD_MAX,
                        Math.max(AI_REPLACE_QUICK_TAP_THRESHOLD_MIN, val)
                      );
                      void updateSetting("ai_replace_quick_tap_threshold_ms", clamped);
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

      {/* ── With Selection Mode ── */}
      <SettingsGroup
        title={t("settings.aiReplace.withSelection.title")}
        description={t("settings.aiReplace.withSelection.description")}
      >
        <SettingContainer
          title={t("settings.aiReplace.withSelection.systemPrompt.title")}
          description={t(systemPromptTooltipKeyByMode("with-selection"))}
          descriptionMode="tooltip"
          grouped={true}
          layout="stacked"
          compact={true}
        >
          <PromptEditor settingKey="ai_replace_system_prompt" rows={3} />
        </SettingContainer>
        <SettingContainer
          title={t("settings.aiReplace.withSelection.userPrompt.title")}
          description={t(userPromptTooltipKeyByMode("with-selection"))}
          descriptionMode="tooltip"
          grouped={true}
          layout="stacked"
          compact={true}
        >
          <PromptEditor settingKey="ai_replace_user_prompt" rows={2} />
        </SettingContainer>
        <PromptPairHelp mode="with-selection" />
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
              value={getSetting("ai_replace_max_chars") ?? 20000}
              onChange={(e) => {
                const val = parseInt(e.target.value, 10);
                if (!isNaN(val) && val > 0) {
                  void updateSetting("ai_replace_max_chars", val);
                }
              }}
              disabled={isUpdating("ai_replace_max_chars")}
              className="w-24"
            />
            <span className="text-sm text-text">
              {t("settings.aiReplace.withSelection.maxChars.suffix")}
            </span>
          </div>
        </SettingContainer>
        <ToggleSwitch
          label={t("settings.aiReplace.withSelection.restoreOnError.label")}
          description={t("settings.aiReplace.withSelection.restoreOnError.description")}
          descriptionMode="tooltip"
          grouped={true}
          checked={getSetting("ai_replace_restore_on_error") ?? true}
          onChange={(checked) => void updateSetting("ai_replace_restore_on_error", checked)}
          disabled={isUpdating("ai_replace_restore_on_error")}
        />
      </SettingsGroup>

      <SettingsGroup title={t("settings.aiReplace.api.title")}>
        <LlmConfigSection
          title=""
          description={t("settings.aiReplace.api.description")}
          state={aiReplaceState}
          apiKeyFeature="ai_replace"
          showBaseUrl={false}
          reasoningSettingPrefix="ai_replace"
          sameAsSummary={t("settings.aiReplace.api.usingPostProcessingModel", { model: aiReplaceState.model })}
        />
      </SettingsGroup>
    </div>
  );
};
