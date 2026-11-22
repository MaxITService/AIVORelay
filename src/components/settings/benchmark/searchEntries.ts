import type { SettingsSearchEntry } from "../settingsSearchTypes";

export const benchmarkSearchEntries = [
  {
    id: "benchmark",
    section: "benchmark",
    labelKey: "sidebar.benchmark",
    fallbackLabel: "Benchmark",
    keywords: [
      "benchmark",
      "compare providers",
      "compare models",
      "test recording",
      "boost sweep",
      "noise sweep",
      "rnnoise",
      "vad",
      "бенчмарк",
      "сравнить провайдеры",
      "усиление звука",
      "удаление шума",
      "тестовая запись",
    ],
  },
] as const satisfies readonly SettingsSearchEntry[];
