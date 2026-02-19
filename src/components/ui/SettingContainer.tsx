import React, {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { computeTooltipLayout, TooltipLayout } from "./tooltipPositioning";

interface SettingContainerProps {
  title: React.ReactNode;
  description: React.ReactNode;
  children: React.ReactNode;
  descriptionMode?: "inline" | "tooltip" | "none";

  grouped?: boolean;
  layout?: "horizontal" | "stacked";
  disabled?: boolean;
  tooltipPosition?: "top" | "bottom";
  compact?: boolean;
}

export const SettingContainer: React.FC<SettingContainerProps> = ({
  title,
  description,
  children,
  descriptionMode = "tooltip",
  grouped = false,
  layout = "horizontal",
  disabled = false,
  tooltipPosition = "top",
  compact = false,
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
        preferredPlacement: tooltipPosition,
        viewportWidth: window.innerWidth,
        viewportHeight: window.innerHeight,
      })
    );
  }, [showTooltip, tooltipPosition]);

  // Handle click outside to close tooltip
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      // Check if click is on the trigger icon itself
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
  }, [showTooltip, updateTooltipLayout, description]);

  const toggleTooltip = (e: React.MouseEvent) => {
    e.stopPropagation();
    setShowTooltip(!showTooltip);
  };

  const basePadding = compact ? "px-6 py-2.5" : "px-6 py-4";
  const containerClasses = grouped
    ? basePadding
    : `${basePadding} rounded-lg bg-[#2b2b2b]/40 border border-[#2f2f2f] hover:bg-[#323232]/50 hover:border-[#3c3c3c] transition-all duration-200`;

  const renderTooltipPortal = () => {
    if (!showTooltip) return null;

    const activePlacement = tooltipLayout?.placement ?? tooltipPosition;
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
            {description}
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

  const tooltipIcon = (
    <div
      ref={tooltipRef}
      className="relative flex items-center justify-center p-1"
      onMouseEnter={() => setShowTooltip(true)}
      onMouseLeave={() => setShowTooltip(false)}
      onClick={toggleTooltip}
    >
      <svg
        className="w-4 h-4 text-[#707070] cursor-help hover:text-[#ff4d8d] transition-colors duration-200 select-none"
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
        aria-label="More information"
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            toggleTooltip(e as any);
          }
        }}
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

  if (layout === "stacked") {
    if (descriptionMode === "tooltip") {
      return (
        <div className={containerClasses}>
          <div className="flex items-center gap-2 mb-2">
            <h3
              className={`text-sm font-medium ${disabled ? "opacity-50" : ""}`}
            >
              {title}
            </h3>
            {tooltipIcon}
          </div>
          <div className="w-full">{children}</div>
        </div>
      );
    }

    return (
      <div className={containerClasses}>
        <div className={compact ? "mb-1" : "mb-2"}>
          <h3 className={`text-sm font-medium ${disabled ? "opacity-50" : ""}`}>
            {title}
          </h3>
          {descriptionMode !== "none" && (
            <p className={`text-sm ${disabled ? "opacity-50" : ""}`}>
              {description}
            </p>
          )}
        </div>
        <div className="w-full">{children}</div>
      </div>
    );
  }

  // Horizontal layout (default) - responsive: stacks on small screens, side-by-side on md+
  const horizontalContainerClasses = grouped
    ? `flex flex-col ${compact ? "gap-1.5" : "gap-2"} md:flex-row md:items-center md:justify-between ${basePadding}`
    : `flex flex-col ${compact ? "gap-1.5" : "gap-2"} md:flex-row md:items-center md:justify-between ${basePadding} rounded-lg bg-[#2b2b2b]/40 border border-[#2f2f2f] hover:bg-[#323232]/50 hover:border-[#3c3c3c] transition-all duration-200`;

  if (descriptionMode === "tooltip") {
    return (
      <div className={horizontalContainerClasses}>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <h3
              className={`text-sm font-medium ${disabled ? "opacity-50" : ""}`}
            >
              {title}
            </h3>
            {tooltipIcon}
          </div>
        </div>
        <div className="relative">{children}</div>
      </div>
    );
  }

  return (
    <div className={horizontalContainerClasses}>
      <div className="flex-1 min-w-0">
        <h3 className={`text-sm font-medium ${disabled ? "opacity-50" : ""}`}>
          {title}
        </h3>
        {descriptionMode !== "none" && (
          <p className={`text-sm ${disabled ? "opacity-50" : ""}`}>
            {description}
          </p>
        )}
      </div>
      <div className="relative">{children}</div>
    </div>
  );
};
