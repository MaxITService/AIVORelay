export type TooltipPlacement = "top" | "bottom";

export interface TooltipLayout {
  top: number;
  left: number;
  placement: TooltipPlacement;
  arrowLeft: number;
}

interface ComputeTooltipLayoutOptions {
  triggerRect: DOMRect;
  tooltipRect: DOMRect;
  preferredPlacement: TooltipPlacement;
  viewportWidth: number;
  viewportHeight: number;
  edgePadding?: number;
  offset?: number;
  arrowPadding?: number;
}

const clamp = (value: number, min: number, max: number) =>
  Math.min(Math.max(value, min), max);

export function computeTooltipLayout({
  triggerRect,
  tooltipRect,
  preferredPlacement,
  viewportWidth,
  viewportHeight,
  edgePadding = 10,
  offset = 10,
  arrowPadding = 14,
}: ComputeTooltipLayoutOptions): TooltipLayout {
  const requiredHeight = tooltipRect.height + offset;
  const availableAbove = triggerRect.top - edgePadding;
  const availableBelow = viewportHeight - triggerRect.bottom - edgePadding;
  const canPlaceTop = availableAbove >= requiredHeight;
  const canPlaceBottom = availableBelow >= requiredHeight;

  let placement = preferredPlacement;
  if (preferredPlacement === "top" && !canPlaceTop && canPlaceBottom) {
    placement = "bottom";
  } else if (
    preferredPlacement === "bottom" &&
    !canPlaceBottom &&
    canPlaceTop
  ) {
    placement = "top";
  } else if (!canPlaceTop && !canPlaceBottom) {
    placement = availableBelow >= availableAbove ? "bottom" : "top";
  }

  const anchorX = triggerRect.left + triggerRect.width / 2;
  const minLeft = edgePadding;
  const maxLeft = Math.max(edgePadding, viewportWidth - edgePadding - tooltipRect.width);
  const left = clamp(anchorX - tooltipRect.width / 2, minLeft, maxLeft);

  const naturalTop =
    placement === "top"
      ? triggerRect.top - offset - tooltipRect.height
      : triggerRect.bottom + offset;
  const minTop = edgePadding;
  const maxTop = Math.max(edgePadding, viewportHeight - edgePadding - tooltipRect.height);
  const top = clamp(naturalTop, minTop, maxTop);

  const minArrowLeft = arrowPadding;
  const maxArrowLeft = Math.max(minArrowLeft, tooltipRect.width - arrowPadding);
  const arrowLeft = clamp(anchorX - left, minArrowLeft, maxArrowLeft);

  return {
    top,
    left,
    placement,
    arrowLeft,
  };
}
