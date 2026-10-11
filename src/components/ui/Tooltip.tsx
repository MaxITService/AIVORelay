import React, { useCallback, useEffect, useLayoutEffect, useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { usePresence } from "@/hooks/usePresence";
import { useTooltipOpen } from "@/hooks/useTooltipOpen";
import { TOOLTIP_EXIT_DURATION_MS } from "@/lib/motion";
import {
  computeTooltipLayout,
  getTooltipArrowOrigin,
  TooltipLayout,
} from "./tooltipPositioning";
import "./Tooltip.css";

interface TooltipProps {
  content: React.ReactNode;
  children: React.ReactNode;
  position?: "top" | "bottom";
  disabled?: boolean;
  className?: string;
  tabIndex?: number;
  triggerRole?: React.AriaRole;
  ariaLabel?: string;
  ariaDisabled?: boolean;
}

/**
 * Generic tooltip wrapper — hover or focus children to see the tooltip.
 * Reuses the same portal + positioning logic as InfoTooltip / SettingContainer.
 */
export const Tooltip: React.FC<TooltipProps> = ({
  content,
  children,
  position = "top",
  disabled = false,
  className,
  tabIndex,
  triggerRole,
  ariaLabel,
  ariaDisabled,
}) => {
  const { open, show, showNow, hide } = useTooltipOpen();
  const tooltipId = useId();
  const hoveredRef = useRef(false);
  const focusedRef = useRef(false);
  const presence = usePresence(open, TOOLTIP_EXIT_DURATION_MS);
  const visible = !disabled && presence.isMounted;
  const triggerRef = useRef<HTMLSpanElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const [layout, setLayout] = useState<TooltipLayout | null>(null);

  useEffect(() => {
    if (disabled) hide();
    else if (focusedRef.current) showNow();
    else if (hoveredRef.current) show();
  }, [disabled, hide, show, showNow]);

  const updateLayout = useCallback(() => {
    if (!visible || !triggerRef.current || !tooltipRef.current) return;
    const triggerRect = triggerRef.current.getBoundingClientRect();
    const tooltipRect = tooltipRef.current.getBoundingClientRect();
    setLayout(
      computeTooltipLayout({
        triggerRect,
        tooltipRect,
        preferredPlacement: position,
        viewportWidth: window.innerWidth,
        viewportHeight: window.innerHeight,
      }),
    );
  }, [visible, position]);

  useEffect(() => {
    if (!visible) return;
    window.addEventListener("scroll", updateLayout, true);
    window.addEventListener("resize", updateLayout);
    return () => {
      window.removeEventListener("scroll", updateLayout, true);
      window.removeEventListener("resize", updateLayout);
    };
  }, [visible, updateLayout]);

  useLayoutEffect(() => {
    if (visible) updateLayout();
    else setLayout(null);
  }, [visible, updateLayout, content]);

  const placement = layout?.placement ?? position;
  // Measure the outer wrapper: the inner box is scaled while it animates.
  const portal = visible
    ? createPortal(
        <div
          ref={tooltipRef}
          id={tooltipId}
          role="tooltip"
          className="fixed z-[9999] pointer-events-none"
          style={
            layout
              ? { top: layout.top, left: layout.left }
              : { top: 0, left: 0, visibility: "hidden" as const }
          }
        >
          <div
            data-state={presence.state}
            className="aivo-tooltip relative px-4 py-2.5 bg-[#323232]/98 backdrop-blur-xl border border-[#4a4a4a] rounded-lg shadow-[0_8px_24px_rgba(0,0,0,0.5)] max-w-xs min-w-[200px] whitespace-normal"
            style={{
              transformOrigin: getTooltipArrowOrigin(placement, layout?.arrowLeft),
            }}
          >
            <p className="text-sm text-[#e8e8e8] text-center leading-relaxed">
              {content}
            </p>
            <div
              className={`absolute w-0 h-0 border-l-[6px] border-r-[6px] border-[6px] border-l-transparent border-r-transparent ${
                placement === "top"
                  ? "top-full border-t-[#4a4a4a] border-b-transparent"
                  : "bottom-full border-b-[#4a4a4a] border-t-transparent"
              }`}
              style={{
                left: layout ? layout.arrowLeft : "50%",
                transform: "translateX(-50%)",
              }}
            />
          </div>
        </div>,
        document.body,
      )
    : null;

  return (
    <span
      ref={triggerRef}
      tabIndex={tabIndex}
      role={triggerRole}
      aria-label={ariaLabel}
      aria-disabled={ariaDisabled}
      aria-describedby={!disabled ? tooltipId : undefined}
      onMouseEnter={() => {
        hoveredRef.current = true;
        if (!disabled) show();
      }}
      onMouseLeave={() => {
        hoveredRef.current = false;
        if (!focusedRef.current) hide();
      }}
      onFocus={() => {
        focusedRef.current = true;
        if (!disabled) showNow();
      }}
      onBlur={(event) => {
        if (event.currentTarget.contains(event.relatedTarget)) return;
        focusedRef.current = false;
        if (!hoveredRef.current) hide();
      }}
      onKeyDown={(event) => {
        if (event.key === "Escape") hide();
      }}
      className={[!disabled && "cursor-help", className].filter(Boolean).join(" ") || undefined}
    >
      {children}
      {!visible && !disabled && <span id={tooltipId} role="tooltip" className="sr-only">{content}</span>}
      {portal}
    </span>
  );
};
