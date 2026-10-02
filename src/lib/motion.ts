/**
 * Shared timing for disclosure and popover motion.
 * Keep in sync with the `--motion-*` custom properties in App.css.
 */
export const COLLAPSE_MIN_DURATION_MS = 180;
export const COLLAPSE_MAX_DURATION_MS = 450;
export const POPOVER_EXIT_DURATION_MS = 120;

export const prefersReducedMotion = (): boolean =>
  typeof window !== "undefined" &&
  window.matchMedia("(prefers-reduced-motion: reduce)").matches;

/**
 * Height-aware collapse duration (same curve as MUI's auto height timing):
 * tall sections get more time so they glide instead of jumping.
 */
export const getCollapseDurationMs = (heightPx: number): number => {
  if (prefersReducedMotion()) return 0;
  const units = Math.max(0, heightPx) / 36;
  const auto = Math.round((4 + 15 * units ** 0.25 + units / 5) * 10);
  return Math.min(
    COLLAPSE_MAX_DURATION_MS,
    Math.max(COLLAPSE_MIN_DURATION_MS, auto),
  );
};
