import React, { useEffect, useMemo, useRef, useState } from "react";
import { Search, X } from "lucide-react";
import { useTranslation } from "react-i18next";

export type SettingsSearchEntry = {
  id: string;
  section: string;
  anchor?: string;
  expandAnchor?: string;
  labelKey: string;
  fallbackLabel: string;
  keywords: readonly string[];
};

interface SettingsSearchProps {
  entries: readonly SettingsSearchEntry[];
  availableSections: readonly string[];
  sectionLabelKey: (section: string) => string | null;
  onNavigate: (
    section: string,
    anchor?: string,
    expandAnchor?: string,
  ) => void;
}

const normalizeSearchText = (value: string): string =>
  value
    .toLocaleLowerCase()
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .trim();

export const SettingsSearch: React.FC<SettingsSearchProps> = ({
  entries,
  availableSections,
  sectionLabelKey,
  onNavigate,
}) => {
  const { t } = useTranslation();
  const rootRef = useRef<HTMLDivElement>(null);
  const [query, setQuery] = useState("");
  const [isFocused, setIsFocused] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const normalizedQuery = normalizeSearchText(query);

  const results = useMemo(() => {
    if (!normalizedQuery) return [];

    const available = new Set(availableSections);
    return entries
      .filter((entry) => available.has(entry.section))
      .map((entry) => {
        const label = t(entry.labelKey, entry.fallbackLabel);
        const sectionKey = sectionLabelKey(entry.section);
        const sectionLabel = sectionKey ? t(sectionKey) : entry.section;
        const normalizedLabel = normalizeSearchText(label);
        const searchable = normalizeSearchText(
          [label, sectionLabel, ...entry.keywords].join(" "),
        );
        const score = normalizedLabel.startsWith(normalizedQuery)
          ? 0
          : normalizedLabel.includes(normalizedQuery)
            ? 1
            : searchable.includes(normalizedQuery)
              ? 2
              : Number.POSITIVE_INFINITY;

        return { entry, label, sectionLabel, score };
      })
      .filter((result) => Number.isFinite(result.score))
      .sort((left, right) =>
        left.score === right.score
          ? left.label.localeCompare(right.label)
          : left.score - right.score,
      )
      .slice(0, 8);
  }, [
    availableSections,
    entries,
    normalizedQuery,
    sectionLabelKey,
    t,
  ]);

  const showResults = isFocused && normalizedQuery.length > 0;

  useEffect(() => {
    setActiveIndex(0);
  }, [normalizedQuery]);

  const selectEntry = (entry: SettingsSearchEntry) => {
    onNavigate(entry.section, entry.anchor, entry.expandAnchor);
    setQuery("");
    setIsFocused(false);
  };

  return (
    <div
      ref={rootRef}
      className="relative z-50 mb-3 w-full shrink-0"
      onBlur={(event) => {
        if (!rootRef.current?.contains(event.relatedTarget as Node | null)) {
          setIsFocused(false);
        }
      }}
    >
      <label className="sr-only" htmlFor="global-settings-search">
        {t("settingsSearch.label", "Search settings")}
      </label>
      <div className="flex items-center gap-2 rounded-lg border border-[#333333] bg-[#141414] px-2.5 focus-within:border-[#ff4d8d]/70 focus-within:ring-1 focus-within:ring-[#ff4d8d]/25">
        <Search className="h-4 w-4 shrink-0 text-[#777777]" aria-hidden="true" />
        <input
          id="global-settings-search"
          type="search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onFocus={() => setIsFocused(true)}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              setQuery("");
              setIsFocused(false);
              event.currentTarget.blur();
            } else if (event.key === "ArrowDown" && results.length > 0) {
              event.preventDefault();
              setActiveIndex((index) => (index + 1) % results.length);
            } else if (event.key === "ArrowUp" && results.length > 0) {
              event.preventDefault();
              setActiveIndex(
                (index) => (index - 1 + results.length) % results.length,
              );
            } else if (event.key === "Enter" && results[activeIndex]) {
              event.preventDefault();
              selectEntry(results[activeIndex].entry);
            }
          }}
          placeholder={t("settingsSearch.placeholder", "Search settings…")}
          autoComplete="off"
          className="min-w-0 flex-1 bg-transparent py-2 text-xs text-[#f5f5f5] outline-none placeholder:text-[#666666] [&::-webkit-search-cancel-button]:hidden"
          role="combobox"
          aria-expanded={showResults}
          aria-controls="global-settings-search-results"
          aria-activedescendant={
            showResults && results[activeIndex]
              ? `global-settings-search-result-${results[activeIndex].entry.id}`
              : undefined
          }
        />
        {query && (
          <button
            type="button"
            onClick={() => setQuery("")}
            className="rounded p-0.5 text-[#777777] hover:bg-white/[0.06] hover:text-[#d0d0d0]"
            title={t("settingsSearch.clear", "Clear search")}
          >
            <X className="h-3.5 w-3.5" aria-hidden="true" />
          </button>
        )}
      </div>

      {showResults && (
        <div
          id="global-settings-search-results"
          role="listbox"
          className="absolute left-0 top-full mt-1 max-h-80 w-80 overflow-y-auto rounded-lg border border-[#3b3b3b] bg-[#171717] p-1 shadow-2xl"
        >
          {results.length > 0 ? (
            results.map(({ entry, label, sectionLabel }, index) => (
              <button
                key={entry.id}
                id={`global-settings-search-result-${entry.id}`}
                type="button"
                role="option"
                aria-selected={index === activeIndex}
                onMouseEnter={() => setActiveIndex(index)}
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => selectEntry(entry)}
                className={`block w-full rounded-md px-3 py-2 text-left focus:outline-none ${
                  index === activeIndex ? "bg-[#252525]" : "hover:bg-[#252525]"
                }`}
              >
                <span className="block text-sm text-[#f0f0f0]">{label}</span>
                <span className="mt-0.5 block text-[11px] text-[#858585]">
                  {sectionLabel}
                </span>
              </button>
            ))
          ) : (
            <p className="px-3 py-3 text-xs text-[#858585]">
              {t("settingsSearch.noResults", "No matching settings")}
            </p>
          )}
        </div>
      )}
    </div>
  );
};
