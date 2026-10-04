import React, {
  useState,
  useRef,
  useLayoutEffect,
  useCallback,
  useMemo,
  useEffect,
} from "react";
import { useTranslation } from "react-i18next";
import { Cog, FlaskConical, Globe, History, Info, Sparkles, Wand2, Terminal, FileAudio, FileVolume2, Replace, Mic, Palette, Cpu, Radio, TextSelect, CircleHelp, Send } from "lucide-react";
import { type } from "@tauri-apps/plugin-os";
import HandyTextLogo from "./icons/HandyTextLogo";
import SpeechProcessingIcon from "./icons/SpeechProcessingIcon";
import { useSettings } from "../hooks/useSettings";
import { useDragReorder } from "../hooks/useDragReorder";
import { navigateToSettingsAnchor } from "../lib/anchorNavigation";
import { SettingsSearch } from "./settings/SettingsSearch";
import { SETTINGS_SEARCH_ENTRIES } from "./settings/settingsSearchCatalog";
import {
  GeneralSettings,
  AdvancedSettings,
  HistorySettings,
  DebugSettings,
  AboutSettings,
  PostProcessingSettings,
  ModelsSettings,
  BrowserConnectorSettings,
  AiReplaceSelectionSettings,
  VoiceCommandSettings,
  TranscribeFileSettings,
  TextReplacementSettings,
  SendSelectedTextSettings,
  AudioProcessingSettings,
  UserInterfaceSettings,
  LiveSoundTranscriptionSettings,
  TextToSpeechSettings,
  TtsFileOperationsSettings,
  HelpSettings,
} from "./settings";
import "./SidebarIconMotion.css";
import "./SidebarDrag.css";

export type SidebarSection = keyof typeof SECTIONS_CONFIG;

interface IconProps {
  width?: number | string;
  height?: number | string;
  size?: number | string;
  className?: string;
  [key: string]: any;
}

interface SectionConfig {
  labelKey: string;
  icon: React.ComponentType<IconProps>;
  component: React.ComponentType;
  enabled: (settings: any) => boolean;
}

const isWindows = type() === "windows";

export const SECTIONS_CONFIG = {
  general: {
    labelKey: "sidebar.general",
    icon: Mic,
    component: GeneralSettings,
    enabled: () => true,
  },
  models: {
    labelKey: "sidebar.models",
    icon: Cpu,
    component: ModelsSettings,
    enabled: () => true,
  },
  advanced: {
    labelKey: "sidebar.advanced",
    icon: Cog,
    component: AdvancedSettings,
    enabled: () => true,
  },
  postprocessing: {
    labelKey: "sidebar.postProcessing",
    icon: Sparkles,
    component: PostProcessingSettings,
    enabled: (_) => true,
  },
  aiReplace: {
    labelKey: "sidebar.aiReplace",
    icon: Wand2,
    component: AiReplaceSelectionSettings,
    enabled: () => isWindows,
  },
  sendSelectedText: {
    labelKey: "sidebar.sendSelectedText",
    icon: Send,
    component: SendSelectedTextSettings,
    enabled: () => isWindows,
  },
  voiceCommands: {
    labelKey: "sidebar.voiceCommands",
    icon: Terminal,
    component: VoiceCommandSettings,
    enabled: (settings) => isWindows && (settings?.beta_voice_commands_enabled ?? false),
  },
  browserConnector: {
    labelKey: "sidebar.browserConnector",
    icon: Globe,
    component: BrowserConnectorSettings,
    enabled: () => true,
  },
  textReplacement: {
    labelKey: "sidebar.textReplacement",
    icon: Replace,
    component: TextReplacementSettings,
    enabled: () => true,
  },
  userInterface: {
    labelKey: "sidebar.userInterface",
    icon: Palette,
    component: UserInterfaceSettings,
    enabled: () => true,
  },
  history: {
    labelKey: "sidebar.history",
    icon: History,
    component: HistorySettings,
    enabled: (_) => true,
  },
  audioProcessing: {
    labelKey: "sidebar.audioProcessing",
    icon: SpeechProcessingIcon,
    component: AudioProcessingSettings,
    enabled: () => true,
  },
  debug: {
    labelKey: "sidebar.debug",
    icon: FlaskConical,
    component: DebugSettings,
    enabled: (_) => true,
  },
  liveSoundTranscription: {
    labelKey: "sidebar.liveSoundTranscription",
    icon: Radio,
    component: LiveSoundTranscriptionSettings,
    enabled: () => true,
  },
  transcribeFile: {
    labelKey: "sidebar.transcribeFile",
    icon: FileVolume2,
    component: TranscribeFileSettings,
    enabled: () => true,
  },
  textToSpeech: {
    labelKey: "sidebar.textToSpeech",
    icon: TextSelect,
    component: TextToSpeechSettings,
    enabled: () => true,
  },
  ttsFiles: {
    labelKey: "sidebar.ttsFiles",
    icon: FileAudio,
    component: TtsFileOperationsSettings,
    enabled: () => true,
  },
  help: {
    labelKey: "sidebar.help",
    icon: CircleHelp,
    component: HelpSettings,
    enabled: () => true,
  },
  about: {
    labelKey: "sidebar.about",
    icon: Info,
    component: AboutSettings,
    enabled: () => true,
  },
} as const satisfies Record<string, SectionConfig>;

// Extra SVG parts that some hover motions need (hidden until they play)
const ICON_MOTION_EXTRAS: Partial<Record<SidebarSection, React.ReactNode>> = {
  debug: (
    <g key="motion-extras" fill="currentColor" stroke="none">
      <circle className="sidebar-icon-bubble" cx="9.5" cy="19.5" r="1" />
      <circle className="sidebar-icon-bubble" cx="14.2" cy="18.8" r="0.85" />
      <circle className="sidebar-icon-bubble" cx="12" cy="20.2" r="0.75" />
    </g>
  ),
};

const SIDEBAR_ORDER_KEY = "sidebar-section-order";

function loadSavedOrder(available: string[]): string[] {
  try {
    const raw = localStorage.getItem(SIDEBAR_ORDER_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as string[];
      const filtered = parsed.filter((id) => available.includes(id));
      const missing = available.filter((id) => !filtered.includes(id));
      const result = [...filtered];
      for (const id of missing) {
        if (id === "help") {
          const aboutIndex = result.indexOf("about");
          if (aboutIndex >= 0) {
            result.splice(aboutIndex, 0, id);
            continue;
          }
        }
        result.push(id);
      }
      return result;
    }
  } catch {
    // ignore parse errors
  }
  return [...available];
}

interface SidebarProps {
  activeSection: SidebarSection;
  onSectionChange: (section: SidebarSection) => void;
  onSearchHelp: (query: string) => void;
}

export const Sidebar: React.FC<SidebarProps> = ({
  activeSection,
  onSectionChange,
  onSearchHelp,
}) => {
  const { t } = useTranslation();
  const { settings } = useSettings();

  const availableSections = useMemo(
    () =>
      Object.entries(SECTIONS_CONFIG)
        .filter(([_, config]) => config.enabled(settings))
        .map(([id]) => id),
    [settings],
  );

  const [order, setOrder] = useState<string[]>(() =>
    loadSavedOrder(availableSections),
  );

  const navigateFromSearch = useCallback(
    (sectionId: string, anchorId?: string, expandAnchorId?: string, fallbackAnchorId?: string) => {
      const section = sectionId as SidebarSection;
      const sectionAnchorId = `settings-section-${section}`;
      navigateToSettingsAnchor({
        activateSection: () => onSectionChange(section),
        targetId: anchorId ?? sectionAnchorId,
        readyId: sectionAnchorId,
        fallbackId: fallbackAnchorId ?? expandAnchorId ?? sectionAnchorId,
        expandId: expandAnchorId,
        block: "center",
      });
    },
    [onSectionChange],
  );

  // Keep order in sync when availableSections changes
  const prevAvailableRef = useRef(availableSections.join(","));
  useEffect(() => {
    const key = availableSections.join(",");
    if (key !== prevAvailableRef.current) {
      prevAvailableRef.current = key;
      setOrder((prev) => {
        const filtered = prev.filter((id) => availableSections.includes(id));
        const missing = availableSections.filter((id) => !filtered.includes(id));
        return [...filtered, ...missing];
      });
    }
  }, [availableSections]);

  const itemRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const scrollRef = useRef<HTMLDivElement>(null);

  // ── Drag to reorder ──────────────────────────────────────────────────────
  // The drag preview moves items with transforms only, so the rendered order
  // stays the saved order until the drop commits a new one.
  const visualOrder = order;

  const commitOrder = useCallback((next: string[]) => {
    setOrder(next);
    try {
      localStorage.setItem(SIDEBAR_ORDER_KEY, JSON.stringify(next));
    } catch {
      /* ignore */
    }
  }, []);

  const drag = useDragReorder({
    order,
    itemRefs,
    scrollRef,
    onReorder: commitOrder,
    onActivate: (id) => onSectionChange(id as SidebarSection),
  });

  // ── Sliding active indicator ────────────────────────────────────────────
  const navListRef = useRef<HTMLDivElement>(null);
  const indicatorRef = useRef<HTMLDivElement>(null);
  const indicatorBarRef = useRef<HTMLDivElement>(null);
  const indicatorTopRef = useRef<number | null>(null);
  const barAnimationRef = useRef<Animation | null>(null);

  const updateIndicator = useCallback(
    (animate: boolean) => {
      const indicator = indicatorRef.current;
      if (!indicator) return;

      const activeEl = itemRefs.current.get(activeSection);
      if (!activeEl) {
        indicator.style.opacity = "0";
        indicatorTopRef.current = null;
        return;
      }

      // Layout position (ignores transient FLIP transforms on items)
      const top = activeEl.offsetTop;
      const height = activeEl.offsetHeight;
      const prevTop = indicatorTopRef.current;
      if (!animate && top === prevTop && indicator.style.height === `${height}px`) {
        return; // geometry unchanged; do not interrupt a running glide
      }
      const shouldAnimate = animate && prevTop !== null;

      if (!shouldAnimate) {
        // Jump without transition (first paint, resizes)
        indicator.style.transition = "none";
      }
      indicator.style.transform = `translateY(${top}px)`;
      indicator.style.height = `${height}px`;
      indicator.style.opacity = "1";
      if (!shouldAnimate) {
        void indicator.offsetHeight; // force reflow before restoring transitions
        indicator.style.transition = "";
      }
      indicatorTopRef.current = top;

      // Liquid stretch of the accent bar, proportional to travel distance
      const distance = prevTop === null ? 0 : Math.abs(top - prevTop);
      const bar = indicatorBarRef.current;
      const reduceMotion = window.matchMedia(
        "(prefers-reduced-motion: reduce)",
      ).matches;
      if (
        shouldAnimate &&
        distance > 1 &&
        bar &&
        typeof bar.animate === "function" &&
        !reduceMotion
      ) {
        const stretch = Math.min(1 + distance / 100, 2.6);
        barAnimationRef.current?.cancel();
        barAnimationRef.current = bar.animate(
          [
            { transform: "scale(1, 1)" },
            { transform: `scale(0.7, ${stretch})`, offset: 0.3 },
            { transform: "scale(1.15, 0.85)", offset: 0.72 },
            { transform: "scale(1, 1)" },
          ],
          { duration: 520, easing: "cubic-bezier(0.22, 1, 0.36, 1)" },
        );
      }
    },
    [activeSection],
  );

  // Re-measure on section change and on any order change (drag preview, commit)
  useLayoutEffect(() => {
    updateIndicator(true);
  }, [updateIndicator, visualOrder]);

  // Keep the indicator aligned if the list itself resizes. Uses a ref so the
  // observer is not re-created (and does not fire) on every section change.
  const updateIndicatorRef = useRef(updateIndicator);
  updateIndicatorRef.current = updateIndicator;
  useEffect(() => {
    const list = navListRef.current;
    if (!list || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() =>
      updateIndicatorRef.current(false),
    );
    observer.observe(list);
    return () => observer.disconnect();
  }, []);

  // ── Hover icon motion ───────────────────────────────────────────────────
  // Plays the icon's own motion (SidebarIconMotion.css) once per hover. It
  // always runs to completion, and re-entering mid-motion does not restart it.
  const playIconMotion = useCallback((item: HTMLElement) => {
    if (drag.isDragging()) return;
    const icon = item.querySelector<HTMLElement>(".sidebar-icon-motion");
    const svg = icon?.querySelector("svg");
    if (!icon || !svg || icon.dataset.motion) return;
    if (typeof svg.getAnimations !== "function") return;

    icon.dataset.motion = "play";
    // Reading the animations flushes styles, so the new ones are included
    const animations = svg.getAnimations({ subtree: true });
    void Promise.allSettled(animations.map((a) => a.finished)).then(() => {
      delete icon.dataset.motion;
    });
  }, [drag]);

  return (
    <div className="adobe-sidebar flex flex-col w-56 h-full items-center px-3 py-4">
      {/* Logo — fixed at top */}
      <div className="w-full p-3 mb-2 shrink-0">
        <HandyTextLogo className="w-full h-auto drop-shadow-[0_0_8px_rgba(255,107,157,0.3)]" />
      </div>

      {/* Gradient Divider */}
      <div className="section-divider w-full mb-4 shrink-0" />

      <SettingsSearch
        entries={SETTINGS_SEARCH_ENTRIES}
        settings={settings}
        availableSections={availableSections}
        sectionLabelKey={(section) =>
          SECTIONS_CONFIG[section as SidebarSection]?.labelKey ?? null
        }
        onNavigate={navigateFromSearch}
        onSearchHelp={onSearchHelp}
      />

      {/* Navigation Items — scrollable */}
      <div ref={scrollRef} className="flex-1 w-full min-h-0 overflow-y-auto">
        <div
          ref={navListRef}
          className="sidebar-nav-list has-sliding-indicator relative flex flex-col w-full gap-1"
        >
          {/* Shared active indicator; clipped layer prevents scroll overflow */}
          <div className="sidebar-indicator-layer" aria-hidden="true">
            <div ref={indicatorRef} className="sidebar-indicator">
              <div ref={indicatorBarRef} className="sidebar-indicator__bar" />
            </div>
          </div>
          {visualOrder.map((id) => {
            const section = SECTIONS_CONFIG[id as SidebarSection];
            if (!section) return null;

            const Icon: React.ComponentType<IconProps> = section.icon;
            const isActive = activeSection === id;

            return (
              <div
                key={id}
                data-testid={`sidebar-section-${id}`}
                role="button"
                tabIndex={0}
                aria-current={isActive ? "page" : undefined}
                ref={(el) => {
                  if (el) itemRefs.current.set(id, el);
                  else itemRefs.current.delete(id);
                }}
                onPointerEnter={(e) => playIconMotion(e.currentTarget)}
                onPointerDown={(e) => drag.onPointerDown(e, id)}
                onPointerMove={drag.onPointerMove}
                onPointerUp={drag.onPointerUp}
                onPointerCancel={drag.onPointerCancel}
                onLostPointerCapture={drag.onPointerCancel}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    onSectionChange(id as SidebarSection);
                  }
                }}
                className={`adobe-sidebar-item flex gap-3 items-center w-full select-none hover:cursor-grab focus-visible:outline focus-visible:outline-2 focus-visible:outline-[#ff4d8d]/70 ${
                  isActive ? "active" : ""
                }`}
              >
                {/* Icon */}
                <div
                  className={`sidebar-item-icon sidebar-icon-motion shrink-0 transition-all duration-200 ${
                    isActive
                      ? "text-[#ff4d8d] drop-shadow-[0_0_6px_rgba(255,77,141,0.5)]"
                      : "text-[#b8b8b8]"
                  }`}
                >
                  <Icon width={20} height={20} aria-hidden="true">
                    {ICON_MOTION_EXTRAS[id as SidebarSection]}
                  </Icon>
                </div>

                {/* Label */}
                <p
                  className={`text-sm font-medium truncate transition-colors duration-200 ${
                    isActive ? "text-[#f5f5f5]" : "text-[#b8b8b8]"
                  }`}
                  title={t(section.labelKey)}
                >
                  {t(section.labelKey)}
                </p>
              </div>
            );
          })}
        </div>
      </div>

      {/* Footer — fixed at bottom */}
      <div className="section-divider w-full mt-4 shrink-0" />
      <div className="w-full py-3 px-2 text-center shrink-0">
        <span className="text-xs text-[#707070]">AivoRelay</span>
      </div>
    </div>
  );
};
