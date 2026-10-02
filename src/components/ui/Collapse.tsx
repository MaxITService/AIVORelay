import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import { getCollapseDurationMs } from "@/lib/motion";

interface CollapseProps {
  open: boolean;
  children: React.ReactNode;
  id?: string;
  /** Classes for the outer animated wrapper (layout only; padding belongs on children). */
  className?: string;
  /** Keep children mounted while collapsed instead of unmounting after the exit animation. */
  keepMounted?: boolean;
}

/**
 * Animated height disclosure shared by every expandable section.
 * Content is clipped only while animating, so popups inside an open section are not cut off.
 */
export const Collapse: React.FC<CollapseProps> = ({
  open,
  children,
  id,
  className = "",
  keepMounted = false,
}) => {
  const rootRef = useRef<HTMLDivElement>(null);
  const clipRef = useRef<HTMLDivElement>(null);
  const durationRef = useRef(0);
  const [isRendered, setIsRendered] = useState(open);
  const [isExpanded, setIsExpanded] = useState(open);
  const [isSettled, setIsSettled] = useState(open);
  const [wasOpen, setWasOpen] = useState(open);

  if (open !== wasOpen) {
    setWasOpen(open);
    setIsSettled(false);
    if (open) {
      setIsRendered(true);
    }
  }

  // Content is mounted at zero height first; measure it, then start the
  // transition before paint with a duration scaled to the content height.
  useLayoutEffect(() => {
    const root = rootRef.current;
    const clip = clipRef.current;
    if (!root || !clip || open === isExpanded) return;

    const duration = getCollapseDurationMs(clip.scrollHeight);
    durationRef.current = duration;
    root.style.setProperty("--app-collapse-duration", `${duration}ms`);
    void root.offsetHeight;
    setIsExpanded(open);
  }, [open, isExpanded, isRendered]);

  useEffect(() => {
    if (isSettled || open !== isExpanded) return;
    const timer = window.setTimeout(() => {
      if (open) {
        setIsSettled(true);
      } else {
        setIsRendered(false);
      }
    }, durationRef.current);
    return () => window.clearTimeout(timer);
  }, [open, isExpanded, isSettled]);

  useLayoutEffect(() => {
    if (rootRef.current) {
      rootRef.current.inert = !open;
    }
  }, [open, isRendered]);

  if (!isRendered && !keepMounted) {
    return null;
  }

  return (
    <div
      ref={rootRef}
      id={id}
      className={`app-collapse ${className}`}
      data-state={isExpanded ? "open" : "closed"}
      data-settled={isSettled && open ? "" : undefined}
      aria-hidden={open ? undefined : true}
    >
      <div ref={clipRef} className="app-collapse__clip">
        {children}
      </div>
    </div>
  );
};
