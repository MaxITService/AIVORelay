import { useCallback, useRef } from "react";
import { prefersReducedMotion } from "@/lib/motion";

// Time for the glow to close ~63% of the gap to the cursor; higher = more inertia.
const FOLLOW_TIME_CONSTANT_MS = 120;
// How far outside a card the cursor still lights its border. Matches the glow
// gradient in App.css: a 320px circle that fades out at 45% = 144px.
const REACH_PX = 144;
const SETTLE_DISTANCE_PX = 0.5;
const MAX_FRAME_DELTA_MS = 64;

interface Spotlight {
  layer: HTMLElement;
  active: boolean;
  x: number;
  y: number;
  targetX: number;
  targetY: number;
  frame: number;
  lastTime: number;
}

// One document listener drives every mounted card, so the border can light up
// while the cursor is still approaching from outside.
const spotlights = new Set<Spotlight>();
let pointer: { x: number; y: number } | null = null;

const writePosition = (spot: Spotlight) => {
  spot.layer.style.setProperty("--card-spotlight-x", `${spot.x}px`);
  spot.layer.style.setProperty("--card-spotlight-y", `${spot.y}px`);
};

const jumpToTarget = (spot: Spotlight) => {
  cancelAnimationFrame(spot.frame);
  spot.frame = 0;
  spot.x = spot.targetX;
  spot.y = spot.targetY;
  writePosition(spot);
};

const follow = (spot: Spotlight) => {
  // A running loop picks up the new target on its next frame.
  if (spot.frame) return;
  spot.lastTime = 0;

  const step = (time: number) => {
    const delta = spot.lastTime
      ? Math.min(time - spot.lastTime, MAX_FRAME_DELTA_MS)
      : 16;
    spot.lastTime = time;
    const blend = 1 - Math.exp(-delta / FOLLOW_TIME_CONSTANT_MS);
    spot.x += (spot.targetX - spot.x) * blend;
    spot.y += (spot.targetY - spot.y) * blend;

    const settled =
      Math.abs(spot.targetX - spot.x) < SETTLE_DISTANCE_PX &&
      Math.abs(spot.targetY - spot.y) < SETTLE_DISTANCE_PX;
    if (settled) {
      spot.x = spot.targetX;
      spot.y = spot.targetY;
      spot.frame = 0;
    } else {
      spot.frame = requestAnimationFrame(step);
    }
    writePosition(spot);
  };

  spot.frame = requestAnimationFrame(step);
};

const deactivate = (spot: Spotlight) => {
  if (!spot.active) return;
  spot.active = false;
  delete spot.layer.dataset.active;
};

const updateSpotlight = (spot: Spotlight, reduceMotion: boolean) => {
  if (!pointer) return;
  const rect = spot.layer.getBoundingClientRect();
  const dx = Math.max(rect.left - pointer.x, 0, pointer.x - rect.right);
  const dy = Math.max(rect.top - pointer.y, 0, pointer.y - rect.bottom);
  const inReach =
    rect.width > 0 && rect.height > 0 && Math.hypot(dx, dy) <= REACH_PX;
  if (!inReach) {
    deactivate(spot);
    return;
  }

  spot.targetX = pointer.x - rect.left;
  spot.targetY = pointer.y - rect.top;

  // Coming into reach: jump to the cursor; the glow is invisible at the edge
  // of its reach, so the jump never shows.
  if (!spot.active || reduceMotion) {
    spot.active = true;
    spot.layer.dataset.active = "";
    jumpToTarget(spot);
    return;
  }
  follow(spot);
};

const updateAll = () => {
  if (!pointer) return;
  const reduceMotion = prefersReducedMotion();
  for (const spot of spotlights) updateSpotlight(spot, reduceMotion);
};

const handlePointerMove = (event: PointerEvent) => {
  if (event.pointerType === "touch") return;
  pointer = { x: event.clientX, y: event.clientY };
  updateAll();
};

// relatedTarget is null when the pointer leaves the window.
const handlePointerOut = (event: PointerEvent) => {
  if (event.relatedTarget) return;
  pointer = null;
  for (const spot of spotlights) deactivate(spot);
};

const listenerOptions = { capture: true, passive: true } as const;

const register = (spot: Spotlight) => {
  if (spotlights.size === 0) {
    document.addEventListener(
      "pointermove",
      handlePointerMove,
      listenerOptions,
    );
    document.addEventListener("pointerout", handlePointerOut, listenerOptions);
    // Cards move under a still cursor while the page scrolls.
    document.addEventListener("scroll", updateAll, listenerOptions);
  }
  spotlights.add(spot);
  updateSpotlight(spot, prefersReducedMotion());
};

const unregister = (spot: Spotlight) => {
  cancelAnimationFrame(spot.frame);
  spotlights.delete(spot);
  if (spotlights.size === 0) {
    document.removeEventListener(
      "pointermove",
      handlePointerMove,
      listenerOptions,
    );
    document.removeEventListener(
      "pointerout",
      handlePointerOut,
      listenerOptions,
    );
    document.removeEventListener("scroll", updateAll, listenerOptions);
  }
};

/**
 * Cursor spotlight for a card border: lights up as the cursor approaches and
 * trails it with a little inertia. Returns a ref for the
 * `.settings-card__spotlight` layer; the position is written straight to it as
 * `--card-spotlight-x/y` and `data-active` marks it visible, so pointer
 * movement never re-renders the card.
 */
export const useCardSpotlight = () => {
  const spotRef = useRef<Spotlight | null>(null);

  return useCallback((layer: HTMLElement | null) => {
    if (spotRef.current) {
      unregister(spotRef.current);
      spotRef.current = null;
    }
    if (!layer) return;

    const spot: Spotlight = {
      layer,
      active: false,
      x: 0,
      y: 0,
      targetX: 0,
      targetY: 0,
      frame: 0,
      lastTime: 0,
    };
    spotRef.current = spot;
    register(spot);
  }, []);
};
