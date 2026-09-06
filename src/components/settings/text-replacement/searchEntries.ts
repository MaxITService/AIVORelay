import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const textReplacementSearchEntries = [
  { id: "decapitalize-after-edit", section: "textReplacement", anchor: "decapitalize-after-edit-settings", labelKey: "settingsSearch.items.decapitalizeAfterEdit", fallbackLabel: "Decapitalize after manual edit", keywords: ["de", "dec", "decap", "decapitalize", "decapitalization", "lowercase", "lower case", "capitalization", "case", "декапитализация", "строчная буква", "нижний регистр", "регистр"] },
  { id: "custom-words", section: "textReplacement", anchor: "custom-words-settings", labelKey: "settingsSearch.items.customWords", fallbackLabel: "Custom Words", groupLabelKey: "settingsSearch.groups.vocabulary", groupFallbackLabel: "Vocabulary", keywords: ["vocabulary", "dictionary", "correction", "replacement", "словарь", "замена слов"] },
  { id: "text-processing", section: "textReplacement", labelKey: "settingsSearch.items.textProcessing", fallbackLabel: "Text replacement and processing", keywords: ["replacement", "regex", "fuzzy", "correction", "обработка текста"] },
] as const satisfies readonly SettingsSearchEntry[];
