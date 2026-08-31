import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const postProcessingSearchEntries = [
  { id: "postprocessing", section: "postprocessing", labelKey: "settingsSearch.items.postProcessing", fallbackLabel: "LLM post-processing", keywords: ["llm", "cleanup", "prompt", "post processing", "ai", "постобработка"] },
] as const satisfies readonly SettingsSearchEntry[];
