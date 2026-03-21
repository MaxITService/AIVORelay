import React from "react";
import {
  normalizeRecordingOverlayAnimatedBorderMode,
  normalizeRecordingOverlayColor,
  recordingOverlayHexToRgba,
  type RecordingOverlayAnimatedBorderMode,
} from "./recordingOverlayAppearance";

interface RecordingOverlayAnimatedBorderProps {
  mode: RecordingOverlayAnimatedBorderMode;
  accentColor: string;
  levels: number[];
  animationSoftnessPercent?: number;
  depthParallaxPercent?: number;
}

function clampUnit(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }
  return Math.max(0, Math.min(1, value));
}

function averageEnergy(levels: number[]): number {
  if (levels.length === 0) {
    return 0;
  }
  return clampUnit(
    levels.reduce((total, level) => total + clampUnit(level), 0) / levels.length,
  );
}

export const RecordingOverlayAnimatedBorder: React.FC<
  RecordingOverlayAnimatedBorderProps
> = ({
  mode,
  accentColor,
  levels,
  animationSoftnessPercent = 55,
  depthParallaxPercent = 40,
}) => {
  const normalizedMode = normalizeRecordingOverlayAnimatedBorderMode(mode);
  if (normalizedMode === "none") {
    return null;
  }

  const accent = normalizeRecordingOverlayColor(accentColor);
  const energy = averageEnergy(levels);
  const softness = Math.max(0, Math.min(100, Math.round(animationSoftnessPercent))) / 100;
  const parallax = Math.max(0, Math.min(100, Math.round(depthParallaxPercent))) / 100;
  const transitionMs = Math.round(160 + (softness * 220));
  const driftX = (energy - 0.4) * 8 * parallax;
  const driftY = (0.5 - energy) * 5 * parallax;

  const commonStyle: React.CSSProperties = {
    position: "absolute",
    inset: 0,
    borderRadius: "inherit",
    pointerEvents: "none",
    zIndex: 1,
  };

  if (normalizedMode === "shimmer_edge") {
    return (
      <div
        aria-hidden="true"
        style={{
          ...commonStyle,
          inset: "1px",
          border: `1px solid ${recordingOverlayHexToRgba(accent, 0.16 + (energy * 0.1))}`,
          boxShadow: `inset 0 0 0 1px ${recordingOverlayHexToRgba(accent, 0.06)}, 0 0 16px ${recordingOverlayHexToRgba(accent, 0.12)}`,
          transform: `translate(${driftX}px, ${driftY}px)`,
          transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1), box-shadow ${transitionMs}ms ease-out`,
        }}
      />
    );
  }

  if (normalizedMode === "traveling_highlight") {
    return (
      <div aria-hidden="true" style={{ ...commonStyle, overflow: "hidden" }}>
        <div
          style={{
            position: "absolute",
            inset: "1px",
            borderRadius: "inherit",
            border: `1px solid ${recordingOverlayHexToRgba(accent, 0.12)}`,
          }}
        />
        <div
          style={{
            position: "absolute",
            left: "-20%",
            top: 0,
            bottom: 0,
            width: "32%",
            background: `linear-gradient(90deg, ${recordingOverlayHexToRgba(accent, 0)} 0%, ${recordingOverlayHexToRgba("#ffffff", 0.18)} 48%, ${recordingOverlayHexToRgba(accent, 0)} 100%)`,
            transform: `translateX(${(energy * 68) + (parallax * 10)}%) skewX(-16deg)`,
            filter: "blur(4px)",
            opacity: 0.76,
            transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1), opacity ${transitionMs}ms ease-out`,
          }}
        />
      </div>
    );
  }

  if (normalizedMode === "breathing_contour") {
    return (
      <div
        aria-hidden="true"
        style={{
          ...commonStyle,
          inset: "1px",
          border: `1px solid ${recordingOverlayHexToRgba(accent, 0.16 + (energy * 0.14))}`,
          boxShadow: `0 0 ${12 + (energy * 10)}px ${recordingOverlayHexToRgba(accent, 0.16 + (energy * 0.1))}, inset 0 0 ${10 + (energy * 8)}px ${recordingOverlayHexToRgba(accent, 0.06 + (energy * 0.06))}`,
          transform: `translate(${driftX}px, ${driftY}px) scale(${0.998 + (energy * 0.012)})`,
          transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1), box-shadow ${transitionMs}ms ease-out`,
        }}
      />
    );
  }

  return null;
};
