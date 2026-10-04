import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const postProcessingSearchEntries = [
  { id: "postprocessing", section: "postprocessing", labelKey: "settingsSearch.items.postProcessing", fallbackLabel: "LLM post-processing", keywords: ["llm", "cleanup", "prompt", "post processing", "ai", "постобработка"] },
  { id: "postprocess-prompts", section: "postprocessing", anchor: "settings-postprocess-prompts", labelKey: "settings.postProcessing.prompts.title", fallbackLabel: "Post-Processing Prompts", groupLabelKey: "settingsSearch.items.postProcessing", groupFallbackLabel: "LLM post-processing", keywords: ["system prompt","user prompt","prompt templates","selected prompt","системный промпт","промпт постобработки","шаблон промпта"] },
] as const satisfies readonly SettingsSearchEntry[];
