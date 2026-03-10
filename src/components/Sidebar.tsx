import React, {
  useState,
  useRef,
  useLayoutEffect,
  useCallback,
  useMemo,
  useEffect,
} from "react";
import { useTranslation } from "react-i18next";
import { Cog, FlaskConical, Globe, History, Info, Sparkles, Wand2, Terminal, FileAudio, Replace, Mic, Palette, Cpu, Radio } from "lucide-react";
import { type } from "@tauri-apps/plugin-os";
import HandyTextLogo from "./icons/HandyTextLogo";
import HandyHand from "./icons/HandyHand";
import { useSettings } from "../hooks/useSettings";
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
  AudioProcessingSettings,
  UserInterfaceSettings,
  LiveSoundTranscriptionSettings,
} from "./settings";

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
    icon: HandyHand,
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
    icon: Mic,
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
    icon: FileAudio,
    component: TranscribeFileSettings,
    enabled: () => true,
  },
  about: {
    labelKey: "sidebar.about",
    icon: Info,
    component: AboutSettings,
    enabled: () => true,
  },
} as const satisfies Record<string, SectionConfig>;

const SIDEBAR_ORDER_KEY = "sidebar-section-order";
const DRAG_THRESHOLD_PX = 5;

function loadSavedOrder(available: string[]): string[] {
  try {
    const raw = localStorage.getItem(SIDEBAR_ORDER_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as string[];
      const filtered = parsed.filter((id) => available.includes(id));
      const missing = available.filter((id) => !filtered.includes(id));
      return [...filtered, ...missing];
    }
  } catch {
    // ignore parse errors
  }
  return [...available];
}

interface SidebarProps {
  activeSection: SidebarSection;
  onSectionChange: (section: SidebarSection) => void;
}

export const Sidebar: React.FC<SidebarProps> = ({
  activeSection,
  onSectionChange,
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

  // ── Pointer-based drag state ─────────────────────────────────────────────
  const [draggingId, setDraggingId] = useState<string | null>(null);
  const [hoverTargetId, setHoverTargetId] = useState<string | null>(null);
  // Ref mirrors hoverTargetId so pointer callbacks always see the latest value
  // (state updates are async — closures would read stale values)
  const hoverTargetRef = useRef<string | null>(null);

  // Pointer drag bookkeeping (mutable, survives re-renders)
  const dragRef = useRef<{
    active: boolean;
    id: string;
    startY: number;
    pointerId: number;
    movedPastThreshold: boolean;
  } | null>(null);

  const itemRefs = useRef<Map<string, HTMLDivElement>>(new Map());

  // Visual (preview) order during drag
  const visualOrder = useMemo(() => {
    if (!draggingId || !hoverTargetId || draggingId === hoverTargetId)
      return order;
    const result = [...order];
    const fromIdx = result.indexOf(draggingId);
    const toIdx = result.indexOf(hoverTargetId);
    if (fromIdx === -1 || toIdx === -1) return order;
    result.splice(fromIdx, 1);
    result.splice(toIdx, 0, draggingId);
    return result;
  }, [order, draggingId, hoverTargetId]);

  // ── FLIP animation ──────────────────────────────────────────────────────
  const prevPositions = useRef<Map<string, number>>(new Map());
  const flipNeeded = useRef(false);

  const capturePositions = useCallback(() => {
    itemRefs.current.forEach((el, id) => {
      prevPositions.current.set(id, el.getBoundingClientRect().top);
    });
  }, []);

  useLayoutEffect(() => {
    if (!flipNeeded.current) return;
    flipNeeded.current = false;

    itemRefs.current.forEach((el, id) => {
      if (id === draggingId) return;
      const prev = prevPositions.current.get(id);
      if (prev === undefined) return;
      const current = el.getBoundingClientRect().top;
      const diff = prev - current;
      if (Math.abs(diff) < 1) return;

      el.style.transition = "none";
      el.style.transform = `translateY(${diff}px)`;

      requestAnimationFrame(() => {
        el.style.transition =
          "transform 0.32s cubic-bezier(0.34, 1.56, 0.64, 1)";
        el.style.transform = "translateY(0px)";
        el.addEventListener(
          "transitionend",
          () => {
            el.style.transition = "";
            el.style.transform = "";
          },
          { once: true },
        );
      });
    });
  });

  // ── Pointer event helpers ────────────────────────────────────────────────
  // Determine which sidebar item the pointer is currently over
  const hitTest = useCallback(
    (clientY: number): string | null => {
      for (const [id, el] of itemRefs.current) {
        const rect = el.getBoundingClientRect();
        if (clientY >= rect.top && clientY <= rect.bottom) return id;
      }
      return null;
    },
    [],
  );

  const finalizeDrag = useCallback(() => {
    const drag = dragRef.current;
    if (!drag) return;

    const target = hoverTargetRef.current;
    if (drag.active && target && drag.id !== target) {
      // Commit the reorder
      setOrder((prev) => {
        const result = [...prev];
        const fromIdx = result.indexOf(drag.id);
        const toIdx = result.indexOf(target);
        if (fromIdx === -1 || toIdx === -1) return prev;
        result.splice(fromIdx, 1);
        result.splice(toIdx, 0, drag.id);
        try {
          localStorage.setItem(SIDEBAR_ORDER_KEY, JSON.stringify(result));
        } catch {
          /* ignore */
        }
        return result;
      });
    }

    dragRef.current = null;
    hoverTargetRef.current = null;
    setDraggingId(null);
    setHoverTargetId(null);
  }, []);

  const onPointerDown = useCallback(
    (e: React.PointerEvent<HTMLDivElement>, id: string) => {
      if (e.button !== 0) return; // left button only
      dragRef.current = {
        active: false,
        id,
        startY: e.clientY,
        pointerId: e.pointerId,
        movedPastThreshold: false,
      };
      // Capture so we get move/up even outside the element
      (e.currentTarget as HTMLDivElement).setPointerCapture(e.pointerId);
    },
    [],
  );

  const onPointerMove = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      const drag = dragRef.current;
      if (!drag) return;

      if (!drag.movedPastThreshold) {
        if (Math.abs(e.clientY - drag.startY) < DRAG_THRESHOLD_PX) return;
        drag.movedPastThreshold = true;
        drag.active = true;
        setDraggingId(drag.id);
      }

      const target = hitTest(e.clientY);
      if (target && target !== hoverTargetRef.current && target !== drag.id) {
        capturePositions();
        flipNeeded.current = true;
        hoverTargetRef.current = target;
        setHoverTargetId(target);
      }
    },
    [hitTest, capturePositions],
  );

  const onPointerUp = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      const drag = dragRef.current;
      if (!drag) return;
      (e.currentTarget as HTMLDivElement).releasePointerCapture(drag.pointerId);

      if (!drag.movedPastThreshold) {
        // It was a click, not a drag — navigate
        dragRef.current = null;
        onSectionChange(drag.id as SidebarSection);
        return;
      }

      finalizeDrag();
    },
    [finalizeDrag, onSectionChange],
  );

  return (
    <div className="adobe-sidebar flex flex-col w-56 h-full items-center px-3 py-4">
      {/* Logo — fixed at top */}
      <div className="w-full p-3 mb-2 shrink-0">
        <HandyTextLogo className="w-full h-auto drop-shadow-[0_0_8px_rgba(255,107,157,0.3)]" />
      </div>

      {/* Gradient Divider */}
      <div className="section-divider w-full mb-4 shrink-0" />

      {/* Navigation Items — scrollable */}
      <div className="flex-1 w-full min-h-0 overflow-y-auto">
        <div className="flex flex-col w-full gap-1">
          {visualOrder.map((id) => {
            const section = SECTIONS_CONFIG[id as SidebarSection];
            if (!section) return null;

            const Icon = section.icon;
            const isActive = activeSection === id;
            const isDragging = draggingId === id;
            const isDropTarget = hoverTargetId === id && !isDragging;

            return (
              <div
                key={id}
                ref={(el) => {
                  if (el) itemRefs.current.set(id, el);
                  else itemRefs.current.delete(id);
                }}
                onPointerDown={(e) => onPointerDown(e, id)}
                onPointerMove={onPointerMove}
                onPointerUp={onPointerUp}
                className={`adobe-sidebar-item flex gap-3 items-center w-full select-none hover:cursor-grab ${
                  isActive ? "active" : ""
                } ${isDragging ? "opacity-40 cursor-grabbing" : ""} ${
                  isDropTarget
                    ? "outline outline-1 outline-[#ff4d8d]/40 rounded"
                    : ""
                }`}
              >
                {/* Icon */}
                <div
                  className={`shrink-0 transition-all duration-200 ${
                    isActive
                      ? "text-[#ff4d8d] drop-shadow-[0_0_6px_rgba(255,77,141,0.5)]"
                      : "text-[#b8b8b8]"
                  }`}
                >
                  <Icon width={20} height={20} />
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
