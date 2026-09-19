import React, { useCallback, useEffect, useRef } from "react";
import {
  normalizeRecordingOverlayAnimatedBorderMode,
  normalizeRecordingOverlayColor,
  recordingOverlayHexToRgba,
  type RecordingOverlayAnimatedBorderMode,
} from "./recordingOverlayAppearance";
import "./RecordingOverlayAnimatedBorder.css";

interface RecordingOverlayAnimatedBorderProps {
  mode: RecordingOverlayAnimatedBorderMode;
  accentColor: string;
  levels: number[];
  animationSoftnessPercent?: number;
  /** Corner radius of the overlay frame, so the traveling light follows its outline. */
  frameRadiusPx?: number;
}

type ActiveAnimatedBorderMode = Exclude<RecordingOverlayAnimatedBorderMode, "none">;

interface AnimatedBorderMotion {
  keyframes: Keyframe[];
  easing: string;
  /** Loop duration at playback rate 1: base for softness 0 plus the span added at softness 1. */
  cycleMs: [base: number, softnessSpan: number];
  /** Playback rate in silence plus the extra rate at full energy. */
  rate: [base: number, energySpan: number];
}

/** Width of one shimmer tile; must match `background-size` in the stylesheet. */
const SHIMMER_TILE_PX = 96;

/** One lap of the traveling light; the rects declare this as their `pathLength`. */
const LAP_UNITS = 100;

const ANIMATED_BORDER_MOTION: Record<ActiveAnimatedBorderMode, AnimatedBorderMotion> = {
  shimmer_edge: {
    keyframes: [
      { backgroundPosition: "0px 0px" },
      { backgroundPosition: `${SHIMMER_TILE_PX}px 0px` },
    ],
    easing: "linear",
    cycleMs: [2200, 1600],
    rate: [0.7, 1.3],
  },
  traveling_highlight: {
    keyframes: [{ strokeDashoffset: "0" }, { strokeDashoffset: `${LAP_UNITS}` }],
    easing: "linear",
    cycleMs: [2600, 1400],
    rate: [0.75, 1.75],
  },
  breathing_contour: {
    keyframes: [{ opacity: 0.5 }, { opacity: 1 }, { opacity: 0.5 }],
    easing: "ease-in-out",
    cycleMs: [2400, 1600],
    rate: [0.85, 0.9],
  },
};

function clampUnit(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }
  return Math.max(0, Math.min(1, value));
}

/**
 * Loudness that drives the border. The bar average stays low even during
 * speech, so the loudest bar lifts the response.
 */
function borderEnergy(levels: number[]): number {
  if (levels.length === 0) {
    return 0;
  }
  let total = 0;
  let peak = 0;
  for (const level of levels) {
    const value = clampUnit(level);
    total += value;
    peak = Math.max(peak, value);
  }
  return clampUnit((total / levels.length) * 1.4 + peak * 0.35);
}

/** Reads the frame's CSS `borderRadius` for the `frameRadiusPx` prop. */
export function resolveRecordingOverlayFrameRadiusPx(
  borderRadius: unknown,
): number {
  const parsed =
    typeof borderRadius === "number"
      ? borderRadius
      : Number.parseFloat(String(borderRadius ?? ""));
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : 18;
}

export const RecordingOverlayAnimatedBorder: React.FC<
  RecordingOverlayAnimatedBorderProps
> = ({
  mode,
  accentColor,
  levels,
  animationSoftnessPercent = 55,
  frameRadiusPx = 18,
}) => {
  const normalizedMode = normalizeRecordingOverlayAnimatedBorderMode(mode);
  const motion =
    normalizedMode === "none" ? null : ANIMATED_BORDER_MOTION[normalizedMode];
  const softness = Math.max(0, Math.min(100, Math.round(animationSoftnessPercent))) / 100;
  const cycleMs = motion
    ? Math.round(motion.cycleMs[0] + (softness * motion.cycleMs[1]))
    : 0;
  const energy = borderEnergy(levels);
  const playbackRate = motion ? motion.rate[0] + (energy * motion.rate[1]) : 1;

  // The looping motion runs through the Web Animations API instead of CSS
  // keyframes so the energy can change its speed without restarting it.
  const animatedNodeRef = useRef<Element | null>(null);
  const animationRef = useRef<Animation | null>(null);
  const playbackRateRef = useRef(playbackRate);
  playbackRateRef.current = playbackRate;
  const setAnimatedNode = useCallback((node: Element | null) => {
    animatedNodeRef.current = node;
  }, []);

  useEffect(() => {
    const node = animatedNodeRef.current;
    if (!motion || !node || typeof node.animate !== "function") {
      return;
    }
    if (window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) {
      return;
    }
    const animation = node.animate(motion.keyframes, {
      duration: cycleMs,
      easing: motion.easing,
      iterations: Infinity,
    });
    animation.playbackRate = playbackRateRef.current;
    animationRef.current = animation;
    return () => {
      animation.cancel();
      animationRef.current = null;
    };
  }, [motion, cycleMs]);

  useEffect(() => {
    const animation = animationRef.current;
    if (!animation) {
      return;
    }
    if (typeof animation.updatePlaybackRate === "function") {
      animation.updatePlaybackRate(playbackRate);
    } else {
      animation.playbackRate = playbackRate;
    }
  }, [playbackRate]);

  if (!motion) {
    return null;
  }

  const accent = normalizeRecordingOverlayColor(accentColor);
  const transitionMs = Math.round(160 + (softness * 220));
  const layerStyle: React.CSSProperties = {
    transition: `opacity ${transitionMs}ms ease-out`,
  };
  const glowStyle: React.CSSProperties = {
    transition: `box-shadow ${transitionMs}ms ease-out`,
  };

  if (normalizedMode === "shimmer_edge") {
    Object.assign(layerStyle, {
      opacity: 0.75 + (energy * 0.25),
      "--rob-thickness": "1.5px",
      "--rob-line": recordingOverlayHexToRgba(accent, 0.28 + (energy * 0.14)),
      "--rob-spark": recordingOverlayHexToRgba(accent, 0.7 + (energy * 0.3)),
      "--rob-spark-core": `rgba(255, 255, 255, ${0.7 + (energy * 0.25)})`,
      "--rob-glow": recordingOverlayHexToRgba(accent, 0.08 + (energy * 0.14)),
      "--rob-glow-size": `${Math.round(10 + (energy * 8))}px`,
    });
    return (
      <div
        aria-hidden="true"
        className="recording-overlay-border"
        data-mode={normalizedMode}
        style={layerStyle}
      >
        <div className="recording-overlay-border-glow" style={glowStyle} />
        <div className="recording-overlay-border-ring" ref={setAnimatedNode} />
      </div>
    );
  }

  if (normalizedMode === "traveling_highlight") {
    Object.assign(layerStyle, {
      opacity: 1,
      "--rob-thickness": "1px",
      "--rob-line": recordingOverlayHexToRgba(accent, 0.2 + (energy * 0.1)),
      "--rob-glow": recordingOverlayHexToRgba(accent, 0.05 + (energy * 0.1)),
      "--rob-glow-size": `${Math.round(8 + (energy * 6))}px`,
    });
    const cornerRadius = Math.max(0, frameRadiusPx - 1);
    const strokeStyle: React.CSSProperties = {
      transition: `opacity ${transitionMs}ms ease-out`,
    };
    // Every dash starts at the same point, which is the head of the comet, and
    // the shorter, brighter dashes stack on top of the longer, dimmer tail.
    const laps = (
      dash: number,
      strokeWidth: number,
      stroke: string,
      style?: React.CSSProperties,
    ) => (
      <rect
        x={0}
        y={0}
        width="100%"
        height="100%"
        rx={cornerRadius}
        ry={cornerRadius}
        pathLength={LAP_UNITS}
        fill="none"
        stroke={stroke}
        strokeWidth={strokeWidth}
        strokeLinecap="round"
        strokeDasharray={`${dash} ${LAP_UNITS - dash}`}
        style={{ ...strokeStyle, ...style }}
      />
    );
    return (
      <div
        aria-hidden="true"
        className="recording-overlay-border"
        data-mode={normalizedMode}
        style={layerStyle}
      >
        <div className="recording-overlay-border-glow" style={glowStyle} />
        <div className="recording-overlay-border-ring" />
        <svg className="recording-overlay-border-svg">
          <g ref={setAnimatedNode}>
            {laps(16, 6, recordingOverlayHexToRgba(accent, 0.5 + (energy * 0.5)), {
              filter: "blur(5px)",
              opacity: 0.55 + (energy * 0.45),
            })}
            {laps(12, 1.6, recordingOverlayHexToRgba(accent, 0.75), {
              opacity: 0.6 + (energy * 0.4),
            })}
            {laps(7, 1.6, recordingOverlayHexToRgba(accent, 1))}
            {laps(3, 1.6, "rgba(255, 255, 255, 0.92)")}
          </g>
        </svg>
      </div>
    );
  }

  Object.assign(layerStyle, {
    opacity: 0.6 + (energy * 0.4),
    "--rob-thickness": "1.5px",
    "--rob-line": recordingOverlayHexToRgba(accent, 0.5 + (energy * 0.2)),
    "--rob-glow": recordingOverlayHexToRgba(accent, 0.16 + (energy * 0.2)),
    "--rob-glow-size": `${Math.round(14 + (energy * 10))}px`,
  });
  return (
    <div
      aria-hidden="true"
      className="recording-overlay-border"
      data-mode={normalizedMode}
      style={layerStyle}
    >
      <div className="recording-overlay-border-pulse" ref={setAnimatedNode}>
        <div className="recording-overlay-border-glow" style={glowStyle} />
        <div className="recording-overlay-border-ring" />
      </div>
    </div>
  );
};
