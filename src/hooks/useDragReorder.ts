import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { PointerEvent as ReactPointerEvent, RefObject } from "react";
import { prefersReducedMotion } from "@/lib/motion";

// Drag-to-reorder for a vertical list. The picked item lifts and follows the
// pointer, the others slide apart to open the gap it will drop into, the
// scroll container auto-scrolls near its edges, Escape cancels, and on release
// every item glides into its slot. Items keep their DOM order while dragging:
// the preview only uses the `translate` property, so layout stays stable and
// the drop slot never flickers.
//
// Styling hooks: the scroll container gets `data-reorder-drag` for the whole
// drag and settle, and the dragged item gets `data-drag-state="lifted"`, then
// "settling" while it glides home.

const DRAG_THRESHOLD_PX = 5;
const AUTOSCROLL_EDGE_PX = 36;
const AUTOSCROLL_MAX_STEP_PX = 14;
const SETTLE_DURATION_MS = 360;
// Leaves room for other list effects (e.g. an active-item indicator) to finish
const SETTLE_CLEANUP_MS = 520;

const LIFT_TRANSITION = "box-shadow 180ms ease, background-color 180ms ease";
const SHIFT_TRANSITION = "translate 240ms cubic-bezier(0.2, 0.8, 0.2, 1)";
const SETTLE_TRANSITION = [
  `translate ${SETTLE_DURATION_MS}ms cubic-bezier(0.34, 1.3, 0.64, 1)`,
  "box-shadow 220ms ease",
  "background-color 220ms ease",
].join(", ");

interface Slot {
  id: string;
  el: HTMLElement;
  top: number;
  height: number;
}

interface DragSession {
  id: string;
  pointerId: number;
  startY: number;
  lastY: number;
  started: boolean;
  slots: Slot[];
  fromIndex: number;
  dropIndex: number;
  gap: number;
  scroller: HTMLElement | null;
  startScrollTop: number;
  frame: number;
}

interface PendingSettle {
  tops: Map<string, number>;
  liftedId: string;
  scroller: HTMLElement | null;
}

interface DragReorderOptions {
  order: string[];
  itemRefs: { current: Map<string, HTMLElement> };
  scrollRef: RefObject<HTMLElement | null>;
  onReorder: (next: string[]) => void;
  /** Called on a press that never moved far enough to become a drag. */
  onActivate: (id: string) => void;
}

const moveItem = (ids: string[], from: number, to: number) => {
  const next = [...ids];
  const [moved] = next.splice(from, 1);
  next.splice(to, 0, moved);
  return next;
};

// One engine per list; it reads the latest options through `options.current`
function createDragEngine(options: { current: DragReorderOptions }) {
  let session: DragSession | null = null;
  let pendingSettle: PendingSettle | null = null;
  let settleTimer = 0;

  const getItem = (id: string) => options.current.itemRefs.current.get(id);

  // Opens the drop gap: every other item moves to where it will sit after the drop
  const applyShifts = (s: DragSession) => {
    const byId = new Map(s.slots.map((slot) => [slot.id, slot]));
    const ids = s.slots.map((slot) => slot.id);
    let top = s.slots[0].top;
    for (const id of moveItem(ids, s.fromIndex, s.dropIndex)) {
      const slot = byId.get(id)!;
      if (id !== s.id) {
        const shift = top - slot.top;
        slot.el.style.translate = shift ? `0 ${shift}px` : "";
      }
      top += slot.height + s.gap;
    }
  };

  const positionDrag = (s: DragSession) => {
    const from = s.slots[s.fromIndex];
    const first = s.slots[0];
    const last = s.slots[s.slots.length - 1];
    const scrolled = s.scroller ? s.scroller.scrollTop - s.startScrollTop : 0;
    const delta = Math.min(
      Math.max(s.lastY - s.startY + scrolled, first.top - from.top),
      last.top + last.height - from.top - from.height,
    );
    from.el.style.translate = `0 ${delta}px`;

    // Drop slot = how many other items have their centre above the dragged one's
    const center = from.top + delta + from.height / 2;
    let dropIndex = 0;
    for (const slot of s.slots) {
      if (slot !== from && slot.top + slot.height / 2 < center) dropIndex++;
    }
    if (dropIndex !== s.dropIndex) {
      s.dropIndex = dropIndex;
      applyShifts(s);
    }
  };

  const autoScroll = (s: DragSession) => {
    if (!s.scroller) return;
    const rect = s.scroller.getBoundingClientRect();
    const intoTop = rect.top + AUTOSCROLL_EDGE_PX - s.lastY;
    const intoBottom = s.lastY - (rect.bottom - AUTOSCROLL_EDGE_PX);
    const depth = Math.max(intoTop, intoBottom);
    if (depth <= 0) return;
    const step =
      AUTOSCROLL_MAX_STEP_PX * Math.min(1, depth / AUTOSCROLL_EDGE_PX);
    s.scroller.scrollTop += intoTop > 0 ? -step : step;
  };

  const tick = () => {
    if (!session?.started) return;
    autoScroll(session);
    positionDrag(session);
    session.frame = requestAnimationFrame(tick);
  };

  // FLIP every item from where it is drawn now into its laid-out slot
  const settle = ({ tops, liftedId, scroller }: PendingSettle) => {
    const reduceMotion = prefersReducedMotion();
    const entries: { el: HTMLElement; oldTop: number }[] = [];
    tops.forEach((oldTop, id) => {
      const el = getItem(id);
      if (!el) return;
      el.style.transition = "none";
      el.style.translate = "";
      entries.push({ el, oldTop });
    });
    if (!reduceMotion) {
      const offsets = entries.map(
        ({ el, oldTop }) => oldTop - el.getBoundingClientRect().top,
      );
      entries.forEach(({ el }, i) => {
        if (Math.abs(offsets[i]) >= 0.5) el.style.translate = `0 ${offsets[i]}px`;
      });
    }
    void document.body.offsetHeight; // commit the start positions untransitioned
    for (const { el } of entries) {
      el.style.transition = reduceMotion ? "" : SETTLE_TRANSITION;
      el.style.translate = "";
    }

    const lifted = getItem(liftedId);
    if (lifted) lifted.dataset.dragState = "settling";

    window.clearTimeout(settleTimer);
    settleTimer = window.setTimeout(
      () => {
        for (const { el } of entries) el.style.transition = "";
        if (lifted) delete lifted.dataset.dragState;
        scroller?.removeAttribute("data-reorder-drag");
      },
      reduceMotion ? 0 : SETTLE_CLEANUP_MS,
    );
  };

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key !== "Escape") return;
    event.preventDefault();
    event.stopPropagation();
    finishDrag(false);
  };

  const beginDrag = (s: DragSession) => {
    const slots: Slot[] = [];
    for (const id of options.current.order) {
      const el = getItem(id);
      if (el) slots.push({ id, el, top: el.offsetTop, height: el.offsetHeight });
    }
    const fromIndex = slots.findIndex((slot) => slot.id === s.id);
    if (fromIndex === -1) return;

    window.clearTimeout(settleTimer);
    const reduceMotion = prefersReducedMotion();
    s.started = true;
    s.slots = slots;
    s.fromIndex = fromIndex;
    s.dropIndex = fromIndex;
    s.gap =
      slots.length > 1 ? slots[1].top - slots[0].top - slots[0].height : 0;
    s.scroller = options.current.scrollRef.current;
    s.startScrollTop = s.scroller?.scrollTop ?? 0;
    s.scroller?.setAttribute("data-reorder-drag", "");

    for (const slot of slots) {
      delete slot.el.dataset.dragState;
      slot.el.style.transition = reduceMotion
        ? "none"
        : slot.id === s.id
          ? LIFT_TRANSITION
          : SHIFT_TRANSITION;
    }
    slots[fromIndex].el.dataset.dragState = "lifted";

    window.addEventListener("keydown", onKeyDown, true);
    s.frame = requestAnimationFrame(tick);
  };

  const finishDrag = (commit: boolean) => {
    const s = session;
    if (!s) return;
    session = null;
    if (!s.started) return;
    cancelAnimationFrame(s.frame);
    window.removeEventListener("keydown", onKeyDown, true);

    const ids = s.slots.map((slot) => slot.id);
    const next = commit ? moveItem(ids, s.fromIndex, s.dropIndex) : ids;
    const pending: PendingSettle = {
      tops: new Map(
        s.slots.map((slot) => [slot.id, slot.el.getBoundingClientRect().top]),
      ),
      liftedId: s.id,
      scroller: s.scroller,
    };

    if (next.some((id, i) => id !== ids[i])) {
      // Settle once React has moved the DOM nodes (see flushSettle)
      pendingSettle = pending;
      options.current.onReorder(next);
    } else {
      settle(pending);
    }
  };

  return {
    onPointerDown(event: ReactPointerEvent<HTMLElement>, id: string) {
      if (event.button !== 0 || session) return;
      session = {
        id,
        pointerId: event.pointerId,
        startY: event.clientY,
        lastY: event.clientY,
        started: false,
        slots: [],
        fromIndex: -1,
        dropIndex: -1,
        gap: 0,
        scroller: null,
        startScrollTop: 0,
        frame: 0,
      };
      // Keep receiving moves and the release even outside the item
      event.currentTarget.setPointerCapture(event.pointerId);
    },

    onPointerMove(event: ReactPointerEvent<HTMLElement>) {
      if (!session || event.pointerId !== session.pointerId) return;
      session.lastY = event.clientY;
      if (
        !session.started &&
        Math.abs(session.lastY - session.startY) >= DRAG_THRESHOLD_PX
      ) {
        beginDrag(session);
      }
    },

    onPointerUp(event: ReactPointerEvent<HTMLElement>) {
      if (!session || event.pointerId !== session.pointerId) return;
      if (!session.started) {
        // A press without movement is a click
        const { id } = session;
        session = null;
        options.current.onActivate(id);
        return;
      }
      // The release can arrive before the next animation frame.
      session.lastY = event.clientY;
      positionDrag(session);
      finishDrag(true);
    },

    // Pointer cancelled or capture lost before release: put everything back
    onPointerCancel(event: ReactPointerEvent<HTMLElement>) {
      if (!session || event.pointerId !== session.pointerId) return;
      finishDrag(false);
    },

    isDragging: () => session?.started ?? false,

    flushSettle() {
      if (!pendingSettle) return;
      const pending = pendingSettle;
      pendingSettle = null;
      settle(pending);
    },

    dispose() {
      if (session) cancelAnimationFrame(session.frame);
      session = null;
      window.removeEventListener("keydown", onKeyDown, true);
      window.clearTimeout(settleTimer);
    },
  };
}

export function useDragReorder(options: DragReorderOptions) {
  const optionsRef = useRef(options);
  optionsRef.current = options;
  const [engine] = useState(() => createDragEngine(optionsRef));

  // Runs after every render, so a committed reorder settles before paint
  useLayoutEffect(() => engine.flushSettle());
  useEffect(() => () => engine.dispose(), [engine]);

  return engine;
}
