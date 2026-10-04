import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import { prefersReducedMotion } from "@/lib/motion";
import "./FileDropZone.css";

/** Keep in sync with `aivo-file-drop-swallow` in FileDropZone.css. */
const SWALLOW_DURATION_MS = 300;
/** Keep in sync with `aivo-file-card-leave` in FileDropZone.css. */
const RELEASE_DURATION_MS = 180;
/** Delay + duration of `aivo-file-drop-icon-pop` in FileDropZone.css. */
const ZONE_ENTRANCE_DURATION_MS = 610;
const HEIGHT_SETTLE_DURATION_MS = 420;
/** Dash + gap of the outline's `stroke-dasharray`. */
const DASH_PERIOD = 14;
const OUTLINE_INSET = 1;
const OUTLINE_RADIUS = 11;

type SlotPhase = "zone" | "swallowing" | "file" | "releasing";

interface FileDropZoneProps {
  hasFile: boolean;
  /** A file is being dragged over the window. */
  isDragOver: boolean;
  /** A dropped or picked file is still being prepared. */
  isReceiving: boolean;
  onSelect: () => void;
  title: string;
  subtitle: string;
  formats: string;
  /** Selected-file content; the first child is the file card. */
  children: React.ReactNode;
}

/**
 * Drop zone that swallows the incoming file and springs the file card out of
 * its center; clearing the file shrinks the card away and unfolds the zone
 * again. A file that is already selected on mount appears in place.
 */
export const FileDropZone: React.FC<FileDropZoneProps> = ({
  hasFile,
  isDragOver,
  isReceiving,
  onSelect,
  title,
  subtitle,
  formats,
  children,
}) => {
  const [phase, setPhase] = useState<SlotPhase>(hasFile ? "file" : "zone");
  // The current view arrived through a transition and plays its entrance.
  const [entering, setEntering] = useState(false);
  const slotRef = useRef<HTMLDivElement>(null);
  const zoneRef = useRef<HTMLDivElement>(null);
  const outlineRef = useRef<SVGRectElement>(null);
  const fileRef = useRef<HTMLDivElement>(null);
  const switchFromHeightRef = useRef(0);
  const lastFileContentRef = useRef<React.ReactNode>(null);

  // The parent drops its file content as soon as the file is cleared; keep the
  // last one so the card can play its exit.
  if (hasFile) lastFileContentRef.current = children;
  const fileContent = hasFile ? children : lastFileContentRef.current;
  const showFile = phase === "file" || phase === "releasing";

  // Layout effect: phase changes must land before paint so the outgoing view
  // never flashes empty or remounts.
  useLayoutEffect(() => {
    const reduceMotion = prefersReducedMotion();
    const beginSwitch = (next: SlotPhase) => {
      switchFromHeightRef.current = slotRef.current?.offsetHeight ?? 0;
      setPhase(next);
    };

    if (hasFile && phase === "zone") {
      if (reduceMotion) {
        setEntering(false);
        setPhase("file");
      } else {
        beginSwitch("swallowing");
      }
      return;
    }
    if (!hasFile && phase === "file") {
      if (reduceMotion) {
        setEntering(false);
        setPhase("zone");
      } else {
        beginSwitch("releasing");
      }
      return;
    }
    if (phase === "swallowing") {
      // Cleared while swallowing: the zone is still mounted, bring it back.
      if (!hasFile) {
        setEntering(true);
        setPhase("zone");
        return;
      }
      const timer = window.setTimeout(() => {
        setEntering(true);
        setPhase("file");
      }, SWALLOW_DURATION_MS);
      return () => window.clearTimeout(timer);
    }
    if (phase === "releasing") {
      // A new file arrived while the old card was leaving.
      if (hasFile) {
        setEntering(true);
        setPhase("file");
        return;
      }
      const timer = window.setTimeout(() => {
        setEntering(true);
        setPhase("zone");
      }, RELEASE_DURATION_MS);
      return () => window.clearTimeout(timer);
    }
  }, [hasFile, phase]);

  // Leave the entrance state once the zone has unfolded, so dragging a file
  // over and away again does not replay the icon pop.
  useEffect(() => {
    if (phase !== "zone" || !entering) return;
    const timer = window.setTimeout(
      () => setEntering(false),
      ZONE_ENTRANCE_DURATION_MS,
    );
    return () => window.clearTimeout(timer);
  }, [phase, entering]);

  // Fit a whole number of dashes around the outline so the marching pattern
  // has no seam where the path starts.
  useLayoutEffect(() => {
    const zone = zoneRef.current;
    const outline = outlineRef.current;
    if (showFile || !zone || !outline) return;

    const fitDashes = () => {
      const width = zone.clientWidth - OUTLINE_INSET * 2;
      const height = zone.clientHeight - OUTLINE_INSET * 2;
      const radius = Math.min(OUTLINE_RADIUS, width / 2, height / 2);
      const perimeter = 2 * (width + height) - (8 - 2 * Math.PI) * radius;
      const dashCount = Math.max(1, Math.round(perimeter / DASH_PERIOD));
      outline.setAttribute("pathLength", String(dashCount * DASH_PERIOD));
    };

    fitDashes();
    const observer = new ResizeObserver(fitDashes);
    observer.observe(zone);
    return () => observer.disconnect();
  }, [showFile]);

  // Glide the slot from the outgoing view's height to the incoming one. The
  // card springs out of the point where the zone collapsed; the zone unfolds
  // downward as the slot grows.
  useLayoutEffect(() => {
    const slot = slotRef.current;
    const fromHeight = switchFromHeightRef.current;
    switchFromHeightRef.current = 0;
    if (!entering || !slot || fromHeight <= 0) return;

    const file = fileRef.current;
    const card = file?.firstElementChild as HTMLElement | null | undefined;
    if (showFile && file && card) {
      const cardCenter = card.offsetTop + card.offsetHeight / 2;
      file.style.setProperty(
        "--aivo-file-card-from-y",
        `${fromHeight / 2 - cardCenter}px`,
      );
    }

    const toHeight = slot.offsetHeight;
    if (fromHeight === toHeight) return;

    slot.dataset.settling = "true";
    const animation = slot.animate(
      [{ height: `${fromHeight}px` }, { height: `${toHeight}px` }],
      {
        duration: HEIGHT_SETTLE_DURATION_MS,
        easing: "cubic-bezier(0.22, 1, 0.36, 1)",
      },
    );
    const settle = () => {
      delete slot.dataset.settling;
    };
    animation.onfinish = settle;
    animation.oncancel = settle;
    return () => animation.cancel();
  }, [showFile, entering]);

  return (
    <div ref={slotRef} className="aivo-file-slot">
      {showFile ? (
        <div
          ref={fileRef}
          className="aivo-file-slot__file space-y-4"
          data-state={
            phase === "releasing" ? "leaving" : entering ? "entering" : "idle"
          }
        >
          {fileContent}
        </div>
      ) : (
        <div
          ref={zoneRef}
          role="button"
          tabIndex={phase === "swallowing" ? -1 : 0}
          aria-label={title}
          aria-busy={isReceiving || phase === "swallowing"}
          onClick={onSelect}
          onKeyDown={(event) => {
            if (event.key === "Enter" || event.key === " ") {
              event.preventDefault();
              onSelect();
            }
          }}
          className="aivo-file-drop rounded-xl p-8 text-center cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#ff4d8d]/60"
          data-drag-over={isDragOver}
          data-receiving={isReceiving}
          data-phase={
            phase === "swallowing"
              ? "swallowing"
              : entering
                ? "entering"
                : "idle"
          }
        >
          <svg className="aivo-file-drop__outline" aria-hidden="true">
            <rect
              ref={outlineRef}
              width="100%"
              height="100%"
              rx={OUTLINE_RADIUS}
              ry={OUTLINE_RADIUS}
            />
          </svg>
          <div className="aivo-file-drop__content flex flex-col items-center gap-3">
            <div className="aivo-file-drop__icon">
              {/* lucide Upload, split so the arrow can stretch on its own */}
              <svg
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth={2}
                strokeLinecap="round"
                strokeLinejoin="round"
                aria-hidden="true"
              >
                <path className="aivo-file-drop__arrow-shaft" d="M12 3v12" />
                <path className="aivo-file-drop__arrow-head" d="m17 8-5-5-5 5" />
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              </svg>
            </div>
            <div>
              <p className="text-sm font-medium text-[#f5f5f5]">{title}</p>
              <p className="text-xs text-[#808080] mt-1">{subtitle}</p>
            </div>
            <p className="text-xs text-[#606060]">{formats}</p>
          </div>
        </div>
      )}
    </div>
  );
};
