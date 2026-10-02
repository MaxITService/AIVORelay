import { useEffect, useState } from "react";
import { POPOVER_EXIT_DURATION_MS } from "@/lib/motion";

/**
 * Keeps a popover mounted while its exit animation plays.
 * Render the popover while `isMounted` and pass `state` to its `data-state`.
 */
export const usePresence = (
  open: boolean,
  exitDurationMs: number = POPOVER_EXIT_DURATION_MS,
) => {
  const [isExiting, setIsExiting] = useState(false);
  const [wasOpen, setWasOpen] = useState(open);

  if (open !== wasOpen) {
    setWasOpen(open);
    setIsExiting(!open);
  }

  useEffect(() => {
    if (!isExiting) return;
    const timer = window.setTimeout(() => setIsExiting(false), exitDurationMs);
    return () => window.clearTimeout(timer);
  }, [isExiting, exitDurationMs]);

  return {
    isMounted: open || isExiting,
    state: (open ? "open" : "closed") as "open" | "closed",
  };
};
