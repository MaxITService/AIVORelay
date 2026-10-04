import React, { useEffect, useRef } from "react";
import { Copy } from "lucide-react";
import "./CopyFeedbackIcon.css";

interface CopyFeedbackIconProps {
  copied: boolean;
  /** Increment on every successful copy to replay the burst, even while already copied. */
  burstKey: number;
  size?: number;
}

const SPARK_ANGLES = [30, 90, 150, 210, 270, 330];

/**
 * Copy icon that morphs into a hand-drawn check with a bounce, ring and sparks.
 * Place it as the direct child of a button to get the press-in squeeze.
 */
export const CopyFeedbackIcon: React.FC<CopyFeedbackIconProps> = ({
  copied,
  burstKey,
  size = 16,
}) => {
  const bodyRef = useRef<HTMLSpanElement>(null);
  const checkPathRef = useRef<SVGPathElement>(null);

  useEffect(() => {
    if (burstKey === 0) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;

    const animations: Animation[] = [];
    const body = bodyRef.current;
    if (body && typeof body.animate === "function") {
      animations.push(
        body.animate(
          [
            { transform: "scale(1)" },
            { transform: "scale(0.78)", offset: 0.16 },
            { transform: "scale(1.22)", offset: 0.48 },
            { transform: "scale(0.94)", offset: 0.74 },
            { transform: "scale(1)" },
          ],
          { duration: 540, easing: "ease-out" },
        ),
      );
    }

    // Draw the check stroke from the short leg to the long one
    const path = checkPathRef.current;
    if (path && typeof path.animate === "function") {
      animations.push(
        path.animate([{ strokeDashoffset: "1" }, { strokeDashoffset: "0" }], {
          duration: 360,
          delay: 90,
          easing: "cubic-bezier(0.65, 0, 0.35, 1)",
          fill: "backwards",
        }),
      );
    }

    return () => animations.forEach((animation) => animation.cancel());
  }, [burstKey]);

  return (
    <span
      className="aivo-copy-icon"
      data-copied={copied}
      style={{ width: size, height: size }}
    >
      {burstKey > 0 && (
        <span key={burstKey} className="aivo-copy-icon__burst" aria-hidden="true">
          <span className="aivo-copy-icon__ring" />
          {SPARK_ANGLES.map((angle, index) => (
            <span
              key={angle}
              className="aivo-copy-icon__spark"
              data-alt={index % 2 === 1}
              style={{ "--aivo-copy-spark-angle": `${angle}deg` } as React.CSSProperties}
            />
          ))}
        </span>
      )}
      <span ref={bodyRef} className="aivo-copy-icon__body">
        <Copy className="aivo-copy-icon__copy" width={size} height={size} />
        <svg
          className="aivo-copy-icon__check"
          width={size}
          height={size}
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth={2.5}
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path ref={checkPathRef} d="M4.5 12.5l5 5L20 7" pathLength={1} />
        </svg>
      </span>
    </span>
  );
};
