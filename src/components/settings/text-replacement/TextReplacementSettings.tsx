import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Trash2, ArrowRight, HelpCircle, ChevronDown, ChevronUp } from "lucide-react";
import { useSettings } from "@/hooks/useSettings";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { ToggleSwitch } from "@/components/ui/ToggleSwitch";

interface TextReplacementRule {
  id: string;
  from: string;
  to: string;
  enabled: boolean;
}

export const TextReplacementSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting, isUpdating } = useSettings();

  const [newFrom, setNewFrom] = useState("");
  const [newTo, setNewTo] = useState("");
  const [showHelp, setShowHelp] = useState(false);

  const replacements: TextReplacementRule[] = settings?.text_replacements ?? [];
  const isEnabled = settings?.text_replacements_enabled ?? false;

  const handleAddRule = () => {
    if (!newFrom.trim()) return;

    const newRule: TextReplacementRule = {
      id: `tr_${Date.now()}`,
      from: newFrom,
      to: newTo,
      enabled: true,
    };

    updateSetting("text_replacements", [...replacements, newRule]);
    setNewFrom("");
    setNewTo("");
  };

  const handleRemoveRule = (id: string) => {
    updateSetting(
      "text_replacements",
      replacements.filter((r) => r.id !== id)
    );
  };

  const handleToggleRule = (id: string) => {
    updateSetting(
      "text_replacements",
      replacements.map((r) =>
        r.id === id ? { ...r, enabled: !r.enabled } : r
      )
    );
  };

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && newFrom.trim()) {
      e.preventDefault();
      handleAddRule();
    }
  };

  // Format display text to show escape sequences visually
  const formatDisplayText = (text: string): string => {
    if (!text) return t("textReplacement.emptyValue", "(empty)");
    return text
      .replace(/\n/g, "⏎")
      .replace(/\r/g, "↵")
      .replace(/\t/g, "⇥");
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6 pb-12">
      {/* Main Settings Group */}
      <SettingsGroup
        title={t("textReplacement.title", "Text Replacement")}
        description={t(
          "textReplacement.description",
          "Automatically replace text patterns in transcriptions. Useful for fixing commonly misheard words or applying consistent formatting."
        )}
      >
        {/* Enable Toggle */}
        <div className="px-4 py-3">
          <ToggleSwitch
            checked={isEnabled}
            onChange={(enabled) =>
              updateSetting("text_replacements_enabled", enabled)
            }
            isUpdating={isUpdating("text_replacements_enabled")}
            label={t("textReplacement.enable", "Enable Text Replacement")}
            description={t(
              "textReplacement.enableDescription",
              "Apply replacement rules to all transcriptions after processing."
            )}
            descriptionMode="inline"
          />
        </div>

        {/* Help Section */}
        <div className="px-4 py-3 border-t border-white/[0.05]">
          <button
            onClick={() => setShowHelp(!showHelp)}
            className="flex items-center gap-2 text-sm text-[#9b5de5] hover:text-[#b47eff] transition-colors"
          >
            <HelpCircle className="w-4 h-4" />
            {t("textReplacement.helpTitle", "How to use special characters")}
            {showHelp ? (
              <ChevronUp className="w-4 h-4" />
            ) : (
              <ChevronDown className="w-4 h-4" />
            )}
          </button>

          {showHelp && (
            <div className="mt-3 p-4 bg-[#1a1a1a] rounded-lg border border-[#333333] text-sm">
              <h4 className="font-medium text-[#f5f5f5] mb-2">
                {t("textReplacement.escapeSequences", "Escape Sequences")}
              </h4>
              <p className="text-[#b8b8b8] mb-3">
                {t(
                  "textReplacement.escapeIntro",
                  "Use these codes to match or insert special characters:"
                )}
              </p>
              <ul className="space-y-2 text-[#b8b8b8]">
                <li className="flex items-center gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5]">
                    \n
                  </code>
                  <span>→</span>
                  <span>
                    {t(
                      "textReplacement.escapeNewline",
                      "Line break (LF - Unix/Mac style)"
                    )}
                  </span>
                </li>
                <li className="flex items-center gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5]">
                    \r\n
                  </code>
                  <span>→</span>
                  <span>
                    {t(
                      "textReplacement.escapeCRLF",
                      "Line break (CRLF - Windows style)"
                    )}
                  </span>
                </li>
                <li className="flex items-center gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5]">
                    \r
                  </code>
                  <span>→</span>
                  <span>
                    {t(
                      "textReplacement.escapeCarriageReturn",
                      "Carriage return (CR - old Mac style)"
                    )}
                  </span>
                </li>
                <li className="flex items-center gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5]">
                    \t
                  </code>
                  <span>→</span>
                  <span>{t("textReplacement.escapeTab", "Tab character")}</span>
                </li>
                <li className="flex items-center gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5]">
                    \\
                  </code>
                  <span>→</span>
                  <span>
                    {t("textReplacement.escapeBackslash", "Literal backslash")}
                  </span>
                </li>
              </ul>

              <h4 className="font-medium text-[#f5f5f5] mt-4 mb-2">
                {t("textReplacement.examples", "Examples")}
              </h4>
              <ul className="space-y-2 text-[#b8b8b8]">
                <li>
                  <code className="text-[#808080]">teh</code> →{" "}
                  <code className="text-[#4ade80]">the</code>
                  <span className="text-[#606060] ml-2">
                    {t("textReplacement.exampleTypo", "(fix typo)")}
                  </span>
                </li>
                <li>
                  <code className="text-[#808080]">.\n</code> →{" "}
                  <code className="text-[#4ade80]">.\n\n</code>
                  <span className="text-[#606060] ml-2">
                    {t(
                      "textReplacement.exampleParagraph",
                      "(double-space after periods)"
                    )}
                  </span>
                </li>
                <li>
                  <code className="text-[#808080]">gonna</code> →{" "}
                  <code className="text-[#4ade80]">going to</code>
                  <span className="text-[#606060] ml-2">
                    {t("textReplacement.exampleFormal", "(formal style)")}
                  </span>
                </li>
              </ul>

              <div className="mt-4 p-3 bg-[#252525] rounded border border-[#444444]">
                <p className="text-[#b8b8b8] text-xs">
                  <strong className="text-[#f5f5f5]">
                    {t("textReplacement.noteTitle", "Note:")}
                  </strong>{" "}
                  {t(
                    "textReplacement.noteContent",
                    "For Windows line endings conversion, consider using the 'Convert LF to CRLF' option in Advanced settings instead — it handles this automatically for clipboard paste operations."
                  )}
                </p>
              </div>
            </div>
          )}
        </div>

        {/* Add New Rule */}
        <div className="px-4 py-4 border-t border-white/[0.05] overflow-hidden">
          <div className="flex items-center gap-2 w-full">
            <div className="flex-1 min-w-0">
              <Input
                type="text"
                className="w-full"
                value={newFrom}
                onChange={(e) => setNewFrom(e.target.value)}
                onKeyDown={handleKeyPress}
                placeholder={t("textReplacement.fromPlaceholder", "Find text...")}
                variant="compact"
                disabled={isUpdating("text_replacements")}
              />
            </div>
            <ArrowRight className="w-4 h-4 text-[#606060] shrink-0" />
            <div className="flex-1 min-w-0">
              <Input
                type="text"
                className="w-full"
                value={newTo}
                onChange={(e) => setNewTo(e.target.value)}
                onKeyDown={handleKeyPress}
                placeholder={t(
                  "textReplacement.toPlaceholder",
                  "Replace with..."
                )}
                variant="compact"
                disabled={isUpdating("text_replacements")}
              />
            </div>
            <Button
              onClick={handleAddRule}
              disabled={!newFrom.trim() || isUpdating("text_replacements")}
              variant="primary"
              size="md"
              className="shrink-0"
            >
              <Plus className="w-4 h-4" />
            </Button>
          </div>
        </div>

        {/* Rules List */}
        {replacements.length > 0 && (
          <div className="px-4 py-3 border-t border-white/[0.05]">
            <div className="space-y-2">
              {replacements.map((rule) => (
                <div
                  key={rule.id}
                  className={`flex items-center gap-3 p-3 rounded-lg border transition-all ${
                    rule.enabled
                      ? "bg-[#1a1a1a] border-[#333333]"
                      : "bg-[#0f0f0f] border-[#252525] opacity-60"
                  }`}
                >
                  {/* Enable/Disable Checkbox */}
                  <input
                    type="checkbox"
                    checked={rule.enabled}
                    onChange={() => handleToggleRule(rule.id)}
                    className="accent-[#9b5de5] w-4 h-4 rounded shrink-0"
                    disabled={isUpdating("text_replacements")}
                  />

                  {/* From */}
                  <div className="flex-1 min-w-0">
                    <code
                      className={`text-sm px-2 py-1 rounded block truncate ${
                        rule.enabled
                          ? "bg-[#252525] text-[#f5f5f5]"
                          : "bg-[#1a1a1a] text-[#808080]"
                      }`}
                      title={rule.from}
                    >
                      {formatDisplayText(rule.from)}
                    </code>
                  </div>

                  {/* Arrow */}
                  <ArrowRight
                    className={`w-4 h-4 shrink-0 ${
                      rule.enabled ? "text-[#9b5de5]" : "text-[#444444]"
                    }`}
                  />

                  {/* To */}
                  <div className="flex-1 min-w-0">
                    <code
                      className={`text-sm px-2 py-1 rounded block truncate ${
                        rule.enabled
                          ? "bg-[#252525] text-[#4ade80]"
                          : "bg-[#1a1a1a] text-[#606060]"
                      }`}
                      title={rule.to}
                    >
                      {formatDisplayText(rule.to)}
                    </code>
                  </div>

                  {/* Delete Button */}
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => handleRemoveRule(rule.id)}
                    disabled={isUpdating("text_replacements")}
                    className="shrink-0 text-[#808080] hover:text-red-400"
                    title={t("textReplacement.delete", "Delete rule")}
                  >
                    <Trash2 className="w-4 h-4" />
                  </Button>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Empty State */}
        {replacements.length === 0 && (
          <div className="px-4 py-6 text-center text-[#606060]">
            <p className="text-sm">
              {t(
                "textReplacement.empty",
                "No replacement rules yet. Add one above to get started."
              )}
            </p>
          </div>
        )}
      </SettingsGroup>
    </div>
  );
};

export default TextReplacementSettings;
