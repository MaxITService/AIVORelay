import React from "react";
import { useTranslation } from "react-i18next";
import type { ShortcutBinding } from "@/bindings";
import { getShortcutAnchorId } from "@/lib/shortcutAnchors";

interface HotkeyGroupProps {
  title: string;
  hotkeys: ShortcutBinding[];
  onHotkeyClick: (shortcutId: string) => void;
}

/** Formats a shortcut binding string for display (e.g., "ctrl+shift+space" -> "Ctrl + Shift + Space") */
const formatShortcut = (binding: string): string => {
  if (!binding) return "";
  return binding
    .split("+")
    .map((key) => key.charAt(0).toUpperCase() + key.slice(1))
    .join(" + ");
};

export const HotkeyGroup: React.FC<HotkeyGroupProps> = ({
  title,
  hotkeys,
  onHotkeyClick,
}) => {
  const { t } = useTranslation();

  if (hotkeys.length === 0) return null;

  return (
    <div className="mb-4">
      <h3 className="text-xs font-semibold text-[#808080] uppercase tracking-wider mb-2 px-1">
        {title}
      </h3>
      <div className="flex flex-col gap-1.5">
        {hotkeys.map((hotkey) => (
          <a
            key={hotkey.id}
            href={`#${getShortcutAnchorId(hotkey.id)}`}
            onClick={(event) => {
              event.preventDefault();
              onHotkeyClick(hotkey.id);
            }}
            className="flex items-center justify-between gap-2 px-3 py-2 rounded-lg bg-[#1a1a1a]/60 hover:bg-[#252525]/80 transition-colors focus:outline-none focus:ring-2 focus:ring-[#ff4d8d]/45"
          >
            <span
              className="flex min-w-0 flex-1 flex-wrap items-baseline gap-x-2 gap-y-0.5 pr-2"
              title={hotkey.name}
            >
              <span className="text-sm text-[#d0d0d0] truncate">
                {hotkey.name}
              </span>
              {hotkey.id === "repaste_last" && (
                <span className="rounded-md border border-amber-400/30 bg-amber-400/10 px-1.5 py-0.5 text-[11px] font-semibold leading-snug text-amber-200">
                  {t("hotkeySidebar.visibleHints.repasteLastNetworkRetry")}
                </span>
              )}
            </span>
            <kbd className="text-xs font-mono text-[#ff6b9d] bg-[#2a1a22] px-2 py-1 rounded border border-[#3a2a32] whitespace-nowrap">
              {formatShortcut(hotkey.current_binding)}
            </kbd>
          </a>
        ))}
      </div>
    </div>
  );
};
