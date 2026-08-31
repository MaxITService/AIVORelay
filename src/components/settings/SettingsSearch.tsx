import React, { useEffect, useMemo, useRef, useState } from "react";
import { Search, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { AppSettings } from "@/bindings";
import type { SettingsSearchEntry } from "./settingsSearchTypes";

interface SettingsSearchProps {
  entries: readonly SettingsSearchEntry[];
  settings: AppSettings | null;
  availableSections: readonly string[];
  sectionLabelKey: (section: string) => string | null;
  onNavigate: (
    section: string,
    anchor?: string,
    expandAnchor?: string,
  ) => void;
  onSearchHelp: (query: string) => void;
}

const MAX_SETTINGS_SEARCH_RESULTS = 20;

const normalizeSearchFragment = (value: string): string =>
  value
    .toLocaleLowerCase()
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "");

const normalizeSearchText = (value: string): string =>
  normalizeSearchFragment(value).trim();

const findNormalizedMatchRange = (
  text: string,
  query: string,
): [number, number] | null => {
  const normalizedQuery = normalizeSearchText(query);
  if (!normalizedQuery) return null;

  const sourceRanges: Array<[number, number]> = [];
  let normalizedText = "";
  let sourceOffset = 0;

  for (const character of text) {
    const characterEnd = sourceOffset + character.length;
    const normalizedCharacter = normalizeSearchFragment(character);
    normalizedText += normalizedCharacter;
    for (let index = 0; index < normalizedCharacter.length; index += 1) {
      sourceRanges.push([sourceOffset, characterEnd]);
    }
    sourceOffset = characterEnd;
  }

  const normalizedStart = normalizedText.indexOf(normalizedQuery);
  if (normalizedStart < 0) return null;

  const firstRange = sourceRanges[normalizedStart];
  const lastRange = sourceRanges[normalizedStart + normalizedQuery.length - 1];
  return firstRange && lastRange ? [firstRange[0], lastRange[1]] : null;
};

const HighlightMatch: React.FC<{ text: string; query: string }> = ({
  text,
  query,
}) => {
  const matchRange = findNormalizedMatchRange(text, query);
  if (!matchRange) return <>{text}</>;

  const [matchStart, matchEnd] = matchRange;
  return (
    <>
      {text.slice(0, matchStart)}
      <mark className="rounded-sm bg-[#ff4d8d]/25 px-0.5 text-inherit">
        {text.slice(matchStart, matchEnd)}
      </mark>
      {text.slice(matchEnd)}
    </>
  );
};

export const SettingsSearch: React.FC<SettingsSearchProps> = ({
  entries,
  settings,
  availableSections,
  sectionLabelKey,
  onNavigate,
  onSearchHelp,
}) => {
  const { t } = useTranslation();
  const rootRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [isFocused, setIsFocused] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const normalizedQuery = normalizeSearchText(query);

  const results = useMemo(() => {
    if (!normalizedQuery) return [];

    const available = new Set(availableSections);
    const matches = entries
      .map((entry) => {
        const label = t(entry.labelKey, entry.fallbackLabel);
        const sectionKey = sectionLabelKey(entry.section);
        const sectionLabel = sectionKey ? t(sectionKey) : entry.section;
        const groupLabel = entry.groupLabelKey
          ? t(entry.groupLabelKey, entry.groupFallbackLabel ?? "")
          : (entry.groupFallbackLabel ?? "");
        const normalizedLabel = normalizeSearchText(label);
        const normalizedSection = normalizeSearchText(sectionLabel);
        const normalizedGroup = normalizeSearchText(groupLabel);
        const matchedKeyword = entry.keywords.find((keyword) =>
          normalizeSearchText(keyword).includes(normalizedQuery),
        );
        const score = normalizedLabel.startsWith(normalizedQuery)
          ? 0
          : normalizedLabel.includes(normalizedQuery)
            ? 1
            : normalizedSection.includes(normalizedQuery) ||
                normalizedGroup.includes(normalizedQuery)
              ? 2
              : matchedKeyword
                ? 3
                : Number.POSITIVE_INFINITY;
        const isAvailable =
          available.has(entry.section) &&
          (entry.isAvailable?.(settings) ?? true);
        const unavailableReason = isAvailable
          ? null
          : entry.unavailableReasonKey
            ? t(
                entry.unavailableReasonKey,
                entry.unavailableReasonFallback ?? "",
              )
            : (entry.unavailableReasonFallback ??
              t(
                "settingsSearch.unavailable.default",
                "This section is currently unavailable.",
              ));

        return {
          entry,
          label,
          sectionLabel,
          groupLabel,
          matchedKeyword,
          score,
          isAvailable,
          unavailableReason,
        };
      })
      .filter((result) => Number.isFinite(result.score));
    const sectionScores = new Map<string, number>();
    for (const result of matches) {
      sectionScores.set(
        result.entry.section,
        Math.min(
          sectionScores.get(result.entry.section) ?? Number.POSITIVE_INFINITY,
          result.score,
        ),
      );
    }

    return matches
      .sort((left, right) => {
        const sectionScore =
          (sectionScores.get(left.entry.section) ?? left.score) -
          (sectionScores.get(right.entry.section) ?? right.score);
        if (sectionScore !== 0) return sectionScore;
        if (left.isAvailable !== right.isAvailable) {
          return left.isAvailable ? -1 : 1;
        }
        const sectionOrder = left.sectionLabel.localeCompare(
          right.sectionLabel,
        );
        if (sectionOrder !== 0) return sectionOrder;
        if (left.score !== right.score) return left.score - right.score;
        return left.label.localeCompare(right.label);
      })
      .slice(0, MAX_SETTINGS_SEARCH_RESULTS);
  }, [
    availableSections,
    entries,
    normalizedQuery,
    sectionLabelKey,
    settings,
    t,
  ]);

  const showResults = isFocused && normalizedQuery.length > 0;

  useEffect(() => {
    setActiveIndex(0);
  }, [normalizedQuery]);

  useEffect(() => {
    if (!showResults || !results[activeIndex]) return;

    document
      .getElementById(
        `global-settings-search-result-${results[activeIndex].entry.id}`,
      )
      ?.scrollIntoView({ block: "nearest" });
  }, [activeIndex, results, showResults]);

  useEffect(() => {
    const focusSearch = (event: KeyboardEvent) => {
      const key = event.key.toLocaleLowerCase();
      const isModifiedShortcut =
        !event.altKey &&
        (event.ctrlKey || event.metaKey) &&
        (key === "k" || key === "f");
      const target = event.target;
      const isEditableTarget =
        target instanceof HTMLElement &&
        (target.isContentEditable ||
          /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName));
      const isSlashShortcut =
        event.key === "/" &&
        !event.ctrlKey &&
        !event.metaKey &&
        !event.altKey &&
        !isEditableTarget;
      if (
        event.defaultPrevented ||
        event.isComposing ||
        (!isModifiedShortcut && !isSlashShortcut)
      ) {
        return;
      }

      event.preventDefault();
      inputRef.current?.focus();
      setIsFocused(true);
    };

    window.addEventListener("keydown", focusSearch);
    return () => window.removeEventListener("keydown", focusSearch);
  }, []);

  const selectEntry = (entry: SettingsSearchEntry) => {
    if (
      !availableSections.includes(entry.section) ||
      entry.isAvailable?.(settings) === false
    ) {
      return;
    }
    onNavigate(entry.section, entry.anchor, entry.expandAnchor);
    setQuery("");
    setIsFocused(false);
  };

  const searchHelp = () => {
    const helpQuery = query.trim();
    if (!helpQuery) return;
    setQuery("");
    setIsFocused(false);
    onSearchHelp(helpQuery);
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
          ref={inputRef}
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
          aria-autocomplete="list"
          aria-haspopup="listbox"
          aria-expanded={showResults}
          aria-controls="global-settings-search-results"
          aria-keyshortcuts="Control+F Meta+F Control+K Meta+K /"
          aria-activedescendant={
            showResults && results[activeIndex]
              ? `global-settings-search-result-${results[activeIndex].entry.id}`
              : undefined
          }
        />
        {!query && (
          <kbd className="hidden rounded border border-[#3a3a3a] px-1 text-[9px] text-[#777777] sm:inline">
            Ctrl F
          </kbd>
        )}
        {query && (
          <button
            type="button"
            onClick={() => setQuery("")}
            aria-label={t("settingsSearch.clear", "Clear search")}
            className="rounded p-0.5 text-[#777777] hover:bg-white/[0.06] hover:text-[#d0d0d0] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#ff4d8d]/60"
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
          aria-label={t("settingsSearch.results", "Settings search results")}
          className="absolute left-0 top-full mt-1 max-h-96 w-96 max-w-[calc(100vw-2rem)] overflow-y-auto rounded-lg border border-[#3b3b3b] bg-[#171717] p-1 shadow-2xl"
        >
          {results.length > 0 ? (
            results.map((result, index) => {
              const {
                entry,
                label,
                sectionLabel,
                groupLabel,
                matchedKeyword,
                isAvailable,
                unavailableReason,
              } = result;
              const startsSection =
                index === 0 ||
                results[index - 1].entry.section !== entry.section;

              return (
                <React.Fragment key={entry.id}>
                  {startsSection && (
                    <div
                      role="presentation"
                      className="px-3 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-wide text-[#707070]"
                    >
                      <HighlightMatch text={sectionLabel} query={query} />
                    </div>
                  )}
                  <button
                    id={`global-settings-search-result-${entry.id}`}
                    type="button"
                    role="option"
                    aria-selected={index === activeIndex}
                    aria-disabled={!isAvailable}
                    tabIndex={-1}
                    onMouseEnter={() => setActiveIndex(index)}
                    onMouseDown={(event) => event.preventDefault()}
                    onClick={() => selectEntry(entry)}
                    className={`block w-full rounded-md px-3 py-2 text-left focus:outline-none ${
                      index === activeIndex ? "bg-[#252525]" : "hover:bg-[#252525]"
                    } ${isAvailable ? "" : "cursor-not-allowed opacity-65"}`}
                  >
                    <span className="block text-sm text-[#f0f0f0]">
                      <HighlightMatch text={sectionLabel} query={query} />
                      {groupLabel && (
                        <>
                          <span aria-hidden="true" className="px-1.5 text-[#626262]">
                            →
                          </span>
                          <HighlightMatch text={groupLabel} query={query} />
                        </>
                      )}
                      <span aria-hidden="true" className="px-1.5 text-[#626262]">
                        →
                      </span>
                      <HighlightMatch text={label} query={query} />
                    </span>
                    {matchedKeyword &&
                      !normalizeSearchText(label).includes(normalizedQuery) &&
                      !normalizeSearchText(sectionLabel).includes(
                        normalizedQuery,
                      ) &&
                      !normalizeSearchText(groupLabel).includes(
                        normalizedQuery,
                      ) && (
                        <span className="mt-1 block text-[11px] text-[#858585]">
                          {t("settingsSearch.matched", "Matched")}: {" "}
                          <HighlightMatch text={matchedKeyword} query={query} />
                        </span>
                      )}
                    {!isAvailable && unavailableReason && (
                      <span className="mt-1 block text-[11px] text-[#d8a36f]">
                        {unavailableReason}
                      </span>
                    )}
                  </button>
                </React.Fragment>
              );
            })
          ) : (
            <div className="px-3 py-3">
              <p
                role="status"
                aria-live="polite"
                className="text-xs text-[#858585]"
              >
                {t("settingsSearch.noResults", "No matching settings")}
              </p>
              <button
                type="button"
                onMouseDown={(event) => event.preventDefault()}
                onClick={searchHelp}
                className="mt-2 text-left text-xs font-medium text-[#ff8ebb] hover:text-[#ffc0d5] focus-visible:rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#ff4d8d]/60"
              >
                {t("settingsSearch.searchHelp", "Search Help for “{{query}}”", {
                  query: query.trim(),
                })}
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
};
