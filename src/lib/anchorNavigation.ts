const ANCHOR_HIGHLIGHT_CLASS = "settings-anchor-highlight";
const ANCHOR_HIGHLIGHT_DURATION_MS = 1800;

let activeAnchor: HTMLElement | null = null;
let highlightTimer: number | null = null;

const prefersReducedMotion = (): boolean =>
  typeof window !== "undefined" &&
  window.matchMedia("(prefers-reduced-motion: reduce)").matches;

const highlightAnchor = (anchor: HTMLElement): void => {
  if (activeAnchor && activeAnchor !== anchor) {
    activeAnchor.classList.remove(ANCHOR_HIGHLIGHT_CLASS);
  }

  anchor.classList.remove(ANCHOR_HIGHLIGHT_CLASS);
  void anchor.offsetWidth;
  anchor.classList.add(ANCHOR_HIGHLIGHT_CLASS);
  activeAnchor = anchor;

  if (highlightTimer !== null) {
    window.clearTimeout(highlightTimer);
  }

  highlightTimer = window.setTimeout(() => {
    anchor.classList.remove(ANCHOR_HIGHLIGHT_CLASS);
    if (activeAnchor === anchor) {
      activeAnchor = null;
    }
    highlightTimer = null;
  }, ANCHOR_HIGHLIGHT_DURATION_MS);
};

export const scrollAndFocusAnchor = (
  anchor: HTMLElement,
  block: ScrollLogicalPosition = "start",
): void => {
  highlightAnchor(anchor);
  anchor.scrollIntoView({
    behavior: prefersReducedMotion() ? "auto" : "smooth",
    block,
  });

  window.requestAnimationFrame(() => {
    if (document.contains(anchor)) {
      anchor.focus({ preventScroll: true });
    }
  });
};
