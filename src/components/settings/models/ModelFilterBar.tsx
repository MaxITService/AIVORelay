import React, { useMemo, useRef, useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Search, X, ChevronDown, RotateCcw } from "lucide-react";
import type { ModelInfo } from "@/bindings";
import { Input } from "../../ui/Input";
import { Button } from "../../ui/Button";
import { getLocalizedLanguageLabel } from "./ModelMetadataPanel";
import {
  SIZE_RANGES,
  engineFilterKey,
  type EngineFilterKey,
  type SizeRangeKey,
  type ModelFilters,
} from "../../../hooks/useModelFilters";

// Engine chip labels (display name -> filter key)
const ENGINE_CHIPS: { key: EngineFilterKey; label: string }[] = [
  { key: "Whisper", label: "GGML" },
  { key: "TranscribeCpp", label: "GGUF" },
  { key: "Onnx", label: "ONNX" },
  { key: "MoonshineStreaming", label: "Moonshine V2" },
];

// Size chip labels
const SIZE_CHIPS: { key: SizeRangeKey; label: string }[] = [
  { key: "<100", label: "< 100 MB" },
  { key: "100-500", label: "100–500 MB" },
  { key: "500-1000", label: "500 MB–1 GB" },
  { key: ">1000", label: "> 1 GB" },
];

interface ModelFilterBarProps {
  /** All local models (unfiltered union) for computing chip counts. */
  allLocalModels: ModelInfo[];
  filters: ModelFilters;
  isAnyFilterActive: boolean;
  onSearch: (value: string) => void;
  onToggleSet: <K extends "engines" | "sizeRanges" | "languages">(
    key: K,
    value: ModelFilters[K] extends Set<infer V> ? V : never,
  ) => void;
  onToggleBoolean: (key: "supportsTranslation" | "supportsStreaming") => void;
  onToggleRecommended: () => void;
  onReleasedAfterChange: (value: string) => void;
  onReset: () => void;
  filterBarRef?: React.Ref<HTMLDivElement>;
}

// Debounce helper
function useDebouncedCallback(callback: (value: string) => void, delay: number) {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const callbackRef = useRef(callback);
  callbackRef.current = callback;

  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  return useCallback(
    (value: string) => {
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => callbackRef.current(value), delay);
    },
    [delay],
  );
}

// Toggle chip component
const FilterChip: React.FC<{
  label: string;
  count?: number;
  active: boolean;
  triState?: boolean; // true=green, false=red, null=inactive
  triValue?: boolean | null;
  onClick: () => void;
}> = ({ label, count, active, triState, triValue, onClick }) => {
  let colorClasses: string;

  if (triState) {
    if (triValue === true) {
      colorClasses = "border-emerald-500/50 bg-emerald-500/15 text-emerald-300";
    } else if (triValue === false) {
      colorClasses = "border-red-400/50 bg-red-400/15 text-red-300";
    } else {
      colorClasses =
        "border-[#3d3d3d] bg-[#1b1b1b] text-[#9a9a9a] hover:border-[#555] hover:text-[#cfcfcf]";
    }
  } else {
    colorClasses = active
      ? "border-[#ff4d8d]/50 bg-[#ff4d8d]/15 text-[#ff8ebb]"
      : "border-[#3d3d3d] bg-[#1b1b1b] text-[#9a9a9a] hover:border-[#555] hover:text-[#cfcfcf]";
  }

  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-full border px-2.5 py-1 text-[11px] font-medium transition-all duration-150 cursor-pointer select-none ${colorClasses}`}
    >
      {label}
      {count !== undefined && (
        <span className="ml-1 opacity-60">({count})</span>
      )}
    </button>
  );
};

// Language dropdown component
const LanguageDropdown: React.FC<{
  allLanguages: { code: string; label: string }[];
  selected: Set<string>;
  onToggle: (code: string) => void;
}> = ({ allLanguages, selected, onToggle }) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [langSearch, setLangSearch] = useState("");
  const dropdownRef = useRef<HTMLDivElement>(null);

  // Close on outside click
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  const filtered = useMemo(() => {
    if (!langSearch) return allLanguages;
    const lower = langSearch.toLowerCase();
    return allLanguages.filter(
      (l) => l.label.toLowerCase().includes(lower) || l.code.toLowerCase().includes(lower),
    );
  }, [allLanguages, langSearch]);

  return (
    <div ref={dropdownRef} className="relative">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className={`flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] font-medium transition-all duration-150 cursor-pointer select-none ${
          selected.size > 0
            ? "border-[#ff4d8d]/50 bg-[#ff4d8d]/15 text-[#ff8ebb]"
            : "border-[#3d3d3d] bg-[#1b1b1b] text-[#9a9a9a] hover:border-[#555] hover:text-[#cfcfcf]"
        }`}
      >
        {t("modelSelector.filter.languages", "Languages")}
        {selected.size > 0 && (
          <span className="rounded-full bg-[#ff4d8d]/30 px-1.5 text-[10px]">
            {selected.size}
          </span>
        )}
        <ChevronDown
          className={`h-3 w-3 transition-transform ${open ? "rotate-180" : ""}`}
        />
      </button>

      {open && (
        <div className="absolute left-0 top-full z-50 mt-1.5 w-64 rounded-lg border border-[#3d3d3d] bg-[#1a1a1a] shadow-xl shadow-black/50 overflow-hidden">
          {/* Search within languages */}
          <div className="p-2 border-b border-[#2d2d2d]">
            <input
              type="text"
              value={langSearch}
              onChange={(e) => setLangSearch(e.target.value)}
              placeholder={t("modelSelector.filter.searchLanguages", "Search languages...")}
              className="w-full rounded-md border border-[#3c3c3c] bg-[#141414] px-2.5 py-1.5 text-xs text-[#e8e8e8] placeholder-[#6b6b6b] outline-none focus:border-[#ff4d8d]"
              autoFocus
            />
          </div>

          {/* Language list */}
          <div className="max-h-48 overflow-y-auto overscroll-contain">
            {filtered.length === 0 && (
              <p className="px-3 py-2 text-xs text-[#6b6b6b]">
                {t("modelSelector.filter.noLanguagesFound", "No languages found")}
              </p>
            )}
            {filtered.map((lang) => (
              <button
                key={lang.code}
                type="button"
                onClick={() => onToggle(lang.code)}
                className={`flex w-full items-center gap-2 px-3 py-1.5 text-xs transition-colors cursor-pointer ${
                  selected.has(lang.code)
                    ? "bg-[#ff4d8d]/10 text-[#ff8ebb]"
                    : "text-[#cfcfcf] hover:bg-white/5"
                }`}
              >
                <span
                  className={`flex h-3.5 w-3.5 shrink-0 items-center justify-center rounded border text-[9px] ${
                    selected.has(lang.code)
                      ? "border-[#ff4d8d] bg-[#ff4d8d]/20 text-[#ff4d8d]"
                      : "border-[#4a4a4a] bg-transparent"
                  }`}
                >
                  {selected.has(lang.code) && "✓"}
                </span>
                <span className="truncate">{lang.label}</span>
                <span className="ml-auto text-[10px] text-[#6b6b6b]">{lang.code}</span>
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};

export const ModelFilterBar: React.FC<ModelFilterBarProps> = ({
  allLocalModels,
  filters,
  isAnyFilterActive,
  onSearch,
  onToggleSet,
  onToggleBoolean,
  onToggleRecommended,
  onReleasedAfterChange,
  onReset,
  filterBarRef,
}) => {
  const { t, i18n } = useTranslation();
  const [localSearch, setLocalSearch] = useState(filters.search);
  const debouncedSearch = useDebouncedCallback(onSearch, 200);

  // Sync local search state when filters are reset externally
  useEffect(() => {
    if (filters.search === "" && localSearch !== "") {
      setLocalSearch("");
    }
  }, [filters.search]); // eslint-disable-line react-hooks/exhaustive-deps

  const handleSearchChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    setLocalSearch(value);
    debouncedSearch(value);
  };

  // Compute chip counts from all local models
  const engineCounts = useMemo(() => {
    const counts = new Map<EngineFilterKey, number>();
    for (const m of allLocalModels) {
      const key = engineFilterKey(m.engine_type);
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    return counts;
  }, [allLocalModels]);

  const sizeCounts = useMemo(() => {
    const counts = new Map<SizeRangeKey, number>();
    for (const m of allLocalModels) {
      const mb = Number(m.size_mb);
      for (const [key, range] of Object.entries(SIZE_RANGES) as [SizeRangeKey, { min: number; max: number }][]) {
        if (mb >= range.min && mb < range.max) {
          counts.set(key, (counts.get(key) ?? 0) + 1);
        }
      }
    }
    return counts;
  }, [allLocalModels]);

  // Collect all distinct languages across all models
  const allLanguages = useMemo(() => {
    const langSet = new Set<string>();
    for (const m of allLocalModels) {
      for (const code of m.supported_languages) {
        langSet.add(code);
      }
    }
    return Array.from(langSet)
      .map((code) => ({
        code,
        label: getLocalizedLanguageLabel(code, i18n.language),
      }))
      .sort((a, b) => a.label.localeCompare(b.label));
  }, [allLocalModels, i18n.language]);

  const translationLabel = (() => {
    const base = t("modelSelector.filter.translation", "Translation");
    if (filters.supportsTranslation === true) return `${base}: ✓`;
    if (filters.supportsTranslation === false) return `${base}: ✗`;
    return base;
  })();

  const nativeStreamingLabel = t(
    "modelSelector.filter.nativeStreaming",
    "⚡ Native streaming",
  );

  const nativeStreamingActive = filters.supportsStreaming === true;

  const handleNativeStreamingToggle = () => {
    onToggleBoolean("supportsStreaming");
  };

  return (
    <div
      ref={filterBarRef}
      className={`glass-panel-subtle border rounded-xl p-4 space-y-3 transition-all duration-500 ${
        isAnyFilterActive
          ? "model-filter-panel-active border-emerald-500/60"
          : "border-[#3d3d3d]"
      }`}
    >
      {/* Search */}
      <div className="relative">
        <Search className="absolute left-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[#6b6b6b] pointer-events-none" />
        <Input
          variant="compact"
          value={localSearch}
          onChange={handleSearchChange}
          placeholder={t("modelSelector.filter.searchPlaceholder", "Search models...")}
          className="w-full pl-8 pr-8"
        />
        {localSearch && (
          <button
            type="button"
            onClick={() => {
              setLocalSearch("");
              onSearch("");
            }}
            className="absolute right-2.5 top-1/2 -translate-y-1/2 text-[#6b6b6b] hover:text-[#cfcfcf] transition-colors cursor-pointer"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        )}
      </div>

      {/* Filter chips */}
      <div className="flex flex-wrap items-center gap-2">
        {/* Engine chips */}
        {ENGINE_CHIPS.map((chip) => {
          const count = engineCounts.get(chip.key) ?? 0;
          if (count === 0) return null;
          return (
            <FilterChip
              key={chip.key}
              label={chip.label}
              count={count}
              active={filters.engines.has(chip.key)}
              onClick={() => onToggleSet("engines", chip.key)}
            />
          );
        })}

        {/* Separator */}
        <span className="mx-0.5 h-4 w-px bg-[#3d3d3d]" />

        {/* Size chips */}
        {SIZE_CHIPS.map((chip) => {
          const count = sizeCounts.get(chip.key) ?? 0;
          if (count === 0) return null;
          return (
            <FilterChip
              key={chip.key}
              label={chip.label}
              count={count}
              active={filters.sizeRanges.has(chip.key)}
              onClick={() => onToggleSet("sizeRanges", chip.key)}
            />
          );
        })}

        {/* Separator */}
        <span className="mx-0.5 h-4 w-px bg-[#3d3d3d]" />

        {/* Capability chips */}
        <FilterChip
          label={translationLabel}
          active={filters.supportsTranslation !== null}
          triState
          triValue={filters.supportsTranslation}
          onClick={() => onToggleBoolean("supportsTranslation")}
        />
        <FilterChip
          label={nativeStreamingLabel}
          active={nativeStreamingActive}
          onClick={handleNativeStreamingToggle}
        />
        <FilterChip
          label={t("modelSelector.filter.recommended", "Recommended")}
          active={filters.recommendedOnly}
          onClick={onToggleRecommended}
        />

        {/* Separator */}
        <span className="mx-0.5 h-4 w-px bg-[#3d3d3d]" />

        {/* Language dropdown */}
        <LanguageDropdown
          allLanguages={allLanguages}
          selected={filters.languages}
          onToggle={(code) => onToggleSet("languages", code)}
        />

        {/* Separator */}
        <span className="mx-0.5 h-4 w-px bg-[#3d3d3d]" />

        {/* Release date */}
        <div
          className={`flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] transition-colors ${
            filters.releasedAfter
              ? "border-[#ff4d8d]/50 bg-[#ff4d8d]/15 text-[#ff8ebb]"
              : "border-[#3d3d3d] bg-[#1b1b1b] text-[#9a9a9a]"
          }`}
        >
          <span>{t("modelSelector.filter.releasedAfter", "Released after")}</span>
          <input
            type="date"
            value={filters.releasedAfter}
            onChange={(event) => onReleasedAfterChange(event.target.value)}
            aria-label={t(
              "modelSelector.filter.releasedAfter",
              "Released after",
            )}
            className="w-[7.4rem] bg-transparent text-[10px] text-current outline-none [color-scheme:dark]"
          />
          {filters.releasedAfter && (
            <button
              type="button"
              onClick={() => onReleasedAfterChange("")}
              aria-label={t("common.clear", "Clear")}
              className="rounded-full p-0.5 hover:bg-white/10"
            >
              <X className="h-3 w-3" />
            </button>
          )}
        </div>

        {/* Reset */}
        {isAnyFilterActive && (
          <Button
            variant="ghost"
            size="sm"
            onClick={onReset}
            className="ml-auto flex items-center gap-1.5 !px-2.5 !py-1 !text-[11px]"
          >
            <RotateCcw className="h-3 w-3" />
            {t("modelSelector.filter.reset", "Reset")}
          </Button>
        )}
      </div>
    </div>
  );
};
