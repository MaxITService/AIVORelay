import { useCallback, useEffect, useRef, useState } from "react";
import { TOOLTIP_OPEN_DELAY_MS, TOOLTIP_SKIP_DELAY_MS } from "@/lib/motion";

// Shared by every tooltip, so moving the cursor to a neighbour skips the delay.
let lastHiddenAt = Number.NEGATIVE_INFINITY;

/**
 * Open state for hover tooltips. `show` waits `TOOLTIP_OPEN_DELAY_MS` so a
 * passing cursor does not pop the tooltip up, unless another tooltip was just
 * open: then it opens at once. `showNow` always skips the delay (e.g. on click).
 */
export const useTooltipOpen = () => {
  const [open, setOpen] = useState(false);
  const openRef = useRef(false);
  const timerRef = useRef<number | undefined>(undefined);

  const setOpenState = useCallback((next: boolean) => {
    openRef.current = next;
    setOpen(next);
  }, []);

  const showNow = useCallback(() => {
    window.clearTimeout(timerRef.current);
    setOpenState(true);
  }, [setOpenState]);

  const show = useCallback(() => {
    window.clearTimeout(timerRef.current);
    if (
      openRef.current ||
      performance.now() - lastHiddenAt < TOOLTIP_SKIP_DELAY_MS
    ) {
      setOpenState(true);
      return;
    }
    timerRef.current = window.setTimeout(
      () => setOpenState(true),
      TOOLTIP_OPEN_DELAY_MS,
    );
  }, [setOpenState]);

  const hide = useCallback(() => {
    window.clearTimeout(timerRef.current);
    if (!openRef.current) return;
    lastHiddenAt = performance.now();
    setOpenState(false);
  }, [setOpenState]);

  useEffect(() => () => window.clearTimeout(timerRef.current), []);

  return { open, show, showNow, hide };
};
