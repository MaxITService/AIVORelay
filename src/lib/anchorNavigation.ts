import { COLLAPSE_MAX_DURATION_MS, prefersReducedMotion } from "./motion";

const ANCHOR_HIGHLIGHT_CLASS = "settings-anchor-highlight";
const ANCHOR_HIGHLIGHT_DURATION_MS = 1800;

let activeAnchor: HTMLElement | null = null;
let highlightTimer: number | null = null;
let navigationGeneration = 0;
let pendingRevealTimer: number | null = null;

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
  isCurrent: () => boolean = () => true,
): void => {
  if (!isCurrent()) return;

  anchor.scrollIntoView({
    behavior: prefersReducedMotion() ? "auto" : "smooth",
    block,
  });

  window.requestAnimationFrame(() => {
    if (isCurrent() && document.contains(anchor)) {
      highlightAnchor(anchor);
      anchor.focus({ preventScroll: true });
    }
  });
};

/** Only reveal the disclosure belonging to the requested anchor. */
const COLLAPSED_TOGGLE_SELECTOR =
  '[data-settings-search-reveal-for][aria-expanded="false"]';

const findCollapsedToggle = (anchor: HTMLElement): HTMLElement | undefined =>
  Array.from(
    anchor.querySelectorAll<HTMLElement>(COLLAPSED_TOGGLE_SELECTOR),
  ).find((button) => button.dataset.settingsSearchRevealFor === anchor.id);

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
  navigationGeneration += 1;
  const currentGeneration = navigationGeneration;
  const isCurrent = () => currentGeneration === navigationGeneration;

  if (pendingRevealTimer !== null) {
    window.clearTimeout(pendingRevealTimer);
    pendingRevealTimer = null;
  }

  activateSection();

  if (updateHash) {
    window.history.replaceState(null, "", `#${targetId}`);
  }

  let attempts = 0;
  const revealAnchor = () => {
    pendingRevealTimer = null;
    if (!isCurrent()) return;

    if (!document.getElementById(readyId)) {
      attempts += 1;
      if (attempts <= 20) {
        pendingRevealTimer = window.setTimeout(revealAnchor, 50);
      }
      return;
    }

    const expansionTarget = expandId
      ? document.getElementById(expandId)
      : null;
    if (expandId && !expansionTarget && attempts < 20) {
      attempts += 1;
      pendingRevealTimer = window.setTimeout(revealAnchor, 50);
      return;
    }

    const collapsedDetails =
      expansionTarget instanceof HTMLDetailsElement && !expansionTarget.open
        ? expansionTarget
        : null;
    if (collapsedDetails) collapsedDetails.open = true;

    const explicitRevealButton =
      expansionTarget instanceof HTMLButtonElement &&
      expansionTarget.dataset.settingsSearchReveal === "true"
        ? expansionTarget
        : null;
    const collapsedToggle =
      explicitRevealButton ??
      (expansionTarget ? findCollapsedToggle(expansionTarget) : undefined);
    collapsedToggle?.click();

    let targetAttempts = 0;
    const revealExpandedTarget = () => {
      pendingRevealTimer = null;
      if (!isCurrent()) return;

      const target = document.getElementById(targetId);
      if (!target && collapsedToggle && targetAttempts < 20) {
        targetAttempts += 1;
        pendingRevealTimer = window.setTimeout(revealExpandedTarget, 50);
        return;
      }

      const targetToggle =
        target && target !== expansionTarget
          ? findCollapsedToggle(target)
          : null;
      targetToggle?.click();

      const destination =
        target ??
        (fallbackId ? document.getElementById(fallbackId) : null) ??
        expansionTarget;
      if (!destination) return;

      let openedDetails = false;
      let parentDetails = destination.closest("details");
      while (parentDetails) {
        if (!parentDetails.open) {
          parentDetails.open = true;
          openedDetails = true;
        }
        parentDetails = parentDetails.parentElement?.closest("details") ?? null;
      }

      // Let expand animations finish so the scroll lands on the final layout.
      if (collapsedDetails || collapsedToggle || targetToggle || openedDetails) {
        pendingRevealTimer = window.setTimeout(() => {
          pendingRevealTimer = null;
          scrollAndFocusAnchor(destination, block, isCurrent);
        }, prefersReducedMotion() ? 0 : COLLAPSE_MAX_DURATION_MS);
        return;
      }

      scrollAndFocusAnchor(destination, block, isCurrent);
    };

    window.requestAnimationFrame(() => {
      if (!isCurrent()) return;
      window.requestAnimationFrame(revealExpandedTarget);
    });
  };

  pendingRevealTimer = window.setTimeout(revealAnchor, 0);
};

/** Opens a settings search target: its anchor, else its fallback, after expanding its parent. */
export const navigateToSettingsSearchTarget = (
  activateSection: () => void,
  sectionId: string,
  anchorId?: string,
  expandAnchorId?: string,
  fallbackAnchorId?: string,
): void => {
  const sectionAnchorId = `settings-section-${sectionId}`;
  navigateToSettingsAnchor({
    activateSection,
    targetId: anchorId ?? sectionAnchorId,
    readyId: sectionAnchorId,
    fallbackId: fallbackAnchorId ?? expandAnchorId ?? sectionAnchorId,
    expandId: expandAnchorId,
    block: "center",
  });
};
