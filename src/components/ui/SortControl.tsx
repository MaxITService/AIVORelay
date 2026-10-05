import React from "react";
import { ArrowUpDown, ChevronDown } from "lucide-react";
import { Tooltip } from "./Tooltip";
import type { SortDirection } from "@/lib/displayNameSorting";

export type { SortDirection } from "@/lib/displayNameSorting";

interface SortControlProps {
  direction: SortDirection;
  onChange: (direction: SortDirection) => void;
  onTurnOff?: () => void;
  ariaLabel: string;
  labels: Record<SortDirection, string>;
  offTooltip: string;
  activeTooltip: string;
  variant?: "default" | "subtle";
}

export const SortControl: React.FC<SortControlProps> = ({
  direction,
  onChange,
  onTurnOff,
  ariaLabel,
  labels,
  offTooltip,
  activeTooltip,
  variant = "default",
}) => {
  const handleMouseDown = (event: React.MouseEvent<HTMLSelectElement>) => {
    if (!event.shiftKey) return;
    event.preventDefault();
    if (onTurnOff) {
      onTurnOff();
    } else {
      onChange("off");
    }
    event.currentTarget.blur();
  };

  return (
    <Tooltip
      content={direction === "off" ? offTooltip : activeTooltip}
      position="top"
    >
      <span
        className={`relative inline-block shrink-0 ${
          variant === "subtle" ? "w-[4.5rem]" : "w-[5.75rem]"
        }`}
      >
        {variant !== "subtle" && (
          <ArrowUpDown
            className={`pointer-events-none absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 ${
              direction === "off" ? "text-[#606060]" : "text-[#c69cff]"
            }`}
            aria-hidden="true"
          />
        )}
        <select
          data-sort-control="true"
          value={direction}
          onMouseDown={handleMouseDown}
          onChange={(event) =>
            onChange(event.target.value as SortDirection)
          }
          aria-label={ariaLabel}
          className={`min-h-7 w-full appearance-none rounded-md border py-1 text-xs outline-none transition-colors [color-scheme:dark] ${
            variant === "subtle"
              ? "cursor-pointer border-transparent bg-transparent pl-2 pr-6 font-normal text-[#999999] hover:border-[#3c3c3c] hover:bg-[#282828] hover:text-[#c0c0c0] focus-visible:border-[#606060] focus-visible:bg-[#282828]"
              : `bg-[#181818] pl-7 pr-7 focus:border-[#9b5de5] ${
                  direction === "off"
                    ? "border-[#3c3c3c] text-[#8a8a8a] hover:border-[#505050]"
                    : "border-[#9b5de5] text-[#c69cff]"
                }`
          }`}
        >
          <option value="off" title={offTooltip}>
            {labels.off}
          </option>
          <option value="asc">
            {labels.asc}
          </option>
          <option value="desc">
            {labels.desc}
          </option>
        </select>
        <ChevronDown
          className={`pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 text-[#707070] ${
            variant === "subtle" ? "h-3 w-3" : "h-3.5 w-3.5"
          }`}
          aria-hidden="true"
        />
      </span>
    </Tooltip>
  );
};
