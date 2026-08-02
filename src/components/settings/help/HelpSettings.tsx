import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Search, Sparkles } from "lucide-react";
import { type } from "@tauri-apps/plugin-os";
import { Button } from "../../ui/Button";
import { useSettings } from "../../../hooks/useSettings";
import { useNavigationStore } from "../../../stores/navigationStore";
import { scrollAndFocusAnchor } from "../../../lib/anchorNavigation";
import {
  HELP_SECTIONS,
  type HelpSectionDefinition,
  type HelpSubsectionDefinition,
} from "./helpContent";
import type { HelpSearchResult } from "./helpSearch";

type HelpEntry = HelpSectionDefinition | HelpSubsectionDefinition;
type SearchStatus = "idle" | "loading" | "ready" | "error";
type HelpSearchModule = typeof import("./helpSearch");

const SMART_HELP_ACTIONS = [
  {
    labelKey: "help.smartHelp.actions.transcription",
    anchor: "help-transcription",
  },
  {
    labelKey: "help.smartHelp.actions.cleanup",
    anchor: "help-post-processing",
  },
  {
    labelKey: "help.smartHelp.actions.readAloud",
    anchor: "help-speak-selected-text",
  },
] as const;

export const HelpSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings } = useSettings();
  const setSection = useNavigationStore((state) => state.setSection);
  const pendingHelpAnchor = useNavigationStore(
    (state) => state.pendingHelpAnchor,
  );
  const consumePendingHelpAnchor = useNavigationStore(
    (state) => state.consumePendingHelpAnchor,
  );
  const isVoiceCommandsEnabled =
    type() === "windows" && Boolean(settings?.beta_voice_commands_enabled);

  const visibleSections = useMemo(
    () =>
      HELP_SECTIONS.filter(
        (section) =>
          section.id !== "voiceCommands" || isVoiceCommandsEnabled,
      ),
    [isVoiceCommandsEnabled],
  );
  const helpEntryByAnchor = useMemo(() => {
    const entries = new Map<string, HelpEntry>();
    for (const section of visibleSections) {
      entries.set(section.anchor, section);
      for (const subsection of section.subsections ?? []) {
        entries.set(subsection.anchor, subsection);
      }
    }
    return entries;
  }, [visibleSections]);

  const scrollToHelpAnchor = useCallback(
    (anchor: string) => {
      const heading = document.getElementById(anchor);
      if (heading) scrollAndFocusAnchor(heading);
    },
    [],
  );

  useEffect(() => {
    if (!pendingHelpAnchor) return;

    const frame = window.requestAnimationFrame(() => {
      scrollToHelpAnchor(pendingHelpAnchor);
      consumePendingHelpAnchor();
    });

    return () => window.cancelAnimationFrame(frame);
  }, [consumePendingHelpAnchor, pendingHelpAnchor, scrollToHelpAnchor]);

  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const latestQueryRef = useRef("");
  const searchModulePromiseRef = useRef<Promise<HelpSearchModule> | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchStatus, setSearchStatus] = useState<SearchStatus>("idle");
  const [searchResults, setSearchResults] = useState<HelpSearchResult[]>([]);
  const [searchResultsOpen, setSearchResultsOpen] = useState(false);
  const [activeResultIndex, setActiveResultIndex] = useState(-1);

  const ensureSearchLoaded = useCallback(() => {
    if (searchModulePromiseRef.current) {
      return searchModulePromiseRef.current;
    }

    setSearchStatus("loading");
    const promise = import("./helpSearch")
      .then((module) => {
        setSearchStatus("ready");
        return module;
      })
      .catch((error) => {
        console.error("Failed to load Help search:", error);
        searchModulePromiseRef.current = null;
        setSearchStatus("error");
        throw error;
      });

    searchModulePromiseRef.current = promise;
    return promise;
  }, []);

  const handleSearchFocus = useCallback(() => {
    setSearchResultsOpen(true);
    void ensureSearchLoaded().catch(() => undefined);
  }, [ensureSearchLoaded]);

  const handleSearchChange = useCallback(
    (event: React.ChangeEvent<HTMLInputElement>) => {
      const nextQuery = event.target.value;
      latestQueryRef.current = nextQuery;
      setSearchQuery(nextQuery);
      setSearchResultsOpen(true);
      setActiveResultIndex(-1);

      if (!nextQuery.trim()) {
        setSearchResults([]);
        if (!searchModulePromiseRef.current) setSearchStatus("idle");
        return;
      }

      void ensureSearchLoaded()
        .then((module) => {
          if (latestQueryRef.current !== nextQuery) return;
          const nextResults = module
            .searchHelp(nextQuery, {
              includeVoiceCommands: isVoiceCommandsEnabled,
            })
            .filter((result) => helpEntryByAnchor.has(result.anchor));
          setSearchResults(nextResults);
        })
        .catch(() => {
          if (latestQueryRef.current === nextQuery) {
            setSearchResults([]);
          }
        });
    },
    [ensureSearchLoaded, helpEntryByAnchor, isVoiceCommandsEnabled],
  );

  const chooseSearchResult = useCallback(
    (anchor: string) => {
      setSearchResultsOpen(false);
      setActiveResultIndex(-1);
      scrollToHelpAnchor(anchor);
    },
    [scrollToHelpAnchor],
  );

  const handleSearchKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLInputElement>) => {
      if (event.key === "Escape") {
        event.preventDefault();
        if (searchQuery.trim()) {
          latestQueryRef.current = "";
          setSearchQuery("");
          setSearchResults([]);
          setActiveResultIndex(-1);
        }
        setSearchResultsOpen(false);
        return;
      }

      if (!searchResults.length) return;

      if (event.key === "ArrowDown") {
        event.preventDefault();
        setSearchResultsOpen(true);
        setActiveResultIndex(
          (current) => (current + 1) % searchResults.length,
        );
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        setSearchResultsOpen(true);
        setActiveResultIndex((current) =>
          current <= 0 ? searchResults.length - 1 : current - 1,
        );
      } else if (event.key === "Enter") {
        const resultIndex =
          activeResultIndex >= 0
            ? activeResultIndex
            : searchResults.length === 1
              ? 0
              : -1;
        if (resultIndex >= 0) {
          event.preventDefault();
          chooseSearchResult(searchResults[resultIndex].anchor);
        }
      }
    },
    [activeResultIndex, chooseSearchResult, searchQuery, searchResults],
  );

  const renderDestinationButton = (entry: HelpEntry) => (
    <Button
      type="button"
      variant="secondary"
      size="sm"
      onClick={() => setSection(entry.destination)}
      className="shrink-0 whitespace-nowrap"
    >
      {t(entry.destinationLabelKey)}
    </Button>
  );

  return (
    <div className="w-full space-y-7 pb-12">
      <header className="space-y-2">
        <h1 className="text-2xl font-semibold tracking-tight text-[#f5f5f5]">
          {t("help.title")}
        </h1>
        <p className="text-sm leading-relaxed text-[#b8b8b8]">
          {t("help.definition")}
        </p>
        <p className="text-sm leading-relaxed text-[#a0a0a0]">
          {t("help.intro")}
        </p>
      </header>

      <section aria-labelledby="help-search-title" className="space-y-2">
        <label
          id="help-search-title"
          htmlFor="help-search"
          className="text-xs font-semibold uppercase tracking-[0.12em] text-[#ff8ebb]"
        >
          {t("help.search.label")}
        </label>
        <div className="relative">
          <Search
            aria-hidden="true"
            className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-[#707070]"
          />
          <input
            ref={searchInputRef}
            id="help-search"
            type="search"
            value={searchQuery}
            placeholder={t("help.search.placeholder")}
            autoComplete="off"
            aria-label={t("help.search.label")}
            aria-controls="help-search-results"
            aria-expanded={searchResultsOpen}
            aria-activedescendant={
              activeResultIndex >= 0
                ? `help-search-result-${searchResults[activeResultIndex]?.anchor}`
                : undefined
            }
            onFocus={handleSearchFocus}
            onChange={handleSearchChange}
            onKeyDown={handleSearchKeyDown}
            className="w-full rounded-lg border border-[#333333] bg-[#141414] py-2.5 pl-9 pr-3 text-sm text-[#f5f5f5] outline-none placeholder:text-[#707070] focus:border-[#ff4d8d] focus:ring-2 focus:ring-[#ff4d8d]/25"
          />
        </div>
        {searchResultsOpen && (
          <div
            id="help-search-results"
            className="rounded-lg border border-[#333333] bg-[#1a1a1a] p-2"
            aria-live="polite"
          >
            {searchStatus === "loading" && (
              <p className="px-2 py-1 text-xs text-[#a0a0a0]">
                {t("help.search.loading")}
              </p>
            )}
            {searchStatus === "error" && (
              <p className="px-2 py-1 text-xs text-[#ffb3c9]">
                {t("help.search.error")}
              </p>
            )}
            {searchStatus === "ready" && !searchQuery.trim() && (
              <p className="px-2 py-1 text-xs text-[#a0a0a0]">
                {t("help.search.emptyQuery")}
              </p>
            )}
            {searchStatus === "ready" &&
              searchQuery.trim() &&
              searchResults.length === 0 && (
                <p className="px-2 py-1 text-xs text-[#a0a0a0]">
                  {t("help.search.noResults")}
                </p>
              )}
            {searchStatus === "ready" && searchResults.length > 0 && (
              <ul role="listbox" className="space-y-1">
                {searchResults.map((result, index) => {
                  const entry = helpEntryByAnchor.get(result.anchor);
                  if (!entry) return null;
                  const title = t(entry.titleKey);
                  const destination = t(entry.destinationLabelKey);
                  return (
                    <li key={result.anchor}>
                      <button
                        id={`help-search-result-${result.anchor}`}
                        type="button"
                        role="option"
                        aria-selected={activeResultIndex === index}
                        aria-label={`${title}. ${destination}`}
                        onClick={() => chooseSearchResult(result.anchor)}
                        className={`flex w-full items-start justify-between gap-3 rounded-md px-2 py-2 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#ff4d8d]/60 ${
                          activeResultIndex === index
                            ? "bg-[#ff4d8d]/12"
                            : "hover:bg-white/[0.04]"
                        }`}
                      >
                        <span className="min-w-0">
                          <span className="block text-sm text-[#f5f5f5]">
                            {title}
                          </span>
                          <span className="mt-0.5 block text-xs text-[#a0a0a0]">
                            {destination}
                          </span>
                        </span>
                        <span
                          aria-hidden="true"
                          className="shrink-0 pt-0.5 text-xs text-[#ff8ebb]"
                        >
                          {index + 1}
                        </span>
                      </button>
                    </li>
                  );
                })}
              </ul>
            )}
          </div>
        )}
      </section>

      <section
        aria-labelledby="help-smart-title"
        className="rounded-xl border border-[#ff4d8d]/25 bg-[#1a1a1a] p-4"
      >
        <div className="flex items-start gap-3">
          <Sparkles
            aria-hidden="true"
            className="mt-0.5 h-4 w-4 shrink-0 text-[#ff8ebb]"
          />
          <div className="min-w-0">
            <p className="text-xs font-semibold uppercase tracking-[0.12em] text-[#ff8ebb]">
              {t("help.smartHelp.label")}
            </p>
            <h2
              id="help-smart-title"
              className="mt-1 text-sm font-semibold text-[#f5f5f5]"
            >
              {t("help.smartHelp.title")}
            </h2>
            <p className="mt-1 text-xs leading-relaxed text-[#b8b8b8]">
              {t("help.smartHelp.description")}
            </p>
          </div>
        </div>
        <div className="mt-3 flex flex-wrap gap-2 pl-7">
          {SMART_HELP_ACTIONS.map((action) => (
            <Button
              key={action.anchor}
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => scrollToHelpAnchor(action.anchor)}
              className="whitespace-nowrap"
            >
              {t(action.labelKey)}
            </Button>
          ))}
        </div>
      </section>

      <nav
        aria-labelledby="help-contents-title"
        className="rounded-xl border border-[#333333] bg-[#171717] p-4"
      >
        <h2
          id="help-contents-title"
          className="text-xs font-semibold uppercase tracking-[0.12em] text-[#ff8ebb]"
        >
          {t("help.contents.title")}
        </h2>
        <ol className="mt-3 list-decimal space-y-1.5 pl-5 text-sm text-[#d8d8d8] marker:text-[#ff8ebb]">
          {visibleSections.map((section) => (
            <li key={section.id} className="pl-1">
              <a
                href={`#${section.anchor}`}
                onClick={(event) => {
                  event.preventDefault();
                  scrollToHelpAnchor(section.anchor);
                }}
                className="rounded-sm underline decoration-[#707070] underline-offset-4 hover:text-[#ffb3c9] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#ff4d8d]/60"
              >
                {t(section.titleKey)}
              </a>
              {section.subsections && (
                <ol className="mt-1.5 list-[lower-alpha] space-y-1 pl-5 text-xs text-[#a0a0a0] marker:text-[#707070]">
                  {section.subsections.map((subsection) => (
                    <li key={subsection.id} className="pl-1">
                      <a
                        href={`#${subsection.anchor}`}
                        onClick={(event) => {
                          event.preventDefault();
                          scrollToHelpAnchor(subsection.anchor);
                        }}
                        className="rounded-sm underline decoration-[#555555] underline-offset-4 hover:text-[#ffb3c9] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#ff4d8d]/60"
                      >
                        {t(subsection.titleKey)}
                      </a>
                    </li>
                  ))}
                </ol>
              )}
            </li>
          ))}
        </ol>
      </nav>

      <div className="space-y-6">
        {visibleSections.map((section) => (
          <section
            key={section.id}
            aria-labelledby={section.anchor}
            className="border-t border-[#333333] pt-5 first:border-t-0 first:pt-0"
          >
            <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
              <div className="min-w-0">
                <h2
                  id={section.anchor}
                  tabIndex={-1}
                  data-help-heading="true"
                  className="scroll-mt-6 rounded-sm text-base font-semibold leading-snug text-[#f5f5f5] outline-none focus:ring-2 focus:ring-[#ff4d8d]/60 focus:ring-offset-2 focus:ring-offset-[#121212]"
                >
                  {t(section.titleKey)}
                </h2>
                <p className="mt-1.5 text-sm leading-relaxed text-[#b8b8b8]">
                  {t(section.summaryKey)}
                </p>
              </div>
              {renderDestinationButton(section)}
            </div>

            {section.subsections && (
              <div className="mt-5 space-y-4 border-l border-[#ff4d8d]/30 pl-4 sm:ml-2">
                {section.subsections.map((subsection) => (
                  <div key={subsection.id}>
                    <h3
                      id={subsection.anchor}
                      tabIndex={-1}
                      data-help-heading="true"
                      className="scroll-mt-6 rounded-sm text-sm font-semibold text-[#f5f5f5] outline-none focus:ring-2 focus:ring-[#ff4d8d]/60 focus:ring-offset-2 focus:ring-offset-[#121212]"
                    >
                      {t(subsection.titleKey)}
                    </h3>
                    <div className="mt-1.5 flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between">
                      <p className="text-xs leading-relaxed text-[#a0a0a0]">
                        {t(subsection.summaryKey)}
                      </p>
                      {renderDestinationButton(subsection)}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </section>
        ))}
      </div>

      <div className="border-t border-[#333333] pt-4 text-xs text-[#707070]">
        <span>{t("help.aboutLinkPrefix")} </span>
        <a
          href="#about"
          onClick={(event) => {
            event.preventDefault();
            setSection("about");
          }}
          className="rounded-sm text-[#a0a0a0] underline decoration-[#707070] underline-offset-4 hover:text-[#f5f5f5] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#ff4d8d]/60"
        >
          {t("sidebar.about")}
        </a>
      </div>
    </div>
  );
};
