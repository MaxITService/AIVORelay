import React from "react";
import {
  normalizeRecordingOverlayCenterpieceMode,
  normalizeRecordingOverlayColor,
  recordingOverlayHexToRgba,
  type RecordingOverlayCenterpieceMode,
} from "./recordingOverlayAppearance";

interface RecordingOverlayCenterpieceProps {
  mode: RecordingOverlayCenterpieceMode;
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

export const RecordingOverlayCenterpiece: React.FC<
  RecordingOverlayCenterpieceProps
> = ({
  mode,
  accentColor,
  levels,
  animationSoftnessPercent = 55,
  depthParallaxPercent = 40,
}) => {
  const normalizedMode = normalizeRecordingOverlayCenterpieceMode(mode);
  if (normalizedMode === "none") {
    return null;
  }

  const accent = normalizeRecordingOverlayColor(accentColor);
  const energy = averageEnergy(levels);
  const softness = Math.max(0, Math.min(100, Math.round(animationSoftnessPercent))) / 100;
  const parallax = Math.max(0, Math.min(100, Math.round(depthParallaxPercent))) / 100;
  const transitionMs = Math.round(180 + (softness * 220));
  const driftX = (energy - 0.45) * 14 * parallax;
  const driftY = (0.4 - energy) * 8 * parallax;

  const shellStyle: React.CSSProperties = {
    position: "absolute",
    inset: 0,
    pointerEvents: "none",
    zIndex: 0,
    overflow: "hidden",
    borderRadius: "inherit",
  };

  if (normalizedMode === "halo_core") {
    const ringScale = 0.8 + (energy * 0.28);
    return (
      <div aria-hidden="true" style={shellStyle}>
        <div
          style={{
            position: "absolute",
            left: "50%",
            top: "50%",
            width: "42%",
            height: "68%",
            transform: `translate(calc(-50% + ${driftX}px), calc(-50% + ${driftY}px)) scale(${ringScale})`,
            borderRadius: "999px",
            border: `1px solid ${recordingOverlayHexToRgba(accent, 0.24)}`,
            boxShadow: `0 0 18px ${recordingOverlayHexToRgba(accent, 0.18)}, inset 0 0 18px ${recordingOverlayHexToRgba(accent, 0.08)}`,
            transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1), box-shadow ${transitionMs}ms ease-out`,
          }}
        />
        <div
          style={{
            position: "absolute",
            left: "50%",
            top: "50%",
            width: "22%",
            height: "36%",
            transform: `translate(calc(-50% + ${driftX * 1.4}px), calc(-50% + ${driftY * 1.2}px)) scale(${0.78 + (energy * 0.34)})`,
            borderRadius: "999px",
            background: `radial-gradient(circle, ${recordingOverlayHexToRgba(accent, 0.18 + (energy * 0.1))} 0%, ${recordingOverlayHexToRgba(accent, 0)} 72%)`,
            filter: "blur(12px)",
            transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1), opacity ${transitionMs}ms ease-out`,
          }}
        />
      </div>
    );
  }

  if (normalizedMode === "aurora_ribbon") {
    return (
      <div aria-hidden="true" style={shellStyle}>
        <div
          style={{
            position: "absolute",
            left: "-8%",
            right: "-8%",
            top: "50%",
            height: "44%",
            transform: `translate(${driftX}px, calc(-50% + ${driftY}px)) rotate(${-4 + (energy * 8)}deg)`,
            background: `linear-gradient(90deg, ${recordingOverlayHexToRgba(accent, 0)} 0%, ${recordingOverlayHexToRgba(accent, 0.08 + (energy * 0.08))} 18%, ${recordingOverlayHexToRgba("#ffffff", 0.14)} 48%, ${recordingOverlayHexToRgba(accent, 0.12 + (energy * 0.1))} 66%, ${recordingOverlayHexToRgba(accent, 0)} 100%)`,
            filter: "blur(16px)",
            opacity: 0.9,
            transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1), opacity ${transitionMs}ms ease-out`,
          }}
        />
      </div>
    );
  }

  if (normalizedMode === "orbital_beads") {
    return (
      <div aria-hidden="true" style={shellStyle}>
        {Array.from({ length: 6 }).map((_, index) => {
          const angle = ((Math.PI * 2) / 6) * index + (energy * 0.9);
          const x = Math.cos(angle) * (20 + (energy * 8));
          const y = Math.sin(angle) * (8 + (energy * 12));
          const size = 4 + ((index % 3) * 1.2);
          return (
            <div
              key={index}
              style={{
                position: "absolute",
                left: "50%",
                top: "50%",
                width: `${size}px`,
                height: `${size}px`,
                transform: `translate(calc(-50% + ${x + driftX}px), calc(-50% + ${y + driftY}px))`,
                borderRadius: "999px",
                background: `radial-gradient(circle at 35% 35%, rgba(255,255,255,0.94), ${recordingOverlayHexToRgba(accent, 0.84)} 64%, ${recordingOverlayHexToRgba(accent, 0.16)} 100%)`,
                boxShadow: `0 0 10px ${recordingOverlayHexToRgba(accent, 0.2)}`,
                opacity: 0.4 + (energy * 0.4),
                transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1), opacity ${transitionMs}ms ease-out`,
              }}
            />
          );
        })}
      </div>
    );
  }

  if (normalizedMode === "bloom_heart") {
    const scale = 0.74 + (energy * 0.24);
    return (
      <div aria-hidden="true" style={shellStyle}>
        <div
          style={{
            position: "absolute",
            left: "50%",
            top: "50%",
            width: "34%",
            height: "54%",
            transform: `translate(calc(-50% + ${driftX}px), calc(-50% + ${driftY}px)) scale(${scale})`,
            borderRadius: "999px",
            background: `radial-gradient(circle, ${recordingOverlayHexToRgba(accent, 0.18 + (energy * 0.08))} 0%, ${recordingOverlayHexToRgba(accent, 0)} 72%)`,
            filter: "blur(14px)",
            transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1), opacity ${transitionMs}ms ease-out`,
          }}
        />
        <svg
          width="100%"
          height="100%"
          viewBox="0 0 100 40"
          style={{
            position: "absolute",
            inset: 0,
            overflow: "visible",
            transform: `translate(${driftX}px, ${driftY}px) scale(${scale})`,
            transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1)`,
          }}
        >
          <defs>
            <linearGradient id="bloom-heart-fill" x1="0" y1="0" x2="1" y2="1">
              <stop offset="0%" stopColor="rgba(255,255,255,0.16)" />
              <stop offset="50%" stopColor={recordingOverlayHexToRgba(accent, 0.26)} />
              <stop offset="100%" stopColor={recordingOverlayHexToRgba(accent, 0.06)} />
            </linearGradient>
          </defs>
          <path
            d="M50 31 C46 26,34 19,34 11 C34 6,38 3,43 3 C46 3,49 5,50 8 C51 5,54 3,57 3 C62 3,66 6,66 11 C66 19,54 26,50 31 Z"
            fill="url(#bloom-heart-fill)"
            stroke={recordingOverlayHexToRgba(accent, 0.3)}
            strokeWidth="1.2"
          />
        </svg>
      </div>
    );
  }

  if (normalizedMode === "signal_crown") {
    return (
      <div aria-hidden="true" style={shellStyle}>
        <svg
          width="100%"
          height="100%"
          viewBox="0 0 100 40"
          style={{
            position: "absolute",
            inset: 0,
            overflow: "visible",
            transform: `translate(${driftX}px, ${driftY * 0.7}px)`,
            transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1)`,
          }}
        >
          <path
            d="M22 24 C30 14,38 10,50 9 C62 10,70 14,78 24"
            fill="none"
            stroke={recordingOverlayHexToRgba(accent, 0.24 + (energy * 0.08))}
            strokeWidth="1.4"
            strokeLinecap="round"
          />
          {[30, 40, 50, 60, 70].map((x, index) => (
            <g key={x}>
              <line
                x1={x}
                y1={22 - (index % 2 === 0 ? 6 : 2)}
                x2={x}
                y2={16 - (index % 2 === 0 ? 6 : 2)}
                stroke={recordingOverlayHexToRgba(accent, 0.3 + (energy * 0.12))}
                strokeWidth="1.3"
                strokeLinecap="round"
              />
              <circle
                cx={x}
                cy={14 - (index % 2 === 0 ? 6 : 2)}
                r={1.6 + ((index % 2) * 0.4)}
                fill={recordingOverlayHexToRgba(accent, 0.78)}
              />
            </g>
          ))}
        </svg>
      </div>
    );
  }

  return null;
};
