import React, {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { computeTooltipLayout, TooltipLayout } from "./tooltipPositioning";

interface InfoTooltipProps {
  content: React.ReactNode;
  position?: "top" | "bottom";
}

/**
 * Standalone info tooltip icon (i) that displays a styled tooltip on hover/click.
 * Extracted from SettingContainer for use in custom layouts.
 */
export const InfoTooltip: React.FC<InfoTooltipProps> = ({
  content,
  position = "top",
}) => {
  const [showTooltip, setShowTooltip] = useState(false);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const tooltipContentRef = useRef<HTMLDivElement>(null);
  const [tooltipLayout, setTooltipLayout] = useState<TooltipLayout | null>(
    null
  );

  const updateTooltipLayout = useCallback(() => {
    if (!showTooltip || !tooltipRef.current || !tooltipContentRef.current) {
      return;
    }

    const triggerRect = tooltipRef.current.getBoundingClientRect();
    const tooltipRect = tooltipContentRef.current.getBoundingClientRect();

    setTooltipLayout(
      computeTooltipLayout({
        triggerRect,
        tooltipRect,
        preferredPlacement: position,
        viewportWidth: window.innerWidth,
        viewportHeight: window.innerHeight,
      })
    );
  }, [showTooltip, position]);

  // Handle click outside to close tooltip
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        tooltipRef.current &&
        tooltipRef.current.contains(event.target as Node)
      ) {
        return;
      }
      setShowTooltip(false);
    };

    if (showTooltip) {
      document.addEventListener("mousedown", handleClickOutside);
      window.addEventListener("scroll", updateTooltipLayout, true);
      window.addEventListener("resize", updateTooltipLayout);

      return () => {
        document.removeEventListener("mousedown", handleClickOutside);
        window.removeEventListener("scroll", updateTooltipLayout, true);
        window.removeEventListener("resize", updateTooltipLayout);
      };
    }
  }, [showTooltip, updateTooltipLayout]);

  // Update layout when tooltip opens and content is available
  useLayoutEffect(() => {
    if (showTooltip) {
      updateTooltipLayout();
    } else {
      setTooltipLayout(null);
    }
  }, [showTooltip, updateTooltipLayout, content]);

  const renderTooltipPortal = () => {
    if (!showTooltip) return null;

    const activePlacement = tooltipLayout?.placement ?? position;
    const wrapperStyle: React.CSSProperties = tooltipLayout
      ? {
          top: tooltipLayout.top,
          left: tooltipLayout.left,
        }
      : {
          top: 0,
          left: 0,
          visibility: "hidden",
        };

    return createPortal(
      <div
        className="fixed z-[9999] pointer-events-none"
        style={wrapperStyle}
      >
        <div
          ref={tooltipContentRef}
          className="relative px-4 py-2.5 bg-[#323232]/98 backdrop-blur-xl border border-[#4a4a4a] rounded-lg shadow-[0_8px_24px_rgba(0,0,0,0.5)] max-w-xs min-w-[200px] whitespace-normal animate-in fade-in-0 zoom-in-95 duration-200"
        >
          <p className="text-sm text-[#e8e8e8] text-center leading-relaxed">
            {content}
          </p>
          {/* Arrow */}
          <div
            className={`absolute w-0 h-0 border-l-[6px] border-r-[6px] border-[6px] border-l-transparent border-r-transparent ${
              activePlacement === "top"
                ? "top-full border-t-[#4a4a4a] border-b-transparent"
                : "bottom-full border-b-[#4a4a4a] border-t-transparent"
            }`}
            style={{
              left: tooltipLayout ? tooltipLayout.arrowLeft : "50%",
              transform: "translateX(-50%)",
            }}
          ></div>
        </div>
      </div>,
      document.body
    );
  };

  return (
    <div
      ref={tooltipRef}
      className="relative flex items-center justify-center p-1"
      onMouseEnter={() => setShowTooltip(true)}
      onMouseLeave={() => setShowTooltip(false)}
    >
      <svg
        className="w-4 h-4 text-[#707070] cursor-help hover:text-[#ff4d8d] transition-colors duration-200 select-none"
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
        aria-label="More information"
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={2}
          d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
        />
      </svg>
      {renderTooltipPortal()}
    </div>
  );
};
