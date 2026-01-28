import React, { useState, useRef, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { ChevronLeft, ChevronRight, Pin, PinOff, Keyboard } from "lucide-react";
import { useSettings } from "@/hooks/useSettings";
import { HotkeyGroup } from "./HotkeyGroup";
import type { ShortcutBinding, TranscriptionProfile, AppSettings } from "@/bindings";

const DEFAULT_WIDTH = 350;
const MIN_WIDTH = 250;
const MAX_WIDTH = 600;

interface HotkeyCategory {
  id: string;
  titleKey: string;
  hotkeys: ShortcutBinding[];
}

/** Maps shortcut IDs to their feature enable setting key */
const featureEnabledMap: Record<string, keyof AppSettings> = {
  voice_command: "voice_command_enabled",
  send_to_extension: "send_to_extension_enabled",
  send_to_extension_with_selection: "send_to_extension_with_selection_enabled",
  send_screenshot_to_extension: "send_screenshot_to_extension_enabled",
};

/** Checks if a hotkey's feature is enabled (or has no toggle) */
const isFeatureEnabled = (hotkeyId: string, settings: AppSettings | null): boolean => {
  const settingKey = featureEnabledMap[hotkeyId];
  if (!settingKey || !settings) return true; // No toggle = always enabled
  return settings[settingKey] as boolean;
};

/** Categorizes hotkeys based on their ID, filtering out disabled features */
const categorizeHotkeys = (
  bindings: Record<string, ShortcutBinding>,
  profiles: TranscriptionProfile[],
  settings: AppSettings | null
): HotkeyCategory[] => {
  const assigned = Object.values(bindings).filter(
    (b) => b.current_binding && b.current_binding.trim() !== "" && isFeatureEnabled(b.id, settings)
  );

  // Define category mappings
  const categoryMap: Record<string, string[]> = {
    recording: ["transcribe", "transcribe_default", "cancel", "repaste_last", "cycle_profile"],
    actions: [
      "ai_replace_selection",
      "send_to_extension",
      "send_to_extension_with_selection",
      "send_screenshot_to_extension",
      "voice_command",
    ],
  };

  // Profile IDs (dynamic)
  const profileBindingIds = profiles.map((p) => `transcribe_${p.id}`);

  const categories: HotkeyCategory[] = [];

  // Recording category
  const recordingHotkeys = assigned.filter((h) =>
    categoryMap.recording.includes(h.id)
  );
  if (recordingHotkeys.length > 0) {
    categories.push({ id: "recording", titleKey: "hotkeySidebar.categories.recording", hotkeys: recordingHotkeys });
  }

  // Actions category
  const actionsHotkeys = assigned.filter((h) =>
    categoryMap.actions.includes(h.id)
  );
  if (actionsHotkeys.length > 0) {
    categories.push({ id: "actions", titleKey: "hotkeySidebar.categories.actions", hotkeys: actionsHotkeys });
  }

  // Profiles category
  const profileHotkeys = assigned.filter((h) =>
    profileBindingIds.includes(h.id)
  );
  if (profileHotkeys.length > 0) {
    categories.push({ id: "profiles", titleKey: "hotkeySidebar.categories.profiles", hotkeys: profileHotkeys });
  }

  return categories;
};

export const HotkeySidebar: React.FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting } = useSettings();
  const [isOpen, setIsOpen] = useState(false);
  const [isDragging, setIsDragging] = useState(false);
  const [isResizing, setIsResizing] = useState(false);
  const [width, setWidth] = useState(DEFAULT_WIDTH);
  const dragStartX = useRef<number>(0);
  const resizeStartX = useRef<number>(0);
  const resizeStartWidth = useRef<number>(DEFAULT_WIDTH);
  const sidebarRef = useRef<HTMLDivElement>(null);

  const isPinned = settings?.sidebar_pinned ?? false;
  const savedWidth = (settings as any)?.sidebar_width ?? DEFAULT_WIDTH;
  const bindings = settings?.bindings ?? {};
  const profiles = (settings as any)?.transcription_profiles ?? [];

  // Sync isOpen with isPinned when it changes (pinned = auto-open)
  React.useEffect(() => {
    if (isPinned) {
      setIsOpen(true);
    }
  }, [isPinned]);

  // Sync width with saved setting on load
  const initialWidthLoaded = useRef(false);
  React.useEffect(() => {
    if (!initialWidthLoaded.current && savedWidth) {
      initialWidthLoaded.current = true;
      setWidth(savedWidth);
    }
  }, [savedWidth]);

  const categories = useMemo(
    () => categorizeHotkeys(bindings, profiles, settings),
    [bindings, profiles, settings]
  );

  const hasAnyHotkeys = categories.length > 0;

  const handleTogglePin = useCallback(() => {
    updateSetting("sidebar_pinned", !isPinned);
  }, [isPinned, updateSetting]);

  // Handle click always toggles, regardless of pin state
  const handleToggleOpen = useCallback(() => {
    setIsOpen((prev) => !prev);
  }, []);

  // Drag handling for the edge handle (works regardless of pin state)
  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      setIsDragging(true);
      dragStartX.current = e.clientX;
      e.preventDefault();
    },
    []
  );

  const handleMouseMove = useCallback(
    (e: MouseEvent) => {
      if (!isDragging) return;
      const deltaX = dragStartX.current - e.clientX;
      // If dragged more than 50px to the left, open the sidebar
      if (deltaX > 50 && !isOpen) {
        setIsOpen(true);
        setIsDragging(false);
      }
      // If dragged more than 50px to the right, close the sidebar
      if (deltaX < -50 && isOpen) {
        setIsOpen(false);
        setIsDragging(false);
      }
    },
    [isDragging, isOpen]
  );

  const handleMouseUp = useCallback(() => {
    setIsDragging(false);
  }, []);

  // Resize handling
  const handleResizeStart = useCallback((e: React.MouseEvent) => {
    setIsResizing(true);
    resizeStartX.current = e.clientX;
    resizeStartWidth.current = width;
    e.preventDefault();
    e.stopPropagation();
  }, [width]);

  const handleResizeMove = useCallback((e: MouseEvent) => {
    if (!isResizing) return;
    const deltaX = resizeStartX.current - e.clientX;
    const newWidth = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, resizeStartWidth.current + deltaX));
    setWidth(newWidth);
  }, [isResizing]);

  const handleResizeEnd = useCallback(() => {
    if (isResizing) {
      setIsResizing(false);
      updateSetting("sidebar_width", width);
    }
  }, [isResizing, width, updateSetting]);

  // Attach global mouse listeners for drag
  React.useEffect(() => {
    if (isDragging) {
      window.addEventListener("mousemove", handleMouseMove);
      window.addEventListener("mouseup", handleMouseUp);
      return () => {
        window.removeEventListener("mousemove", handleMouseMove);
        window.removeEventListener("mouseup", handleMouseUp);
      };
    }
  }, [isDragging, handleMouseMove, handleMouseUp]);

  // Attach global mouse listeners for resize
  React.useEffect(() => {
    if (isResizing) {
      window.addEventListener("mousemove", handleResizeMove);
      window.addEventListener("mouseup", handleResizeEnd);
      return () => {
        window.removeEventListener("mousemove", handleResizeMove);
        window.removeEventListener("mouseup", handleResizeEnd);
      };
    }
  }, [isResizing, handleResizeMove, handleResizeEnd]);

  // Sidebar shows when open (isOpen syncs with isPinned automatically)
  const shouldShow = isOpen;

  // Don't render anything if no hotkeys are assigned
  if (!hasAnyHotkeys) {
    return null;
  }

  return (
    <>
      {/* Edge Handle - Samsung Edge style tab */}
      <div
        className="fixed top-1/2 -translate-y-1/2 z-50 cursor-pointer select-none transition-all duration-300 ease-out"
        style={{ right: shouldShow ? width : 0 }}
        onMouseDown={handleMouseDown}
        onClick={handleToggleOpen}
        title={shouldShow ? t("hotkeySidebar.closeSidebar") : t("hotkeySidebar.openSidebar")}
      >
        <div
          className={`flex items-center justify-center w-6 h-20 rounded-l-lg bg-gradient-to-b from-[#2a2a2a] to-[#1a1a1a] border border-r-0 border-[#3a3a3a] shadow-lg transition-all duration-200 hover:w-8 hover:bg-gradient-to-b hover:from-[#353535] hover:to-[#252525] ${
            isDragging ? "w-8 bg-gradient-to-b from-[#404040] to-[#303030]" : ""
          }`}
        >
          {shouldShow ? (
            <ChevronRight className="w-4 h-4 text-[#808080]" />
          ) : (
            <ChevronLeft className="w-4 h-4 text-[#808080]" />
          )}
        </div>
      </div>

      {/* Sidebar Panel */}
      <div
        ref={sidebarRef}
        className={`fixed top-0 right-0 h-full z-40 transition-transform duration-300 ease-out ${
          shouldShow ? "translate-x-0" : "translate-x-full"
        }`}
        style={{ width }}
      >
        {/* Resize Handle */}
        <div
          className="absolute left-0 top-0 w-1 h-full cursor-ew-resize hover:bg-[#ff6b9d]/30 transition-colors z-10"
          onMouseDown={handleResizeStart}
        />
        <div className="h-full bg-gradient-to-bl from-[#1a1a1a] via-[#151515] to-[#121212] border-l border-[#2a2a2a] flex flex-col shadow-2xl">
          {/* Header */}
          <div className="flex items-center justify-between px-4 py-4 border-b border-[#2a2a2a]">
            <div className="flex items-center gap-2">
              <Keyboard className="w-5 h-5 text-[#ff6b9d]" />
              <h2 className="text-base font-semibold text-[#f0f0f0]">{t("hotkeySidebar.title")}</h2>
            </div>
            <button
              onClick={handleTogglePin}
              className={`p-2 rounded-lg transition-colors ${
                isPinned
                  ? "bg-[#ff6b9d]/20 text-[#ff6b9d]"
                  : "text-[#707070] hover:text-[#a0a0a0] hover:bg-[#252525]"
              }`}
              title={isPinned ? t("hotkeySidebar.unpinSidebar") : t("hotkeySidebar.pinSidebar")}
            >
              {isPinned ? (
                <Pin className="w-4 h-4" />
              ) : (
                <PinOff className="w-4 h-4" />
              )}
            </button>
          </div>

          {/* Content */}
          <div className="flex-1 overflow-y-auto px-4 py-4">
            {categories.map((category) => (
              <HotkeyGroup
                key={category.id}
                title={t(category.titleKey)}
                hotkeys={category.hotkeys}
              />
            ))}
          </div>

          {/* Footer hint */}
          <div className="px-4 py-3 border-t border-[#2a2a2a] text-center">
            <span className="text-xs text-[#505050]">
              {t("hotkeySidebar.configureHint")}
            </span>
          </div>
        </div>
      </div>

    </>
  );
};
