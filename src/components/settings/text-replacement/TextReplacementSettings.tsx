import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Trash2, ArrowRight, HelpCircle, ChevronDown, ChevronUp, CaseSensitive, Regex, Check, X } from "lucide-react";
import { useSettings } from "@/hooks/useSettings";
import { SettingsGroup } from "@/components/ui/SettingsGroup";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { ToggleSwitch } from "@/components/ui/ToggleSwitch";
import { CustomWords } from "@/components/settings/CustomWords";
import { Slider } from "@/components/ui/Slider";
import { TellMeMore } from "@/components/ui/TellMeMore";

interface TextReplacementRule {
  id: string;
  from: string;
  to: string;
  enabled: boolean;
  case_sensitive: boolean;
  is_regex: boolean;
}

export const TextReplacementSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting, isUpdating } = useSettings();

  const [newFrom, setNewFrom] = useState("");
  const [newTo, setNewTo] = useState("");
  const [newCaseSensitive, setNewCaseSensitive] = useState(true);
  const [newIsRegex, setNewIsRegex] = useState(false);
  const [showHelp, setShowHelp] = useState(false);
  
  // Editing state
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editFrom, setEditFrom] = useState("");
  const [editTo, setEditTo] = useState("");

  const replacements: TextReplacementRule[] = (settings?.text_replacements ?? []).map((r: any) => ({
    ...r,
    case_sensitive: r.case_sensitive ?? true,
    is_regex: r.is_regex ?? false,
  }));
  const isEnabled = settings?.text_replacements_enabled ?? false;

  const handleAddRule = () => {
    if (!newFrom.trim()) return;

    const newRule: TextReplacementRule = {
      id: `tr_${Date.now()}`,
      from: newFrom,
      to: newTo,
      enabled: true,
      case_sensitive: newCaseSensitive,
      is_regex: newIsRegex,
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

  const handleToggleCaseSensitive = (id: string) => {
    updateSetting(
      "text_replacements",
      replacements.map((r) =>
        r.id === id ? { ...r, case_sensitive: !r.case_sensitive } : r
      )
    );
  };

  const handleToggleIsRegex = (id: string) => {
    updateSetting(
      "text_replacements",
      replacements.map((r) =>
        r.id === id ? { ...r, is_regex: !r.is_regex } : r
      )
    );
  };

  const startEditing = (rule: TextReplacementRule) => {
    setEditingId(rule.id);
    setEditFrom(rule.from);
    setEditTo(rule.to);
  };

  const cancelEditing = () => {
    setEditingId(null);
    setEditFrom("");
    setEditTo("");
  };

  const saveEditing = () => {
    if (!editingId || !editFrom.trim()) return;
    
    updateSetting(
      "text_replacements",
      replacements.map((r) =>
        r.id === editingId ? { ...r, from: editFrom, to: editTo } : r
      )
    );
    setEditingId(null);
    setEditFrom("");
    setEditTo("");
  };

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && newFrom.trim()) {
      e.preventDefault();
      handleAddRule();
    }
  };

  const handleEditKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      saveEditing();
    } else if (e.key === "Escape") {
      cancelEditing();
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

        {/* Apply Before LLM Toggle - only show when post-processing is enabled */}
        {settings?.post_process_enabled && (
          <div className="px-4 py-3 border-t border-white/[0.05]">
            <ToggleSwitch
              checked={settings?.text_replacements_before_llm ?? false}
              onChange={(enabled) =>
                updateSetting("text_replacements_before_llm", enabled)
              }
              isUpdating={isUpdating("text_replacements_before_llm")}
              label={t("textReplacement.beforeLlm", "Apply Before LLM Post-Processing")}
              description={t(
                "textReplacement.beforeLlmDescription",
                "When enabled, text replacements are applied BEFORE LLM processing. This prevents the LLM from modifying your replacement patterns."
              )}
              descriptionMode="inline"
            />
          </div>
        )}

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
                <li className="flex items-center gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5]">
                    \u{"{}"}
                  </code>
                  <span>→</span>
                  <span>
                    {t("textReplacement.escapeUnicode", "Unicode character (e.g., \\u{200D} for Zero Width Joiner)")}
                  </span>
                </li>
              </ul>

              <h4 className="font-medium text-[#f5f5f5] mt-4 mb-2">
                {t("textReplacement.optionsTitle", "Options")}
              </h4>
              <ul className="space-y-2 text-[#b8b8b8]">
                <li className="flex items-start gap-2">
                  <CaseSensitive className="w-4 h-4 mt-0.5 text-[#9b5de5] shrink-0" />
                  <span>
                    <strong>{t("textReplacement.caseSensitiveTitle", "Case Sensitive")}</strong> — {t("textReplacement.caseSensitiveDesc", "When enabled, 'Hello' and 'hello' are treated as different. When disabled, both will match.")}
                  </span>
                </li>
                <li className="flex items-start gap-2">
                  <Regex className="w-4 h-4 mt-0.5 text-[#f97316] shrink-0" />
                  <span>
                    <strong>{t("textReplacement.regexTitle", "Regular Expression")}</strong> — {t("textReplacement.regexDesc", "Enable to use regex patterns for advanced matching. Use $1, $2 in replacement for capture groups.")}
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
                  <code className="text-[#f97316]">\b(\w+)\s+\1\b</code> →{" "}
                  <code className="text-[#4ade80]">$1</code>
                  <span className="text-[#606060] ml-2">
                    {t("textReplacement.exampleRegex", "(remove repeated words)")}
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
          <div className="flex items-center gap-2 w-full mb-2">
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
          </div>
          {/* Options row */}
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <button
                onClick={() => setNewCaseSensitive(!newCaseSensitive)}
                className={`flex items-center gap-1.5 px-2 py-1 rounded text-xs transition-colors ${
                  newCaseSensitive
                    ? "bg-[#9b5de5]/20 text-[#9b5de5] border border-[#9b5de5]/30"
                    : "bg-[#252525] text-[#606060] border border-[#333333]"
                }`}
                title={t("textReplacement.caseSensitiveTooltip", "Toggle case sensitivity")}
              >
                <CaseSensitive className="w-3.5 h-3.5" />
                {t("textReplacement.caseSensitiveShort", "Aa")}
              </button>
              <button
                onClick={() => setNewIsRegex(!newIsRegex)}
                className={`flex items-center gap-1.5 px-2 py-1 rounded text-xs transition-colors ${
                  newIsRegex
                    ? "bg-[#f97316]/20 text-[#f97316] border border-[#f97316]/30"
                    : "bg-[#252525] text-[#606060] border border-[#333333]"
                }`}
                title={t("textReplacement.regexTooltip", "Toggle regex mode")}
              >
                <Regex className="w-3.5 h-3.5" />
                {t("textReplacement.regexShort", ".*")}
              </button>
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
                  className={`p-3 rounded-lg border transition-all ${
                    rule.enabled
                      ? "bg-[#1a1a1a] border-[#333333]"
                      : "bg-[#0f0f0f] border-[#252525] opacity-60"
                  }`}
                >
                  {/* Main row */}
                  <div className="flex items-center gap-3">
                    {/* Enable/Disable Checkbox */}
                    <input
                      type="checkbox"
                      checked={rule.enabled}
                      onChange={() => handleToggleRule(rule.id)}
                      className="accent-[#9b5de5] w-4 h-4 rounded shrink-0"
                      disabled={isUpdating("text_replacements")}
                    />

                    {editingId === rule.id ? (
                      /* Edit mode */
                      <>
                        <div className="flex-1 min-w-0">
                          <Input
                            type="text"
                            className="w-full"
                            value={editFrom}
                            onChange={(e) => setEditFrom(e.target.value)}
                            onKeyDown={handleEditKeyPress}
                            variant="compact"
                            autoFocus
                          />
                        </div>
                        <ArrowRight className="w-4 h-4 text-[#606060] shrink-0" />
                        <div className="flex-1 min-w-0">
                          <Input
                            type="text"
                            className="w-full"
                            value={editTo}
                            onChange={(e) => setEditTo(e.target.value)}
                            onKeyDown={handleEditKeyPress}
                            variant="compact"
                          />
                        </div>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={saveEditing}
                          className="shrink-0 text-[#4ade80] hover:text-[#22c55e]"
                          title={t("textReplacement.save", "Save")}
                        >
                          <Check className="w-4 h-4" />
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={cancelEditing}
                          className="shrink-0 text-[#808080] hover:text-red-400"
                          title={t("textReplacement.cancel", "Cancel")}
                        >
                          <X className="w-4 h-4" />
                        </Button>
                      </>
                    ) : (
                      /* View mode */
                      <>
                        {/* From - clickable to edit */}
                        <div 
                          className="flex-1 min-w-0 cursor-pointer"
                          onClick={() => startEditing(rule)}
                          title={t("textReplacement.clickToEdit", "Click to edit")}
                        >
                          <code
                            className={`text-sm px-2 py-1 rounded block truncate hover:ring-1 hover:ring-[#9b5de5]/50 ${
                              rule.enabled
                                ? "bg-[#252525] text-[#f5f5f5]"
                                : "bg-[#1a1a1a] text-[#808080]"
                            } ${rule.is_regex ? "border-l-2 border-[#f97316]" : ""}`}
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

                        {/* To - clickable to edit */}
                        <div 
                          className="flex-1 min-w-0 cursor-pointer"
                          onClick={() => startEditing(rule)}
                          title={t("textReplacement.clickToEdit", "Click to edit")}
                        >
                          <code
                            className={`text-sm px-2 py-1 rounded block truncate hover:ring-1 hover:ring-[#9b5de5]/50 ${
                              rule.enabled
                                ? "bg-[#252525] text-[#4ade80]"
                                : "bg-[#1a1a1a] text-[#606060]"
                            }`}
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
                      </>
                    )}
                  </div>

                  {/* Options row - only show when not editing */}
                  {editingId !== rule.id && (
                    <div className="flex items-center gap-2 mt-2 ml-7">
                      <button
                        onClick={() => handleToggleCaseSensitive(rule.id)}
                        disabled={isUpdating("text_replacements")}
                        className={`flex items-center gap-1 px-1.5 py-0.5 rounded text-xs transition-colors ${
                          rule.case_sensitive
                            ? "bg-[#9b5de5]/20 text-[#9b5de5]"
                            : "bg-[#252525] text-[#606060]"
                        }`}
                        title={t("textReplacement.caseSensitiveTooltip", "Toggle case sensitivity")}
                      >
                        <CaseSensitive className="w-3 h-3" />
                      </button>
                      <button
                        onClick={() => handleToggleIsRegex(rule.id)}
                        disabled={isUpdating("text_replacements")}
                        className={`flex items-center gap-1 px-1.5 py-0.5 rounded text-xs transition-colors ${
                          rule.is_regex
                            ? "bg-[#f97316]/20 text-[#f97316]"
                            : "bg-[#252525] text-[#606060]"
                        }`}
                        title={t("textReplacement.regexTooltip", "Toggle regex mode")}
                      >
                        <Regex className="w-3 h-3" />
                      </button>
                    </div>
                  )}
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

      {/* Speech Clean-up Group */}
      <SettingsGroup
        title={t("textReplacement.cleanupTitle", "Speech Clean-up")}
        description={t(
          "textReplacement.cleanupDescription",
          "Automatically remove common speech artifacts from the final text."
        )}
      >
        {/* Filler Word Filter */}
        <div className="px-4 py-3">
          <ToggleSwitch
            checked={settings?.filler_word_filter_enabled ?? false}
            onChange={(enabled) =>
              updateSetting("filler_word_filter_enabled", enabled)
            }
            isUpdating={isUpdating("filler_word_filter_enabled")}
            label={t("audioProcessing.fillerFilter", "Remove Filler Words")}
            description={t(
              "audioProcessing.fillerFilterDescription",
              "Automatically remove 'uh', 'um', 'hmm' and similar filler words from transcriptions."
            )}
            descriptionMode="inline"
          />
        </div>

        {/* Filler Word Filter Help */}
        <div className="px-4 py-3 border-t border-white/[0.05]">
          <details className="group">
            <summary className="flex items-center gap-2 text-sm text-[#9b5de5] hover:text-[#b47eff] transition-colors cursor-pointer list-none">
              <HelpCircle className="w-4 h-4" />
              {t("audioProcessing.fillerHelpTitle", "Tell me more about filler word removal")}
              <ChevronDown className="w-4 h-4 group-open:rotate-180 transition-transform" />
            </summary>
            <div className="mt-3 p-4 bg-[#1a1a1a] rounded-lg border border-[#333333] text-sm">
              <h4 className="font-medium text-[#f5f5f5] mb-2">
                {t("audioProcessing.whatItDoes", "What it does")}
              </h4>
              <p className="text-[#b8b8b8] mb-3">
                {t(
                  "audioProcessing.fillerExplanation",
                  "This feature automatically removes common filler words and speech artifacts from your transcriptions:"
                )}
              </p>
              <ul className="space-y-1 text-[#b8b8b8] mb-3">
                <li>• <strong>{t("audioProcessing.fillerWords", "Filler words:")}</strong> uh, um, uhm, umm, ah, eh, hmm, hm, mmm</li>
                <li>• <strong>{t("audioProcessing.hallucinations", "Hallucinations:")}</strong> [AUDIO], (pause), {"<tag>...</tag>"}</li>
                <li>• <strong>{t("audioProcessing.stutters", "Stutters:")}</strong> "w wh wh wh why" → "wh why"</li>
              </ul>

              <h4 className="font-medium text-[#f5f5f5] mt-4 mb-2">
                {t("audioProcessing.howItWorksTitle", "How it works (technical)")}
              </h4>
              <p className="text-[#b8b8b8] mb-2">
                {t(
                  "audioProcessing.howItWorksIntro",
                  "The filter applies several regex patterns in sequence:"
                )}
              </p>
              <ul className="space-y-2 text-[#b8b8b8] mb-3">
                <li className="flex items-start gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5] text-xs whitespace-nowrap shrink-0">{"<TAG>...</TAG>"}</code>
                  <span>→ {t("audioProcessing.regexTagBlock", "Removes XML-style tag blocks (model hallucinations)")}</span>
                </li>
                <li className="flex items-start gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5] text-xs whitespace-nowrap shrink-0">[...]  (...)  {"{"}"...{"}"}</code>
                  <span>→ {t("audioProcessing.regexBrackets", "Removes bracketed content like [AUDIO], (pause), {noise}")}</span>
                </li>
                <li className="flex items-start gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5] text-xs whitespace-nowrap shrink-0">\\b(uh|um|...)\\b</code>
                  <span>→ {t("audioProcessing.regexFillers", "Removes filler words with word boundaries")}</span>
                </li>
                <li className="flex items-start gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5] text-xs whitespace-nowrap shrink-0">{t("audioProcessing.regexStutterPattern", "3+ repetitions")}</code>
                  <span>→ {t("audioProcessing.regexStutters", "Collapses repeated 1-2 letter words (I I I I → I)")}</span>
                </li>
              </ul>

              <div className="mt-4 p-3 bg-[#2a2010] rounded border border-[#f97316]/30">
                <p className="text-[#f5f5f5] text-xs">
                  <strong className="text-[#f97316]">
                    {t("audioProcessing.languageWarningTitle", "⚠️ Non-English Languages:")}
                  </strong>{" "}
                  {t(
                    "audioProcessing.languageWarning",
                    "This feature is optimized for English. If you experience issues with other languages (missing words, incorrect filtering), try disabling this option."
                  )}
                </p>
              </div>
            </div>
          </details>
        </div>

        {/* Zero-Width Character Filter */}
        <div className="px-4 py-3 border-t border-white/[0.05]">
          <ToggleSwitch
            checked={settings?.zero_width_filter_enabled ?? true}
            onChange={(enabled) =>
              updateSetting("zero_width_filter_enabled", enabled)
            }
            isUpdating={isUpdating("zero_width_filter_enabled")}
            label={t("audioProcessing.zeroWidthFilter", "Strip Invisible Characters")}
            description={t(
              "audioProcessing.zeroWidthFilterDescription",
              "Remove invisible Unicode characters from LLM output (post-processing and AI Replace)."
            )}
            descriptionMode="inline"
          />
        </div>

        {/* Zero-Width Filter Help */}
        <div className="px-4 py-3 border-t border-white/[0.05]">
          <details className="group">
            <summary className="flex items-center gap-2 text-sm text-[#9b5de5] hover:text-[#b47eff] transition-colors cursor-pointer list-none">
              <HelpCircle className="w-4 h-4" />
              {t("audioProcessing.zeroWidthHelpTitle", "Tell me more about invisible character removal")}
              <ChevronDown className="w-4 h-4 group-open:rotate-180 transition-transform" />
            </summary>
            <div className="mt-3 p-4 bg-[#1a1a1a] rounded-lg border border-[#333333] text-sm">
              <p className="text-[#b8b8b8] mb-3">
                {t(
                  "audioProcessing.zeroWidthExplanation",
                  "Some LLM providers (notably Qwen) insert invisible Unicode characters into their responses. These are zero-width characters that you can't see, but they can cause issues when pasted into other applications."
                )}
              </p>
              <h4 className="font-medium text-[#f5f5f5] mb-2">
                {t("audioProcessing.zeroWidthWhatRemoved", "Characters removed:")}
              </h4>
              <ul className="space-y-1 text-[#b8b8b8] mb-3">
                <li className="flex items-start gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5] text-xs whitespace-nowrap shrink-0">U+200B</code>
                  <span>{t("audioProcessing.zeroWidthZWS", "Zero-Width Space (U+200B)")}</span>
                </li>
                <li className="flex items-start gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5] text-xs whitespace-nowrap shrink-0">U+200C</code>
                  <span>{t("audioProcessing.zeroWidthZWNJ", "Zero-Width Non-Joiner (U+200C)")}</span>
                </li>
                <li className="flex items-start gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5] text-xs whitespace-nowrap shrink-0">U+200D</code>
                  <span>{t("audioProcessing.zeroWidthZWJ", "Zero-Width Joiner (U+200D)")}</span>
                </li>
                <li className="flex items-start gap-2">
                  <code className="px-2 py-0.5 bg-[#252525] rounded text-[#9b5de5] text-xs whitespace-nowrap shrink-0">U+FEFF</code>
                  <span>{t("audioProcessing.zeroWidthBOM", "Byte Order Mark / Zero-Width No-Break Space (U+FEFF)")}</span>
                </li>
              </ul>
              <p className="text-[#b8b8b8] mb-3">
                {t(
                  "audioProcessing.zeroWidthIssues",
                  "These invisible characters can cause unexpected behavior: broken text searches, incorrect string lengths, copy-paste issues, and invisible formatting problems in documents."
                )}
              </p>
              <div className="mt-3 p-3 bg-[#252525] rounded border border-[#444444]">
                <p className="text-[#b8b8b8] text-xs">
                  <strong className="text-[#f5f5f5]">
                    {t("audioProcessing.noteTitle", "Note:")}
                  </strong>{" "}
                  {t(
                    "audioProcessing.zeroWidthNote",
                    "This filter applies to all LLM output — both post-processing and AI Replace. It's safe to leave enabled for all providers."
                  )}
                </p>
              </div>
            </div>
          </details>
        </div>
      </SettingsGroup>

      {/* Fuzzy Word Correction Group */}
      <SettingsGroup
        title="Fuzzy Word Correction"
        description="Add words that are often misheard (names, technical terms). The system will automatically correct similar-sounding words."
      >
        <div className="px-4 py-3 bg-white/[0.02] border-b border-white/[0.05]">
          <TellMeMore title="How Fuzzy Correction Works">
            <div className="space-y-3 text-sm">
              <p>
                This algorithm fixes misheard words by comparing them to your custom list using two methods:
              </p>
              <ul className="list-disc list-inside space-y-2 ml-1 opacity-90">
                <li>
                  <strong>Sounds Like (Phonetic):</strong> It recognizes that "edge" and "etch" sound similar.
                </li>
                <li>
                  <strong>Looks Like (Levenshtein):</strong> It catches typos like "srart" instead of "start".
                </li>
              </ul>
              <p className="pt-1 text-xs text-text/70 italic">
                Tip: If it corrects words too aggressively, lower the sensitivity slider below.
              </p>
            </div>
          </TellMeMore>
        </div>

        <CustomWords descriptionMode="inline" grouped={true} />

        {/* N-gram toggle for multi-word fuzzy correction */}
        <div className="px-4 py-3 border-t border-white/[0.05]">
          <ToggleSwitch
            checked={(settings as any)?.custom_words_ngram_enabled ?? true}
            onChange={(enabled) =>
              (updateSetting as any)("custom_words_ngram_enabled", enabled)
            }
            isUpdating={isUpdating("custom_words_ngram_enabled")}
            label="Enable Multi-word Matching (N-grams)"
            description="Match up to 3 spoken tokens as one custom term (example: 'Chat G P T' -> 'ChatGPT'). Disable if corrections are too aggressive."
            descriptionMode="inline"
            grouped={true}
          />
        </div>
        
        {/* Word Correction Threshold */}
        <div className="px-4 py-3 border-t border-white/[0.05]">
          <Slider
            value={settings?.word_correction_threshold ?? 0.18}
            onChange={(value) => updateSetting("word_correction_threshold", value)}
            min={0.0}
            max={1.0}
            label="Correction Sensitivity"
            description="Threshold for fuzzy match score (0.0 = exact match only, 1.0 = accept any). Default 0.18 means a word must be ~82% similar to be corrected."
            descriptionMode="inline"
            grouped={true}
          />
        </div>
      </SettingsGroup>

      <SettingsGroup
        title={t("textReplacement.outputWhitespaceTitle", "Output Whitespace")}
        description={t(
          "textReplacement.outputWhitespaceDescription",
          "Control whether transcription output keeps or removes leading/trailing whitespace."
        )}
      >
        <div className="px-4 py-3">
          <ToggleSwitch
            checked={settings?.trim_transcription_output_enabled ?? true}
            onChange={(enabled) =>
              updateSetting("trim_transcription_output_enabled", enabled)
            }
            isUpdating={isUpdating("trim_transcription_output_enabled")}
            label={t(
              "textReplacement.trimOutputLabel",
              "Trim Leading/Trailing Whitespace"
            )}
            description={t(
              "textReplacement.trimOutputDescription",
              "When enabled, whitespace at the start and end of transcription output is removed before final output. Whitespace inside the text is preserved."
            )}
            descriptionMode="inline"
          />
        </div>
      </SettingsGroup>

      <SettingsGroup
        title={t(
          "textReplacement.sonioxRealtimeChunkTitle",
          "Soniox Realtime Chunks"
        )}
        description={t(
          "textReplacement.sonioxRealtimeChunkDescription",
          "Controls chunk-time correction while Soniox Live typing is active."
        )}
      >
        <div className="px-4 py-3">
          <ToggleSwitch
            checked={settings?.soniox_realtime_fuzzy_correction_enabled ?? false}
            onChange={(enabled) =>
              updateSetting("soniox_realtime_fuzzy_correction_enabled", enabled)
            }
            isUpdating={isUpdating("soniox_realtime_fuzzy_correction_enabled")}
            label={t(
              "textReplacement.sonioxRealtimeChunkFuzzyLabel",
              "Enable Fuzzy Word Correction for Soniox Live Chunks"
            )}
            description={t(
              "textReplacement.sonioxRealtimeChunkFuzzyDescription",
              "When enabled, Soniox Live chunk typing applies Custom Words fuzzy correction before Text Replacement rules."
            )}
            descriptionMode="inline"
          />
        </div>

        <div className="px-4 py-3 border-t border-white/[0.05]">
          <ToggleSwitch
            checked={settings?.soniox_realtime_keep_safety_buffer_enabled ?? false}
            onChange={(enabled) =>
              updateSetting("soniox_realtime_keep_safety_buffer_enabled", enabled)
            }
            isUpdating={isUpdating("soniox_realtime_keep_safety_buffer_enabled")}
            label={t(
              "textReplacement.sonioxRealtimeChunkSafetyBufferLabel",
              "Keep Safety Buffer for Cross-chunk Matching"
            )}
            description={t(
              "textReplacement.sonioxRealtimeChunkSafetyBufferDescription",
              "When enabled, Soniox Live keeps a short 2-3 word tail before typing so fuzzy correction can match across chunk boundaries. This may delay the newest words slightly."
            )}
            descriptionMode="inline"
          />
        </div>
      </SettingsGroup>
    </div>
  );
};

export default TextReplacementSettings;
