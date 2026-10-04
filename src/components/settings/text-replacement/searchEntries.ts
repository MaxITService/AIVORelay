import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const textReplacementSearchEntries = [
  { id: "decapitalize-after-edit", section: "textReplacement", anchor: "decapitalize-after-edit-settings", labelKey: "settingsSearch.items.decapitalizeAfterEdit", fallbackLabel: "Decapitalize after manual edit", keywords: ["de", "dec", "decap", "decapitalize", "decapitalization", "lowercase", "lower case", "capitalization", "case", "декапитализация", "строчная буква", "нижний регистр", "регистр"] },
  { id: "custom-words", section: "textReplacement", anchor: "custom-words-settings", labelKey: "settingsSearch.items.customWords", fallbackLabel: "Custom Words", groupLabelKey: "settingsSearch.groups.vocabulary", groupFallbackLabel: "Vocabulary", keywords: ["vocabulary", "dictionary", "correction", "replacement", "словарь", "замена слов"] },
  { id: "text-processing", section: "textReplacement", labelKey: "settingsSearch.items.textProcessing", fallbackLabel: "Text replacement and processing", keywords: ["replacement", "regex", "fuzzy", "correction", "обработка текста"] },
  { id: "text-replacement-enabled", section: "textReplacement", anchor: "settings-text-replacement-enabled", labelKey: "textReplacement.enable", fallbackLabel: "Enable Text Replacement", keywords: ["enable replacements","replacement rules","text replacement","замены текста","правила замены"] },
  { id: "text-replacement-before-llm", section: "textReplacement", anchor: "settings-text-replacement-before-llm", labelKey: "textReplacement.beforeLlm", fallbackLabel: "Apply Before LLM Post-Processing", keywords: ["before llm","replacement order","before post processing","до llm","порядок обработки"] },
] as const satisfies readonly SettingsSearchEntry[];
