import React from "react";
import type { ShortcutBinding } from "@/bindings";

interface HotkeyGroupProps {
  title: string;
  hotkeys: ShortcutBinding[];
}

/** Formats a shortcut binding string for display (e.g., "ctrl+shift+space" -> "Ctrl + Shift + Space") */
const formatShortcut = (binding: string): string => {
  if (!binding) return "";
  return binding
    .split("+")
    .map((key) => key.charAt(0).toUpperCase() + key.slice(1))
    .join(" + ");
};

export const HotkeyGroup: React.FC<HotkeyGroupProps> = ({ title, hotkeys }) => {
  if (hotkeys.length === 0) return null;

  return (
    <div className="mb-4">
      <h3 className="text-xs font-semibold text-[#808080] uppercase tracking-wider mb-2 px-1">
        {title}
      </h3>
      <div className="flex flex-col gap-1.5">
        {hotkeys.map((hotkey) => (
          <div
            key={hotkey.id}
            className="flex items-center justify-between px-3 py-2 rounded-lg bg-[#1a1a1a]/60 hover:bg-[#252525]/80 transition-colors"
          >
            <span className="text-sm text-[#d0d0d0] truncate pr-2" title={hotkey.name}>
              {hotkey.name}
            </span>
            <kbd className="text-xs font-mono text-[#ff6b9d] bg-[#2a1a22] px-2 py-1 rounded border border-[#3a2a32] whitespace-nowrap">
              {formatShortcut(hotkey.current_binding)}
            </kbd>
          </div>
        ))}
      </div>
    </div>
  );
};
