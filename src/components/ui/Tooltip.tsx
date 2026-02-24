import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { computeTooltipLayout, TooltipLayout } from "./tooltipPositioning";

interface TooltipProps {
  content: React.ReactNode;
  children: React.ReactNode;
  position?: "top" | "bottom";
}

/**
 * Generic tooltip wrapper — hover over children to see the tooltip.
 * Reuses the same portal + positioning logic as InfoTooltip / SettingContainer.
 */
export const Tooltip: React.FC<TooltipProps> = ({
  content,
  children,
  position = "top",
}) => {
  const [show, setShow] = useState(false);
  const triggerRef = useRef<HTMLSpanElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const [layout, setLayout] = useState<TooltipLayout | null>(null);

  const updateLayout = useCallback(() => {
    if (!show || !triggerRef.current || !tooltipRef.current) return;
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
  }, [show, position]);

  useEffect(() => {
    if (!show) return;
    window.addEventListener("scroll", updateLayout, true);
    window.addEventListener("resize", updateLayout);
    return () => {
      window.removeEventListener("scroll", updateLayout, true);
      window.removeEventListener("resize", updateLayout);
    };
  }, [show, updateLayout]);

  useLayoutEffect(() => {
    if (show) updateLayout();
    else setLayout(null);
  }, [show, updateLayout, content]);

  const portal = show
    ? createPortal(
        <div
          className="fixed z-[9999] pointer-events-none"
          style={
            layout
              ? { top: layout.top, left: layout.left }
              : { top: 0, left: 0, visibility: "hidden" as const }
          }
        >
          <div
            ref={tooltipRef}
            className="relative px-4 py-2.5 bg-[#323232]/98 backdrop-blur-xl border border-[#4a4a4a] rounded-lg shadow-[0_8px_24px_rgba(0,0,0,0.5)] max-w-xs min-w-[200px] whitespace-normal animate-in fade-in-0 zoom-in-95 duration-200"
          >
            <p className="text-sm text-[#e8e8e8] text-center leading-relaxed">
              {content}
            </p>
            <div
              className={`absolute w-0 h-0 border-l-[6px] border-r-[6px] border-[6px] border-l-transparent border-r-transparent ${
                (layout?.placement ?? position) === "top"
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
      onMouseEnter={() => setShow(true)}
      onMouseLeave={() => setShow(false)}
      className="cursor-help"
    >
      {children}
      {portal}
    </span>
  );
};
