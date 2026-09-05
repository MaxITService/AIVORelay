import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const textReplacementSearchEntries = [
  { id: "quick-text-replacement", section: "textReplacement", anchor: "shortcut-quick_text_replacement", labelKey: "settingsSearch.items.quickTextReplacement", fallbackLabel: "Quick replacement shortcut", groupLabelKey: "settingsSearch.groups.shortcuts", groupFallbackLabel: "Shortcuts", keywords: ["quick replacement", "shortcut", "hotkey", "selected text", "быстрая замена", "горячая клавиша"] },
  { id: "custom-words", section: "textReplacement", anchor: "custom-words-settings", labelKey: "settingsSearch.items.customWords", fallbackLabel: "Custom Words", groupLabelKey: "settingsSearch.groups.vocabulary", groupFallbackLabel: "Vocabulary", keywords: ["vocabulary", "dictionary", "correction", "replacement", "словарь", "замена слов"] },
  { id: "text-processing", section: "textReplacement", labelKey: "settingsSearch.items.textProcessing", fallbackLabel: "Text replacement and processing", keywords: ["replacement", "regex", "fuzzy", "correction", "обработка текста"] },
] as const satisfies readonly SettingsSearchEntry[];
