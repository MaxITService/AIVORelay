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
  anchor.scrollIntoView({
    behavior: prefersReducedMotion() ? "auto" : "smooth",
    block,
  });

  window.requestAnimationFrame(() => {
    if (document.contains(anchor)) {
      highlightAnchor(anchor);
      anchor.focus({ preventScroll: true });
    }
  });
};

interface NavigateToSettingsAnchorOptions {
  activateSection: () => void;
  targetId: string;
  readyId?: string;
  fallbackId?: string;
  expandId?: string;
  block?: ScrollLogicalPosition;
  updateHash?: boolean;
}

export const navigateToSettingsAnchor = ({
  activateSection,
  targetId,
  readyId = targetId,
  fallbackId,
  expandId,
  block = "start",
  updateHash = true,
}: NavigateToSettingsAnchorOptions): void => {
  activateSection();

  if (updateHash) {
    window.history.replaceState(null, "", `#${targetId}`);
  }

  let attempts = 0;
  const revealAnchor = () => {
    if (!document.getElementById(readyId)) {
      attempts += 1;
      if (attempts <= 20) window.setTimeout(revealAnchor, 50);
      return;
    }

    const expansionTarget = expandId
      ? document.getElementById(expandId)
      : null;
    const collapsedToggle = expansionTarget?.querySelector<HTMLButtonElement>(
      'button[aria-expanded="false"]',
    );
    collapsedToggle?.click();

    window.requestAnimationFrame(() => {
      window.requestAnimationFrame(() => {
        const target =
          document.getElementById(targetId) ??
          expansionTarget ??
          (fallbackId ? document.getElementById(fallbackId) : null);
        if (target) scrollAndFocusAnchor(target, block);
      });
    });
  };

  window.setTimeout(revealAnchor, 0);
};
