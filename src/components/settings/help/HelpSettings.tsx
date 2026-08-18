import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  AlertTriangle,
  Check,
  Cloud,
  Copy,
  Laptop,
  Search,
  Settings2,
  Sparkles,
  Volume2,
  Wrench,
} from "lucide-react";
import { Button } from "../../ui/Button";
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

const AIVORELAY_SOURCE_URL = "https://github.com/MaxITService/AIVORelay";

const SMART_HELP_ACTIONS = [
  {
    icon: Settings2,
    labelKey: "help.smartHelp.actions.setupTranscription",
    descriptionKey:
      "help.smartHelp.actions.setupTranscriptionDescription",
    anchor: "help-transcription",
  },
  {
    icon: Laptop,
    labelKey: "help.smartHelp.actions.localModel",
    descriptionKey: "help.smartHelp.actions.localModelDescription",
    anchor: "help-models",
  },
  {
    icon: Cloud,
    labelKey: "help.smartHelp.actions.onlineProvider",
    descriptionKey: "help.smartHelp.actions.onlineProviderDescription",
    anchor: "help-models",
  },
  {
    icon: Volume2,
    labelKey: "help.smartHelp.actions.readAloud",
    descriptionKey: "help.smartHelp.actions.readAloudDescription",
    anchor: "help-speak-selected-text",
  },
  {
    icon: Wrench,
    labelKey: "help.smartHelp.actions.troubleshoot",
    descriptionKey: "help.smartHelp.actions.troubleshootDescription",
    anchor: "help-debug",
  },
] as const;

const WHATS_NEW_ITEMS = [
  "help.whatsNew.items.microphoneRecovery",
  "help.whatsNew.items.customPhrases",
  "help.whatsNew.items.portableModels",
  "help.whatsNew.items.transcriptionAndTts",
  "help.whatsNew.items.secureCredentials",
  "help.whatsNew.items.connectorPasswords",
  "help.whatsNew.items.profileClarity",
  "help.whatsNew.items.aiHelp",
] as const;

export const HelpSettings: React.FC = () => {
  const { t } = useTranslation();
  const setSection = useNavigationStore((state) => state.setSection);
  const pendingHelpAnchor = useNavigationStore(
    (state) => state.pendingHelpAnchor,
  );
  const consumePendingHelpAnchor = useNavigationStore(
    (state) => state.consumePendingHelpAnchor,
  );
  const visibleSections = HELP_SECTIONS;
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
  const [aiExampleCopyStatus, setAiExampleCopyStatus] = useState<
    "idle" | "copied" | "error"
  >("idle");
  const aiExampleCopyResetTimerRef = useRef<number | null>(null);

  useEffect(
    () => () => {
      if (aiExampleCopyResetTimerRef.current !== null) {
        window.clearTimeout(aiExampleCopyResetTimerRef.current);
      }
    },
    [],
  );

  const handleCopyAiExample = useCallback(async () => {
    const copyText = `${t("help.aiAssist.exampleIntro")}\n\n${AIVORELAY_SOURCE_URL}\n${t("help.aiAssist.questionLead")}\n\n`;

    try {
      await navigator.clipboard.writeText(copyText);
      setAiExampleCopyStatus("copied");
    } catch (error) {
      console.error("Failed to copy the Help AI example:", error);
      setAiExampleCopyStatus("error");
    }

    if (aiExampleCopyResetTimerRef.current !== null) {
      window.clearTimeout(aiExampleCopyResetTimerRef.current);
    }
    aiExampleCopyResetTimerRef.current = window.setTimeout(() => {
      setAiExampleCopyStatus("idle");
      aiExampleCopyResetTimerRef.current = null;
    }, 2000);
  }, [t]);

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
            .searchHelp(nextQuery)
            .filter((result) => helpEntryByAnchor.has(result.anchor));
          setSearchResults(nextResults);
        })
        .catch(() => {
          if (latestQueryRef.current === nextQuery) {
            setSearchResults([]);
          }
        });
    },
    [ensureSearchLoaded, helpEntryByAnchor],
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

  const renderSmartHelpAction = (
    action: (typeof SMART_HELP_ACTIONS)[number],
  ) => {
    const Icon = action.icon;

    return (
      <div
        key={action.labelKey}
        className="rounded-lg border border-[#333333] bg-[#151515] p-3 transition-colors hover:border-[#ff4d8d]/45"
      >
        <button
          type="button"
          onClick={() => scrollToHelpAnchor(action.anchor)}
          className="flex w-full items-start gap-2.5 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#ff4d8d]/60 focus-visible:ring-offset-2 focus-visible:ring-offset-[#151515]"
        >
          <span className="mt-0.5 rounded-md bg-[#ff4d8d]/10 p-1.5 text-[#ff8ebb]">
            <Icon aria-hidden="true" className="h-4 w-4" />
          </span>
          <span className="min-w-0 flex-1">
            <span className="block text-xs font-semibold leading-snug text-[#f5f5f5]">
              {t(action.labelKey)}
            </span>
            <span className="mt-1 block text-xs leading-relaxed text-[#a0a0a0]">
              {t(action.descriptionKey)}
            </span>
          </span>
        </button>
      </div>
    );
  };

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

      <section
        aria-labelledby="help-ai-assist-title"
        className="rounded-xl border border-[#ff4d8d]/30 bg-[#1a1a1a] p-4"
      >
        <div className="flex min-w-0 items-start gap-3">
          <Sparkles
            aria-hidden="true"
            className="mt-0.5 h-4 w-4 shrink-0 text-[#ff8ebb]"
          />
          <div className="min-w-0">
            <p className="text-xs font-semibold uppercase tracking-[0.12em] text-[#ff8ebb]">
              {t("help.aiAssist.label")}
            </p>
            <h2
              id="help-ai-assist-title"
              className="mt-1 text-sm font-semibold text-[#f5f5f5]"
            >
              {t("help.aiAssist.title")}
            </h2>
            <p className="mt-1 text-xs leading-relaxed text-[#b8b8b8]">
              {t("help.aiAssist.description")}
            </p>
          </div>
        </div>

        <div className="mt-3 grid grid-cols-[minmax(0,1fr)_auto] overflow-hidden rounded-lg border border-[#363636] bg-[#111111]">
          <div className="min-w-0 px-3 py-3 font-mono text-xs leading-5">
            <p className="text-[#d8d8d8]">{t("help.aiAssist.exampleIntro")}</p>
            <a
              href={AIVORELAY_SOURCE_URL}
              target="_blank"
              rel="noopener noreferrer"
              className="mt-1 block break-all text-[#ff8ebb] underline decoration-[#ff4d8d]/55 underline-offset-4 hover:text-[#ffc0d5] focus-visible:rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#ff4d8d]/60"
            >
              {AIVORELAY_SOURCE_URL}
            </a>
            <p className="mt-3 text-[#d8d8d8]">
              {t("help.aiAssist.questionLead")}
            </p>
            <p className="mt-3 font-semibold text-red-400">
              {t("help.aiAssist.questionPlaceholder")}
            </p>
          </div>
          <div className="flex items-center border-l border-[#333333] bg-[#151515] px-3">
            <Button
              type="button"
              variant="secondary"
              size="sm"
              onClick={() => void handleCopyAiExample()}
              className="inline-flex min-w-[6.75rem] shrink-0 items-center justify-center gap-1.5 whitespace-nowrap"
            >
              {aiExampleCopyStatus === "copied" ? (
                <Check aria-hidden="true" className="h-3.5 w-3.5 text-green-400" />
              ) : (
                <Copy aria-hidden="true" className="h-3.5 w-3.5" />
              )}
              {t(`help.aiAssist.${aiExampleCopyStatus === "idle" ? "copy" : aiExampleCopyStatus}`)}
            </Button>
          </div>
        </div>
        <p className="mt-2.5 max-w-[72ch] text-[11px] leading-4 text-[#969696]">
          {t("help.aiAssist.copyHint")}
        </p>
      </section>

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
        <div className="mt-4 space-y-2 pl-7 sm:hidden">
          {SMART_HELP_ACTIONS.map(renderSmartHelpAction)}
        </div>
        <div className="mt-4 hidden grid-cols-2 items-start gap-2 pl-7 sm:grid">
          <div className="space-y-2">
            {SMART_HELP_ACTIONS.filter((_, index) => index % 2 === 0).map(
              renderSmartHelpAction,
            )}
          </div>
          <div className="space-y-2">
            {SMART_HELP_ACTIONS.filter((_, index) => index % 2 === 1).map(
              renderSmartHelpAction,
            )}
          </div>
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
                {section.id === "voiceCommands" && (
                  <div className="mt-3 rounded-lg border border-red-500/35 bg-red-500/10 p-3 text-xs leading-relaxed text-red-100">
                    <div className="flex items-start gap-2">
                      <AlertTriangle
                        aria-hidden="true"
                        className="mt-0.5 h-4 w-4 shrink-0 text-red-300"
                      />
                      <div className="space-y-1.5">
                        <p className="font-semibold text-red-200">
                          {t("help.sections.voiceCommands.warningTitle")}
                        </p>
                        <p>
                          {t("help.sections.voiceCommands.warning")}
                        </p>
                        <p className="text-red-100/85">
                          {t("help.sections.voiceCommands.enablePath")}
                        </p>
                      </div>
                    </div>
                  </div>
                )}
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
                    {subsection.warningKey && (
                      <div className="mt-2 flex items-start gap-2 rounded-md border border-amber-500/35 bg-amber-500/10 p-2 text-xs leading-relaxed text-amber-100">
                        <AlertTriangle
                          aria-hidden="true"
                          className="mt-0.5 h-4 w-4 shrink-0 text-amber-300"
                        />
                        <p>{t(subsection.warningKey)}</p>
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </section>
        ))}
      </div>

      <section
        aria-labelledby="help-whats-new-title"
        className="rounded-xl border border-[#ff4d8d]/30 bg-[#1a1a1a] p-4"
      >
        <div className="flex items-start gap-3">
          <Sparkles
            aria-hidden="true"
            className="mt-0.5 h-4 w-4 shrink-0 text-[#ff8ebb]"
          />
          <div className="min-w-0">
            <p className="text-xs font-semibold uppercase tracking-[0.12em] text-[#ff8ebb]">
              {t("help.whatsNew.since")}
            </p>
            <h2
              id="help-whats-new-title"
              className="mt-1 text-base font-semibold text-[#f5f5f5]"
            >
              {t("help.whatsNew.title")}
            </h2>
            <p className="mt-1.5 text-sm leading-relaxed text-[#b8b8b8]">
              {t("help.whatsNew.description")}
            </p>
          </div>
        </div>
        <ul className="mt-4 space-y-2 pl-7 text-sm leading-relaxed text-[#b8b8b8]">
          {WHATS_NEW_ITEMS.map((itemKey) => (
            <li key={itemKey} className="relative pl-4">
              <span
                aria-hidden="true"
                className="absolute left-0 top-[0.65em] h-1.5 w-1.5 -translate-y-1/2 rounded-full bg-[#ff4d8d]"
              />
              {t(itemKey)}
            </li>
          ))}
        </ul>
      </section>

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
