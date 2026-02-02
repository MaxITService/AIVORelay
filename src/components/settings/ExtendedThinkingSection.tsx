import React from "react";
import { useTranslation } from "react-i18next";
import { Brain } from "lucide-react";
import { SettingContainer } from "../ui/SettingContainer";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { Input } from "../ui/Input";
import { useSettings } from "../../hooks/useSettings";

interface ExtendedThinkingSectionProps {
  /** Key prefix for settings: "post_process", "ai_replace", or "voice_command" */
  settingPrefix: "post_process" | "ai_replace" | "voice_command";
  /** Whether to show in grouped mode */
  grouped?: boolean;
}

/**
 * Reusable Extended Thinking configuration section.
 * Shows a toggle for enabling reasoning and a number input for the token budget.
 * Used in Post-Processing, AI Replace, and Voice Commands settings.
 */
export const ExtendedThinkingSection: React.FC<ExtendedThinkingSectionProps> = ({
  settingPrefix,
  grouped = true,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();

  // Build setting keys dynamically
  const enabledKey = `${settingPrefix}_reasoning_enabled` as any;
  const budgetKey = `${settingPrefix}_reasoning_budget` as any;

  const isEnabled = (getSetting(enabledKey) as boolean) || false;
  const budget = (getSetting(budgetKey) as number) || 2048;

  const handleEnabledChange = (enabled: boolean) => {
    updateSetting(enabledKey, enabled);
  };

  const handleBudgetChange = (value: string) => {
    const numValue = parseInt(value, 10);
    // Allow typing any valid number (validation happens on blur)
    if (!isNaN(numValue) && numValue > 0) {
      updateSetting(budgetKey, numValue);
    }
  };

  return (
    <div className="space-y-2">
      {/* Extended Thinking Toggle */}
      <SettingContainer
        title={t("settings.extendedThinking.title")}
        description={t("settings.extendedThinking.description")}
        descriptionMode="tooltip"
        layout="horizontal"
        grouped={grouped}
      >
        <div className="flex items-center gap-2">
          <Brain className="w-4 h-4 text-purple-400" />
          <ToggleSwitch
            checked={isEnabled}
            onChange={handleEnabledChange}
            disabled={isUpdating(enabledKey)}
            isUpdating={isUpdating(enabledKey)}
          />
        </div>
      </SettingContainer>

      {/* Token Budget input - only shown when enabled */}
      {isEnabled && (
        <SettingContainer
          title={t("settings.extendedThinking.budget.title")}
          description={t("settings.extendedThinking.budget.description")}
          descriptionMode="tooltip"
          layout="horizontal"
          grouped={grouped}
        >
          <div className="flex items-center gap-2">
            <Input
              type="number"
              value={budget}
              onChange={(e) => handleBudgetChange(e.target.value)}
              onBlur={(e) => {
                // Ensure minimum of 1024 on blur
                const numValue = parseInt(e.target.value, 10);
                if (isNaN(numValue) || numValue < 1024) {
                  updateSetting(budgetKey, 1024);
                }
              }}
              min={1024}
              step={256}
              disabled={isUpdating(budgetKey)}
              className="w-28"
              variant="compact"
            />
            <span className="text-xs text-text/50">
              {t("settings.extendedThinking.budget.tokens")}
            </span>
          </div>
        </SettingContainer>
      )}
    </div>
  );
};
