import { useCallback, useMemo, useState } from "react";
import type { ModelInfo } from "@/bindings";

// Size range presets (in MB)
const SIZE_RANGES = {
  "<100": { min: 0, max: 100 },
  "100-500": { min: 100, max: 500 },
  "500-1000": { min: 500, max: 1000 },
  ">1000": { min: 1000, max: Infinity },
} as const;

export type SizeRangeKey = keyof typeof SIZE_RANGES;

// Canonical engine keys used in the filter UI
export type EngineFilterKey = "Whisper" | "TranscribeCpp" | "Onnx" | "MoonshineStreaming";

export type ModelFilters = {
  search: string;
  engines: Set<EngineFilterKey>;
  sizeRanges: Set<SizeRangeKey>;
  supportsTranslation: boolean | null; // null = any
  supportsStreaming: boolean | null;
  recommendedOnly: boolean;
  languages: Set<string>; // ISO codes
};

const DEFAULT_FILTERS: ModelFilters = {
  search: "",
  engines: new Set(),
  sizeRanges: new Set(),
  supportsTranslation: null,
  supportsStreaming: null,
  recommendedOnly: false,
  languages: new Set(),
};

// Map ModelInfo.engine_type to our canonical filter keys
function engineFilterKey(engineType: string): EngineFilterKey {
  switch (engineType) {
    case "Whisper":
      return "Whisper";
    case "TranscribeCpp":
      return "TranscribeCpp";
    case "MoonshineStreaming":
      return "MoonshineStreaming";
    default:
      return "Onnx";
  }
}

function matchesFilters(model: ModelInfo, filters: ModelFilters, searchLower: string): boolean {
  // Text search (case-insensitive on name + description)
  if (searchLower) {
    const haystack = `${model.name} ${model.description} ${model.id}`.toLowerCase();
    if (!haystack.includes(searchLower)) return false;
  }

  // Engine filter (OR within dimension)
  if (filters.engines.size > 0) {
    if (!filters.engines.has(engineFilterKey(model.engine_type))) return false;
  }

  // Size range filter (OR within dimension)
  if (filters.sizeRanges.size > 0) {
    const sizeMb = Number(model.size_mb);
    let matchesAny = false;
    for (const key of filters.sizeRanges) {
      const range = SIZE_RANGES[key];
      if (sizeMb >= range.min && sizeMb < range.max) {
        matchesAny = true;
        break;
      }
    }
    if (!matchesAny) return false;
  }

  // Translation filter
  if (filters.supportsTranslation !== null) {
    if (model.supports_translation !== filters.supportsTranslation) return false;
  }

  // Native streaming filter
  if (filters.supportsStreaming !== null) {
    if (model.supports_streaming !== filters.supportsStreaming) return false;
  }

  // Recommended only
  if (filters.recommendedOnly) {
    if (!model.is_recommended) return false;
  }

  // Language filter (model must contain ANY of the selected languages)
  if (filters.languages.size > 0) {
    const modelLangs = new Set(model.supported_languages);
    let matchesAny = false;
    for (const lang of filters.languages) {
      if (modelLangs.has(lang)) {
        matchesAny = true;
        break;
      }
    }
    if (!matchesAny) return false;
  }

  return true;
}

export function useModelFilters() {
  const [filters, setFilters] = useState<ModelFilters>({ ...DEFAULT_FILTERS });

  const isAnyFilterActive = useMemo(() => {
    return (
      filters.search !== "" ||
      filters.engines.size > 0 ||
      filters.sizeRanges.size > 0 ||
      filters.supportsTranslation !== null ||
      filters.supportsStreaming !== null ||
      filters.recommendedOnly ||
      filters.languages.size > 0
    );
  }, [filters]);

  const applyFilters = useCallback(
    (models: ModelInfo[]): ModelInfo[] => {
      if (!isAnyFilterActive) return models;
      const searchLower = filters.search.toLowerCase().trim();
      return models.filter((m) => matchesFilters(m, filters, searchLower));
    },
    [filters, isAnyFilterActive],
  );

  const resetFilters = useCallback(() => {
    setFilters({ ...DEFAULT_FILTERS });
  }, []);

  // Convenience: toggle a value in a Set-typed filter dimension
  const toggleSetValue = useCallback(
    <K extends "engines" | "sizeRanges" | "languages">(
      key: K,
      value: ModelFilters[K] extends Set<infer V> ? V : never,
    ) => {
      setFilters((prev) => {
        const next = new Set(prev[key] as Set<string>);
        if (next.has(value as string)) {
          next.delete(value as string);
        } else {
          next.add(value as string);
        }
        return { ...prev, [key]: next };
      });
    },
    [],
  );

  const setSearch = useCallback((search: string) => {
    setFilters((prev) => ({ ...prev, search }));
  }, []);

  const toggleBoolean = useCallback(
    (key: "supportsTranslation" | "supportsStreaming") => {
      setFilters((prev) => {
        if (key === "supportsStreaming") {
          return {
            ...prev,
            supportsStreaming: prev.supportsStreaming === true ? null : true,
          };
        }

        // Cycle: null -> true -> false -> null
        const current = prev[key];
        const next = current === null ? true : current === true ? false : null;
        return { ...prev, [key]: next };
      });
    },
    [],
  );

  const toggleRecommended = useCallback(() => {
    setFilters((prev) => ({ ...prev, recommendedOnly: !prev.recommendedOnly }));
  }, []);

  return {
    filters,
    isAnyFilterActive,
    applyFilters,
    resetFilters,
    setSearch,
    toggleSetValue,
    toggleBoolean,
    toggleRecommended,
  };
}

export { SIZE_RANGES, engineFilterKey };
export type { SizeRangeKey as SizeRangeKeyType };
