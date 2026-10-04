import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const postProcessingSearchEntries = [
  { id: "postprocessing", section: "postprocessing", labelKey: "settingsSearch.items.postProcessing", fallbackLabel: "LLM post-processing", keywords: ["llm", "cleanup", "prompt", "post processing", "ai", "постобработка"] },
  { id: "postprocess-prompts", section: "postprocessing", anchor: "settings-postprocess-prompts", labelKey: "settings.postProcessing.prompts.title", fallbackLabel: "Post-Processing Prompts", groupLabelKey: "settingsSearch.items.postProcessing", groupFallbackLabel: "LLM post-processing", keywords: ["system prompt","user prompt","prompt templates","selected prompt","системный промпт","промпт постобработки","шаблон промпта"] },
  { id: "postprocess-api", section: "postprocessing", anchor: "settings-postprocess-api", labelKey: "settings.postProcessing.api.title", fallbackLabel: "API Configuration", groupLabelKey: "settingsSearch.items.postProcessing", groupFallbackLabel: "LLM post-processing", keywords: ["llm provider","llm model","llm api key","llm endpoint","base url","openrouter","ollama","lm studio","провайдер llm","ключ llm","адрес api"] },
] as const satisfies readonly SettingsSearchEntry[];
