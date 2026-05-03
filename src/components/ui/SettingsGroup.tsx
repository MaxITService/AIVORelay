import React from "react";
import { ChevronDown } from "lucide-react";

interface SettingsGroupProps {
  id?: string;
  title?: string;
  description?: string;
  collapsible?: boolean;
  collapsed?: boolean;
  collapseLabel?: string;
  expandLabel?: string;
  onCollapsedChange?: (collapsed: boolean) => void;
  children: React.ReactNode;
}

export const SettingsGroup: React.FC<SettingsGroupProps> = ({
  id,
  title,
  description,
  collapsible = false,
  collapsed,
  collapseLabel = "Collapse",
  expandLabel = "Expand",
  onCollapsedChange,
  children,
}) => {
  const contentId = React.useId();
  const [uncontrolledCollapsed, setUncontrolledCollapsed] = React.useState(false);
  const isCollapsible = Boolean(title && collapsible);
  const isCollapsed = isCollapsible
    ? (collapsed ?? uncontrolledCollapsed)
    : false;
  const collapsedActionLabel = isCollapsed ? expandLabel : collapseLabel;

  const toggleCollapsed = React.useCallback(() => {
    const nextCollapsed = !isCollapsed;
    if (collapsed === undefined) {
      setUncontrolledCollapsed(nextCollapsed);
    }
    onCollapsedChange?.(nextCollapsed);
  }, [collapsed, isCollapsed, onCollapsedChange]);

  return (
    <div id={id} className="space-y-4">
      {title && (
        <div className="px-1 pt-2">
          {isCollapsible ? (
            <button
              type="button"
              onClick={toggleCollapsed}
              className="flex w-full items-start justify-between gap-3 rounded-lg px-1 py-1 text-left transition-colors hover:bg-white/[0.03] focus:outline-none focus:ring-2 focus:ring-[#ff4d8d]/35"
              aria-expanded={!isCollapsed}
              aria-controls={contentId}
              title={collapsedActionLabel}
            >
              <span>
                <span className="block text-xs font-bold text-[#ff4d8d] uppercase tracking-widest">
                  {title}
                </span>
                {description && (
                  <span className="mt-1.5 block text-xs leading-relaxed text-[#a0a0a0]">
                    {description}
                  </span>
                )}
              </span>
              <span className="mt-0.5 flex shrink-0 items-center gap-1.5 rounded-md border border-[#ff4d8d]/30 bg-[#ff4d8d]/10 px-2 py-1 text-[10px] font-semibold uppercase tracking-[0.12em] text-[#ff8ebb]">
                <span>{collapsedActionLabel}</span>
                <ChevronDown
                  className={`h-3.5 w-3.5 shrink-0 transition-transform ${
                    isCollapsed ? "-rotate-90" : "rotate-0"
                  }`}
                  aria-hidden="true"
                />
              </span>
              <span className="sr-only">
                {collapsedActionLabel}
              </span>
            </button>
          ) : (
            <h2 className="text-xs font-bold text-[#ff4d8d] uppercase tracking-widest">
              {title}
            </h2>
          )}
          {description && !isCollapsible && (
            <p className="text-xs text-[#a0a0a0] mt-1.5 leading-relaxed">{description}</p>
          )}
        </div>
      )}
      {!isCollapsed && (
        <div
          id={contentId}
          className="glass-panel-subtle rounded-xl overflow-visible border border-white/[0.03]"
        >
          <div className="divide-y divide-white/[0.05]">{children}</div>
        </div>
      )}
    </div>
  );
};
