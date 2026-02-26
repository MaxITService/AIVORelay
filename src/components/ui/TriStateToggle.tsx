import React from "react";
import { useTranslation } from "react-i18next";

/**
 * Three-state segmented pill control.
 *
 * States:
 *   global — inherit / follow global setting
 *   off    — explicitly off
 *   on     — explicitly on
 *
 * Renders as three clearly labeled segments the user clicks to select.
 */

export type TriStateValue = "global" | "off" | "on";

interface TriStateToggleProps {
  value: TriStateValue;
  onChange: (value: TriStateValue) => void;
  disabled?: boolean;
  /** Resolved global value, shown when value is global */
  globalValue?: boolean;
}

type Segment = { key: TriStateValue; label: string };

export const TriStateToggle: React.FC<TriStateToggleProps> = ({
  value,
  onChange,
  disabled = false,
  globalValue = false,
}) => {
  const { t } = useTranslation();

  const segments: Segment[] = [
    { key: "global", label: t("common.tristate.global", "Global") },
    { key: "off", label: t("common.tristate.off", "Off") },
    { key: "on", label: t("common.tristate.on", "On") },
  ];

  const isActive = (key: TriStateValue) => value === key;

  const activeStyle = (key: TriStateValue) => {
    if (!isActive(key)) return "";
    if (key === "global") {
      // Inherit: show dimmed purple to indicate it follows global
      return globalValue
        ? "bg-[#9b5de5]/50 text-white shadow-[0_1px_3px_rgba(155,93,229,0.3)]"
        : "bg-[#555]/80 text-white shadow-[0_1px_3px_rgba(0,0,0,0.3)]";
    }
    if (key === "on")
      return "bg-[#9b5de5] text-white shadow-[0_1px_3px_rgba(155,93,229,0.3)]";
    // off
    return "bg-[#555] text-white shadow-[0_1px_3px_rgba(0,0,0,0.3)]";
  };

  return (
    <div
      className={`inline-flex rounded-md bg-[#1a1a1a] p-[2px] ${
        disabled ? "opacity-40 pointer-events-none" : ""
      }`}
      role="radiogroup"
    >
      {segments.map((seg) => (
        <button
          key={String(seg.key)}
          type="button"
          role="radio"
          aria-checked={isActive(seg.key)}
          disabled={disabled}
          onClick={() => onChange(seg.key)}
          className={`px-2 py-[2px] text-[9px] font-medium rounded-[4px] transition-all duration-150 select-none
            ${
              isActive(seg.key)
                ? activeStyle(seg.key)
                : "text-text/40 hover:text-text/70"
            }
          `}
        >
          {seg.label}
        </button>
      ))}
    </div>
  );
};
