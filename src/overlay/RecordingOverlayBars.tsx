import React from "react";
import {
  normalizeRecordingOverlayBarStyle,
  normalizeRecordingOverlayColor,
  recordingOverlayHexToRgba,
  type RecordingOverlayBarStyle,
} from "./recordingOverlayAppearance";

interface RecordingOverlayBarsProps {
  levels: number[];
  barCount: number;
  barWidthPx: number;
  accentColor: string;
  barStyle: RecordingOverlayBarStyle;
  animationSoftnessPercent?: number;
  animated?: boolean;
  maxHeightPx?: number;
}

const BAR_GAP_PX = 3;

function clampUnit(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }
  return Math.max(0, Math.min(1, value));
}

function barHeightFromLevel(level: number, maxHeightPx: number): number {
  return Math.min(maxHeightPx, 4 + Math.pow(clampUnit(level), 0.7) * (maxHeightPx - 4));
}

function easeInQuad(value: number): number {
  const clamped = clampUnit(value);
  return clamped * clamped;
}

function easeOutCubic(value: number): number {
  const clamped = clampUnit(value);
  return 1 - Math.pow(1 - clamped, 3);
}

function pulseOffset(level: number, index: number): number {
  return Math.sin((level * 4.5) + index * 0.7) * 1.2;
}

function laneWidthForStyle(
  style: RecordingOverlayBarStyle,
  effectiveWidth: number,
): number {
  switch (style) {
    case "vinyl":
      return Math.max(effectiveWidth + 6, 10);
    case "bloom_bounce":
    case "daisy":
    case "garden_sway":
    case "lotus":
      return Math.max(effectiveWidth + 10, 16);
    case "orbit":
    case "tuner":
    case "morse":
      return Math.max(effectiveWidth + 2, 8);
    case "constellation":
    case "fireflies":
    case "helix":
    case "petals":
    case "petal_rain":
    case "pulse_rings":
      return Math.max(effectiveWidth + 8, 14);
    default:
      return effectiveWidth;
  }
}

function isCenterAlignedStyle(style: RecordingOverlayBarStyle): boolean {
  switch (style) {
    case "constellation":
    case "fireflies":
    case "bloom_bounce":
    case "daisy":
    case "garden_sway":
    case "helix":
    case "lotus":
    case "orbit":
    case "petals":
    case "petal_rain":
    case "pulse_rings":
    case "radar":
    case "vinyl":
      return true;
    default:
      return false;
  }
}

export const RecordingOverlayBars: React.FC<RecordingOverlayBarsProps> = ({
  levels,
  barCount,
  barWidthPx,
  accentColor,
  barStyle,
  animationSoftnessPercent = 55,
  animated = true,
  maxHeightPx = 20,
}) => {
  const normalizedStyle = normalizeRecordingOverlayBarStyle(barStyle);
  const accent = normalizeRecordingOverlayColor(accentColor);
  const effectiveCount = Math.max(3, Math.min(16, Math.round(barCount)));
  const effectiveWidth = Math.max(2, Math.min(12, Math.round(barWidthPx)));
  const softness = Math.max(0, Math.min(100, Math.round(animationSoftnessPercent))) / 100;
  const heightDurationMs = Math.round(90 + (softness * 120));
  const opacityDurationMs = Math.round(120 + (softness * 120));
  const motionDurationMs = Math.round(120 + (softness * 180));
  const transition = animated
    ? `height ${heightDurationMs}ms ease-out, opacity ${opacityDurationMs}ms ease-out, transform ${motionDurationMs}ms cubic-bezier(0.22, 1, 0.36, 1), top ${motionDurationMs}ms cubic-bezier(0.22, 1, 0.36, 1), left ${motionDurationMs}ms cubic-bezier(0.22, 1, 0.36, 1)`
    : "none";
  const laneWidth = laneWidthForStyle(normalizedStyle, effectiveWidth);
  const alignItems = isCenterAlignedStyle(normalizedStyle) ? "center" : "flex-end";

  return (
    <div
      style={{
        display: "flex",
        alignItems,
        justifyContent: "center",
        gap: `${BAR_GAP_PX}px`,
        height: `${maxHeightPx + 4}px`,
      }}
    >
      {levels.slice(0, effectiveCount).map((rawLevel, index) => {
        const level = clampUnit(rawLevel);
        const easedLevel = easeOutCubic(level);
        const height = barHeightFromLevel(level, maxHeightPx);
        const opacity = Math.max(0.24, Math.min(1, level * 1.75));

        if (normalizedStyle === "bloom_bounce") {
          const blossomSize = Math.max(8, laneWidth - 4);
          const centerX = laneWidth / 2;
          const baseY = maxHeightPx * 0.58;
          const bounce = (1 - easedLevel) * 2.2 - (Math.sin((index * 0.6) + (easedLevel * 2.6)) * 1.2);
          const bloomScale = 0.72 + (easedLevel * 0.55);
          const stemTop = Math.max(4, baseY + (blossomSize * 0.16));
          return (
            <div
              key={index}
              style={{
                width: `${laneWidth}px`,
                height: `${maxHeightPx}px`,
                position: "relative",
              }}
            >
              <div
                style={{
                  position: "absolute",
                  left: `${centerX - 1}px`,
                  top: `${stemTop}px`,
                  width: "2px",
                  height: `${Math.max(6, maxHeightPx - stemTop)}px`,
                  borderRadius: "999px",
                  background: `linear-gradient(180deg, ${recordingOverlayHexToRgba(accent, 0.18)}, ${recordingOverlayHexToRgba(accent, 0.72)})`,
                  opacity: 0.9,
                  transition,
                }}
              />
              {[0, 72, 144, 216, 288].map((angle, petalIndex) => (
                <div
                  key={petalIndex}
                  style={{
                    position: "absolute",
                    left: "50%",
                    top: `${baseY + bounce}px`,
                    width: `${Math.max(5, effectiveWidth * 0.92)}px`,
                    height: `${blossomSize}px`,
                    borderRadius: "999px",
                    background: `linear-gradient(180deg, rgba(255,255,255,0.97), ${recordingOverlayHexToRgba(accent, 0.82)})`,
                    boxShadow: `0 0 10px ${recordingOverlayHexToRgba(accent, 0.18)}`,
                    opacity: Math.max(0.38, opacity - (petalIndex * 0.06)),
                    transform: `translate(-50%, -50%) rotate(${angle + (index * 5)}deg) scale(${bloomScale - (petalIndex * 0.03)})`,
                    transformOrigin: "center center",
                    transition,
                  }}
                />
              ))}
              <div
                style={{
                  position: "absolute",
                  left: "50%",
                  top: `${baseY + bounce}px`,
                  width: `${Math.max(4, effectiveWidth * 0.78)}px`,
                  height: `${Math.max(4, effectiveWidth * 0.78)}px`,
                  borderRadius: "999px",
                  background: `radial-gradient(circle at 35% 35%, rgba(255,252,232,0.98), ${recordingOverlayHexToRgba(accent, 0.96)} 68%, ${recordingOverlayHexToRgba(accent, 0.34)} 100%)`,
                  boxShadow: `0 0 8px ${recordingOverlayHexToRgba(accent, 0.22)}`,
                  transform: `translate(-50%, -50%) scale(${0.8 + (easedLevel * 0.3)})`,
                  transition,
                }}
              />
            </div>
          );
        }

        if (normalizedStyle === "retro") {
          const segments = 5;
          const activeSegments = Math.max(1, Math.round(level * segments));
          const segmentHeight = (maxHeightPx - ((segments - 1) * 2)) / segments;
          return (
            <div
              key={index}
              style={{
                width: `${effectiveWidth}px`,
                height: `${maxHeightPx}px`,
                display: "flex",
                flexDirection: "column-reverse",
                gap: "2px",
              }}
            >
              {Array.from({ length: segments }).map((_, segmentIndex) => {
                const lit = segmentIndex < activeSegments;
                return (
                  <div
                    key={segmentIndex}
                    style={{
                      height: `${segmentHeight}px`,
                      borderRadius: "1px",
                      background: lit
                        ? `linear-gradient(180deg, rgba(255,255,255,0.92), ${recordingOverlayHexToRgba(accent, 0.68)})`
                        : recordingOverlayHexToRgba(accent, 0.1),
                      boxShadow: lit
                        ? `0 0 8px ${recordingOverlayHexToRgba(accent, 0.28)}`
                        : "none",
                      opacity: lit ? 1 : 0.55,
                      transition,
                    }}
                  />
                );
              })}
            </div>
          );
        }

        if (normalizedStyle === "matrix") {
          const segments = 6;
          const activeSegments = Math.max(1, Math.round(level * segments));
          const segmentHeight = (maxHeightPx - ((segments - 1) * 1.5)) / segments;
          return (
            <div
              key={index}
              style={{
                width: `${effectiveWidth}px`,
                height: `${maxHeightPx}px`,
                display: "flex",
                flexDirection: "column-reverse",
                gap: "1.5px",
              }}
            >
              {Array.from({ length: segments }).map((_, segmentIndex) => {
                const lit = segmentIndex < activeSegments;
                return (
                  <div
                    key={segmentIndex}
                    style={{
                      height: `${segmentHeight}px`,
                      borderRadius: "1px",
                      background: lit
                        ? `linear-gradient(180deg, rgba(255,255,255,0.92), ${recordingOverlayHexToRgba(accent, 0.74)})`
                        : recordingOverlayHexToRgba(accent, 0.08),
                      opacity: lit ? 1 : 0.36,
                      boxShadow: lit
                        ? `0 0 6px ${recordingOverlayHexToRgba(accent, 0.22)}`
                        : "none",
                      transition,
                    }}
                  />
                );
              })}
            </div>
          );
        }

        if (normalizedStyle === "morse") {
          const units = [0.24, 0.56, 0.2];
          return (
            <div
              key={index}
              style={{
                width: `${laneWidth}px`,
                height: `${maxHeightPx}px`,
                display: "flex",
                flexDirection: "column",
                justifyContent: "space-between",
              }}
            >
              {units.map((ratio, unitIndex) => {
                const isDash = unitIndex === 1;
                const lit = level > (0.16 + unitIndex * 0.16);
                return (
                  <div
                    key={unitIndex}
                    style={{
                      height: `${Math.max(3, maxHeightPx * ratio)}px`,
                      width: isDash ? "100%" : `${Math.max(4, effectiveWidth * 0.9)}px`,
                      alignSelf: "center",
                      borderRadius: "999px",
                      background: lit
                        ? `linear-gradient(90deg, ${recordingOverlayHexToRgba(accent, 0.32)}, rgba(255,255,255,0.95), ${recordingOverlayHexToRgba(accent, 0.72)})`
                        : recordingOverlayHexToRgba(accent, 0.1),
                      opacity: lit ? 1 : 0.34,
                      transition,
                    }}
                  />
                );
              })}
            </div>
          );
        }

        if (normalizedStyle === "orbit") {
          const dotSize = Math.max(4, Math.min(10, Math.round(effectiveWidth * 0.95)));
          const top = (1 - level) * Math.max(0, maxHeightPx - dotSize);
          return (
            <div
              key={index}
              style={{
                width: `${laneWidth}px`,
                height: `${maxHeightPx}px`,
                position: "relative",
              }}
            >
              <div
                style={{
                  position: "absolute",
                  top: 0,
                  bottom: 0,
                  left: "50%",
                  width: "1px",
                  transform: "translateX(-50%)",
                  background: recordingOverlayHexToRgba(accent, 0.16),
                }}
              />
              <div
                style={{
                  position: "absolute",
                  top: `${top}px`,
                  left:
                    index % 2 === 0
                      ? "0px"
                      : `${Math.max(0, laneWidth - dotSize)}px`,
                  width: `${dotSize}px`,
                  height: `${dotSize}px`,
                  borderRadius: "999px",
                  background: `radial-gradient(circle at 35% 35%, rgba(255,255,255,0.95), ${recordingOverlayHexToRgba(accent, 0.82)} 62%, ${recordingOverlayHexToRgba(accent, 0.28)} 100%)`,
                  boxShadow: `0 0 12px ${recordingOverlayHexToRgba(accent, 0.34)}`,
                  transition,
                }}
              />
            </div>
          );
        }

        if (normalizedStyle === "pulse_rings") {
          const ringSize = Math.max(10, Math.min(maxHeightPx, laneWidth - 1));
          const haloScale = 0.9 + level * 0.45;
          const ringScale = 0.45 + level * 0.75;
          const coreScale = 0.55 + level * 0.5;
          return (
            <div
              key={index}
              style={{
                width: `${laneWidth}px`,
                height: `${maxHeightPx}px`,
                position: "relative",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              <div
                style={{
                  position: "absolute",
                  width: `${ringSize}px`,
                  height: `${ringSize}px`,
                  borderRadius: "999px",
                  border: `1px solid ${recordingOverlayHexToRgba(accent, 0.16)}`,
                  transform: `scale(${haloScale})`,
                  opacity: Math.max(0.18, opacity * 0.48),
                  transition,
                }}
              />
              <div
                style={{
                  position: "absolute",
                  width: `${ringSize}px`,
                  height: `${ringSize}px`,
                  borderRadius: "999px",
                  border: `2px solid ${recordingOverlayHexToRgba(accent, 0.84)}`,
                  boxShadow: `0 0 10px ${recordingOverlayHexToRgba(accent, 0.22)}`,
                  transform: `scale(${ringScale})`,
                  opacity: Math.max(0.3, opacity),
                  transition,
                }}
              />
              <div
                style={{
                  position: "absolute",
                  width: `${Math.max(4, ringSize * 0.34)}px`,
                  height: `${Math.max(4, ringSize * 0.34)}px`,
                  borderRadius: "999px",
                  background: `radial-gradient(circle at 35% 35%, rgba(255,255,255,0.98), ${recordingOverlayHexToRgba(accent, 0.88)} 65%, ${recordingOverlayHexToRgba(accent, 0.28)} 100%)`,
                  boxShadow: `0 0 10px ${recordingOverlayHexToRgba(accent, 0.24)}`,
                  transform: `scale(${coreScale})`,
                  transition,
                }}
              />
            </div>
          );
        }

        if (normalizedStyle === "fireflies") {
          const dotSize = Math.max(3, Math.round(effectiveWidth * 0.75));
          const travel = Math.max(0, maxHeightPx - dotSize);
          const primaryTop = (1 - easedLevel) * travel;
          const secondaryTop =
            (((Math.sin((index * 0.72) + (easedLevel * 2.8)) + 1) / 2) * travel * 0.7) +
            (maxHeightPx * 0.06);
          const tertiaryTop =
            (((Math.cos((index * 0.82) + (easedLevel * 3.1)) + 1) / 2) * travel * 0.56) +
            (maxHeightPx * 0.14);
          const glowScale = 0.86 + easedLevel * 0.26;
          return (
            <div
              key={index}
              style={{
                width: `${laneWidth}px`,
                height: `${maxHeightPx}px`,
                position: "relative",
              }}
            >
              {[
                { top: primaryTop, left: 0.08, size: dotSize * 1.15, alpha: 0.98 },
                { top: secondaryTop, left: 0.48, size: dotSize * 0.9, alpha: 0.64 },
                { top: tertiaryTop, left: 0.76, size: dotSize * 0.72, alpha: 0.42 },
              ].map((dot, dotIndex) => (
                <div
                  key={dotIndex}
                  style={{
                    position: "absolute",
                    top: `${dot.top}px`,
                    left: `${Math.max(0, (laneWidth - dot.size) * dot.left)}px`,
                    width: `${dot.size}px`,
                    height: `${dot.size}px`,
                    borderRadius: "999px",
                    background: `radial-gradient(circle at 35% 35%, rgba(255,255,255,0.98), ${recordingOverlayHexToRgba(accent, 0.84)} 58%, ${recordingOverlayHexToRgba(accent, 0.16)} 100%)`,
                    opacity: Math.max(0.22, dot.alpha * opacity),
                    boxShadow: `0 0 ${6 + (dotIndex * 2)}px ${recordingOverlayHexToRgba(accent, 0.22)}`,
                    transform: `scale(${glowScale - (dotIndex * 0.08)})`,
                    transition,
                  }}
                />
              ))}
            </div>
          );
        }

        if (normalizedStyle === "helix") {
          const inset = 3;
          const strandWidth = Math.max(2, effectiveWidth * 0.42);
          const travel = Math.max(0, maxHeightPx - (inset * 2));
          const phase = (index * 0.6) + (easedLevel * 3.2);
          const leftY = inset + (((Math.sin(phase) + 1) / 2) * travel);
          const rightY = inset + (((Math.sin(phase + Math.PI) + 1) / 2) * travel);
          const midTop = inset + (((Math.sin(phase + Math.PI / 2) + 1) / 2) * travel);
          const midBottom =
            inset + (((Math.sin(phase + (Math.PI * 1.5)) + 1) / 2) * travel);
          return (
            <div
              key={index}
              style={{
                width: `${laneWidth}px`,
                height: `${maxHeightPx}px`,
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              <svg
                width={laneWidth}
                height={maxHeightPx}
                viewBox={`0 0 ${laneWidth} ${maxHeightPx}`}
                style={{ overflow: "visible" }}
              >
                <line
                  x1={4}
                  y1={leftY}
                  x2={laneWidth - 4}
                  y2={rightY}
                  stroke={recordingOverlayHexToRgba(accent, 0.48)}
                  strokeWidth={1.2}
                  strokeLinecap="round"
                  style={{ transition }}
                />
                <line
                  x1={4}
                  y1={midTop}
                  x2={laneWidth - 4}
                  y2={midBottom}
                  stroke={recordingOverlayHexToRgba(accent, 0.22)}
                  strokeWidth={1}
                  strokeLinecap="round"
                  style={{ transition }}
                />
                <circle
                  cx={4}
                  cy={leftY}
                  r={strandWidth}
                  fill={recordingOverlayHexToRgba(accent, 0.9)}
                  style={{ transition }}
                />
                <circle
                  cx={laneWidth - 4}
                  cy={rightY}
                  r={strandWidth}
                  fill="rgba(255,255,255,0.95)"
                  style={{ transition }}
                />
              </svg>
            </div>
          );
        }

        if (normalizedStyle === "constellation") {
          const nodeX = [2, laneWidth * 0.32, laneWidth * 0.68, laneWidth - 2];
          const upperY = 3 + ((1 - easedLevel) * maxHeightPx * 0.52);
          const midY = (maxHeightPx * 0.36) + (Math.sin(index + easedLevel * 2.4) * 1.8);
          const lowerY = (maxHeightPx * 0.66) - (easedLevel * maxHeightPx * 0.24);
          const tailY = maxHeightPx - 3 - (((Math.cos(index * 0.54 + easedLevel * 2.7) + 1) / 2) * maxHeightPx * 0.16);
          const points = [
            `${nodeX[0]},${upperY}`,
            `${nodeX[1]},${midY}`,
            `${nodeX[2]},${lowerY}`,
            `${nodeX[3]},${tailY}`,
          ].join(" ");
          return (
            <div
              key={index}
              style={{
                width: `${laneWidth}px`,
                height: `${maxHeightPx}px`,
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              <svg width={laneWidth} height={maxHeightPx} viewBox={`0 0 ${laneWidth} ${maxHeightPx}`}>
                <polyline
                  points={points}
                  fill="none"
                  stroke={recordingOverlayHexToRgba(accent, 0.58)}
                  strokeWidth={1.4}
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  style={{ transition }}
                />
                {[
                  { x: nodeX[0], y: upperY, r: 1.7 },
                  { x: nodeX[1], y: midY, r: 1.5 },
                  { x: nodeX[2], y: lowerY, r: 1.7 },
                  { x: nodeX[3], y: tailY, r: 2 },
                ].map((node, nodeIndex) => (
                  <circle
                    key={nodeIndex}
                    cx={node.x}
                    cy={node.y}
                    r={node.r + (level * 0.45)}
                    fill={nodeIndex === 3 ? "rgba(255,255,255,0.96)" : recordingOverlayHexToRgba(accent, 0.88)}
                    style={{ transition }}
                  />
                ))}
              </svg>
            </div>
          );
        }

        if (normalizedStyle === "petals") {
          const petalSize = Math.max(8, laneWidth - 3);
          const bloom = 0.6 + easedLevel * 0.42;
          const rotation = (index * 8) + (easedLevel * 18);
          return (
            <div
              key={index}
              style={{
                width: `${laneWidth}px`,
                height: `${maxHeightPx}px`,
                position: "relative",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              {[0, 90, 180, 270].map((angle, petalIndex) => (
                <div
                  key={petalIndex}
                  style={{
                    position: "absolute",
                    width: `${Math.max(4, effectiveWidth * 0.9)}px`,
                    height: `${petalSize}px`,
                    borderRadius: "999px",
                    background: `linear-gradient(180deg, rgba(255,255,255,0.96), ${recordingOverlayHexToRgba(accent, 0.84)})`,
                    boxShadow: `0 0 10px ${recordingOverlayHexToRgba(accent, 0.16)}`,
                    opacity: Math.max(0.28, opacity - (petalIndex * 0.08)),
                    transform: `translate(-50%, -50%) rotate(${angle + rotation}deg) scale(${bloom - (petalIndex * 0.06)})`,
                    left: "50%",
                    top: "50%",
                    transformOrigin: "center center",
                    transition,
                  }}
                />
              ))}
              <div
                style={{
                  position: "absolute",
                  width: `${Math.max(4, effectiveWidth * 0.72)}px`,
                  height: `${Math.max(4, effectiveWidth * 0.72)}px`,
                  borderRadius: "999px",
                  background: `radial-gradient(circle at 35% 35%, rgba(255,255,255,0.98), ${recordingOverlayHexToRgba(accent, 0.92)} 70%)`,
                  boxShadow: `0 0 8px ${recordingOverlayHexToRgba(accent, 0.24)}`,
                  transform: `translate(-50%, -50%) scale(${0.8 + level * 0.26})`,
                  left: "50%",
                  top: "50%",
                  transition,
                }}
              />
            </div>
          );
        }

        if (normalizedStyle === "petal_rain") {
          const petalCount = level < 0.2 ? 2 : level < 0.55 ? 3 : 4;
          const petalSize = Math.max(4, effectiveWidth * 0.78);
          return (
            <div
              key={index}
              style={{
                width: `${laneWidth}px`,
                height: `${maxHeightPx}px`,
                position: "relative",
                overflow: "hidden",
              }}
            >
              {Array.from({ length: petalCount }).map((_, petalIndex) => {
                const phase = (easedLevel * 0.82) + (((index + petalIndex) % 5) * 0.13);
                const progress = phase % 1;
                const gravity = easeInQuad(progress);
                const top = (-petalSize * 1.4) + (gravity * (maxHeightPx + (petalSize * 1.9)));
                const drift = Math.sin((index * 0.7) + (petalIndex * 1.3) + (easedLevel * 2.1)) * (laneWidth * 0.16);
                const leftBase = laneWidth * (0.16 + (petalIndex * 0.2));
                const rotation = (-24 + (petalIndex * 14)) + (gravity * 36);
                const scale = 0.72 + (easedLevel * 0.42) - (petalIndex * 0.05);
                return (
                  <div
                    key={petalIndex}
                    style={{
                      position: "absolute",
                      top: `${top}px`,
                      left: `${leftBase + drift}px`,
                      width: `${petalSize}px`,
                      height: `${Math.max(8, petalSize * 1.7)}px`,
                      borderRadius: "70% 70% 70% 70% / 90% 90% 55% 55%",
                      background: `linear-gradient(180deg, rgba(255,255,255,0.96), ${recordingOverlayHexToRgba(accent, 0.78)} 58%, ${recordingOverlayHexToRgba(accent, 0.26)} 100%)`,
                      boxShadow: `0 0 8px ${recordingOverlayHexToRgba(accent, 0.14)}`,
                      opacity: Math.max(0.24, (1 - (petalIndex * 0.14)) * opacity),
                      transform: `rotate(${rotation}deg) scale(${scale})`,
                      transition,
                    }}
                  />
                );
              })}
            </div>
          );
        }

        if (normalizedStyle === "daisy") {
          const centerX = laneWidth / 2;
          const headY = maxHeightPx * 0.5;
          const blossomScale = 0.72 + (easedLevel * 0.48);
          const sway = Math.sin((index * 0.5) + (easedLevel * 2.3)) * 6;
          return (
            <div
              key={index}
              style={{
                width: `${laneWidth}px`,
                height: `${maxHeightPx}px`,
                position: "relative",
              }}
            >
              <div
                style={{
                  position: "absolute",
                  left: `${centerX - 1}px`,
                  top: `${headY + 4}px`,
                  width: "2px",
                  height: `${Math.max(5, maxHeightPx - headY - 4)}px`,
                  borderRadius: "999px",
                  background: `linear-gradient(180deg, ${recordingOverlayHexToRgba(accent, 0.16)}, ${recordingOverlayHexToRgba(accent, 0.7)})`,
                  transform: `translateX(${sway * 0.1}px)`,
                  transition,
                }}
              />
              {[0, 60, 120, 180, 240, 300].map((angle, petalIndex) => (
                <div
                  key={petalIndex}
                  style={{
                    position: "absolute",
                    left: "50%",
                    top: `${headY}px`,
                    width: `${Math.max(5, effectiveWidth * 0.86)}px`,
                    height: `${Math.max(10, laneWidth * 0.9)}px`,
                    borderRadius: "999px",
                    background: `linear-gradient(180deg, rgba(255,255,255,0.98), ${recordingOverlayHexToRgba(accent, 0.5)})`,
                    opacity: Math.max(0.34, opacity - (petalIndex * 0.05)),
                    boxShadow: `0 0 8px ${recordingOverlayHexToRgba(accent, 0.12)}`,
                    transform: `translate(-50%, -50%) rotate(${angle + sway}deg) scale(${blossomScale})`,
                    transition,
                  }}
                />
              ))}
              <div
                style={{
                  position: "absolute",
                  left: "50%",
                  top: `${headY}px`,
                  width: `${Math.max(5, effectiveWidth * 0.9)}px`,
                  height: `${Math.max(5, effectiveWidth * 0.9)}px`,
                  borderRadius: "999px",
                  background: `radial-gradient(circle at 35% 35%, rgba(255,250,224,0.98), ${recordingOverlayHexToRgba(accent, 0.94)} 72%)`,
                  transform: `translate(-50%, -50%) scale(${0.82 + (easedLevel * 0.26)})`,
                  boxShadow: `0 0 9px ${recordingOverlayHexToRgba(accent, 0.18)}`,
                  transition,
                }}
              />
            </div>
          );
        }

        if (normalizedStyle === "lotus") {
          const baseY = maxHeightPx * 0.76;
          const blossomHeight = Math.max(12, laneWidth * 0.95);
          const blossomWidth = Math.max(6, effectiveWidth * 0.84);
          const openness = 0.42 + (easedLevel * 0.5);
          return (
            <div
              key={index}
              style={{
                width: `${laneWidth}px`,
                height: `${maxHeightPx}px`,
                position: "relative",
              }}
            >
              {[0, 1, 2, 3, 4].map((petalIndex) => {
                const xOffset = (petalIndex - 2) * (laneWidth * 0.12) * openness;
                const rotation = (petalIndex - 2) * (12 + (openness * 10));
                const scale = 0.82 + (easedLevel * 0.22) - (Math.abs(petalIndex - 2) * 0.05);
                return (
                  <div
                    key={petalIndex}
                    style={{
                      position: "absolute",
                      left: `calc(50% + ${xOffset}px)`,
                      top: `${baseY - (petalIndex === 2 ? blossomHeight * 0.38 : blossomHeight * 0.18)}px`,
                      width: `${blossomWidth}px`,
                      height: `${blossomHeight}px`,
                      borderRadius: "75% 75% 30% 30% / 92% 92% 24% 24%",
                      background: `linear-gradient(180deg, rgba(255,255,255,0.97), ${recordingOverlayHexToRgba(accent, 0.84)} 64%, ${recordingOverlayHexToRgba(accent, 0.22)} 100%)`,
                      opacity: Math.max(0.32, opacity - (Math.abs(petalIndex - 2) * 0.06)),
                      transform: `translate(-50%, -50%) rotate(${rotation}deg) scale(${scale})`,
                      transformOrigin: "center bottom",
                      boxShadow: `0 0 10px ${recordingOverlayHexToRgba(accent, 0.14)}`,
                      transition,
                    }}
                  />
                );
              })}
              <div
                style={{
                  position: "absolute",
                  left: "50%",
                  top: `${baseY + 1}px`,
                  width: `${laneWidth * 0.8}px`,
                  height: `${Math.max(4, effectiveWidth * 0.38)}px`,
                  borderRadius: "999px",
                  background: `linear-gradient(90deg, ${recordingOverlayHexToRgba(accent, 0.14)}, ${recordingOverlayHexToRgba(accent, 0.46)}, ${recordingOverlayHexToRgba(accent, 0.14)})`,
                  transform: "translateX(-50%)",
                  transition,
                }}
              />
            </div>
          );
        }

        if (normalizedStyle === "garden_sway") {
          const sway = Math.sin((index * 0.45) + (easedLevel * 2.1)) * 7;
          const headY = maxHeightPx * 0.38;
          const bloomSize = Math.max(6, effectiveWidth * (1.15 + (easedLevel * 0.32)));
          return (
            <div
              key={index}
              style={{
                width: `${laneWidth}px`,
                height: `${maxHeightPx}px`,
                position: "relative",
              }}
            >
              <svg
                width={laneWidth}
                height={maxHeightPx}
                viewBox={`0 0 ${laneWidth} ${maxHeightPx}`}
              >
                <path
                  d={`M ${laneWidth / 2} ${maxHeightPx} C ${laneWidth / 2} ${maxHeightPx * 0.78}, ${laneWidth / 2 + (sway * 0.28)} ${maxHeightPx * 0.58}, ${laneWidth / 2 + sway} ${headY + 3}`}
                  fill="none"
                  stroke={recordingOverlayHexToRgba(accent, 0.68)}
                  strokeWidth={2}
                  strokeLinecap="round"
                  style={{ transition }}
                />
                <ellipse
                  cx={(laneWidth / 2) + (sway * 0.22)}
                  cy={maxHeightPx * 0.66}
                  rx={Math.max(3, effectiveWidth * 0.42)}
                  ry={Math.max(2, effectiveWidth * 0.24)}
                  fill={recordingOverlayHexToRgba(accent, 0.28)}
                  transform={`rotate(${-28 + (sway * 0.5)} ${(laneWidth / 2) + (sway * 0.22)} ${maxHeightPx * 0.66})`}
                  style={{ transition }}
                />
                <ellipse
                  cx={(laneWidth / 2) - (sway * 0.16)}
                  cy={maxHeightPx * 0.77}
                  rx={Math.max(3, effectiveWidth * 0.42)}
                  ry={Math.max(2, effectiveWidth * 0.24)}
                  fill={recordingOverlayHexToRgba(accent, 0.22)}
                  transform={`rotate(${28 + (sway * 0.42)} ${(laneWidth / 2) - (sway * 0.16)} ${maxHeightPx * 0.77})`}
                  style={{ transition }}
                />
              </svg>
              {[0, 120, 240].map((angle, petalIndex) => (
                <div
                  key={petalIndex}
                  style={{
                    position: "absolute",
                    left: `calc(50% + ${sway}px)`,
                    top: `${headY}px`,
                    width: `${Math.max(6, effectiveWidth * 0.9)}px`,
                    height: `${bloomSize}px`,
                    borderRadius: "999px",
                    background: `linear-gradient(180deg, rgba(255,255,255,0.98), ${recordingOverlayHexToRgba(accent, 0.82)})`,
                    boxShadow: `0 0 8px ${recordingOverlayHexToRgba(accent, 0.14)}`,
                    opacity: Math.max(0.34, opacity - (petalIndex * 0.06)),
                    transform: `translate(-50%, -50%) rotate(${angle + (sway * 0.8)}deg) scale(${0.82 + (easedLevel * 0.36)})`,
                    transition,
                  }}
                />
              ))}
              <div
                style={{
                  position: "absolute",
                  left: `calc(50% + ${sway}px)`,
                  top: `${headY}px`,
                  width: `${Math.max(4, effectiveWidth * 0.68)}px`,
                  height: `${Math.max(4, effectiveWidth * 0.68)}px`,
                  borderRadius: "999px",
                  background: `radial-gradient(circle at 35% 35%, rgba(255,251,232,0.98), ${recordingOverlayHexToRgba(accent, 0.92)} 70%)`,
                  transform: `translate(-50%, -50%) scale(${0.84 + (easedLevel * 0.22)})`,
                  transition,
                }}
              />
            </div>
          );
        }

        if (normalizedStyle === "tuner") {
          const markerHeight = Math.max(4, Math.round(maxHeightPx * 0.18));
          const markerTop = (1 - level) * Math.max(0, maxHeightPx - markerHeight);
          return (
            <div
              key={index}
              style={{
                width: `${laneWidth}px`,
                height: `${maxHeightPx}px`,
                position: "relative",
              }}
            >
              <div
                style={{
                  position: "absolute",
                  top: 0,
                  bottom: 0,
                  left: "50%",
                  width: "2px",
                  transform: "translateX(-50%)",
                  background: recordingOverlayHexToRgba(accent, 0.14),
                  borderRadius: "999px",
                }}
              />
              <div
                style={{
                  position: "absolute",
                  top: `${markerTop}px`,
                  left: "50%",
                  width: `${laneWidth}px`,
                  height: `${markerHeight}px`,
                  transform: "translateX(-50%)",
                  borderRadius: "999px",
                  background: `linear-gradient(90deg, ${recordingOverlayHexToRgba(accent, 0.3)}, rgba(255,255,255,0.97), ${recordingOverlayHexToRgba(accent, 0.84)})`,
                  boxShadow: `0 0 8px ${recordingOverlayHexToRgba(accent, 0.22)}`,
                  transition,
                }}
              />
            </div>
          );
        }

        if (normalizedStyle === "vinyl") {
          const discSize = Math.max(6, Math.min(maxHeightPx, effectiveWidth + 6));
          const innerSize = Math.max(2, discSize * (0.2 + level * 0.2));
          return (
            <div
              key={index}
              style={{
                width: `${laneWidth}px`,
                height: `${maxHeightPx}px`,
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              <div
                style={{
                  width: `${discSize}px`,
                  height: `${discSize}px`,
                  borderRadius: "999px",
                  background: `radial-gradient(circle at 50% 50%, rgba(15,15,15,0.95) ${Math.max(8, innerSize)}%, ${recordingOverlayHexToRgba(accent, 0.16)} ${Math.max(9, innerSize + 1)}%, rgba(15,15,15,0.92) 62%, ${recordingOverlayHexToRgba(accent, 0.38)} 100%)`,
                  boxShadow: `0 0 10px ${recordingOverlayHexToRgba(accent, 0.18)}`,
                  transform: `scale(${0.74 + level * 0.26})`,
                  transition,
                }}
              />
            </div>
          );
        }

        if (normalizedStyle === "radar") {
          return (
            <div
              key={index}
              style={{
                width: `${effectiveWidth}px`,
                height: `${maxHeightPx}px`,
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              <div
                style={{
                  width: `${effectiveWidth}px`,
                  height: `${maxHeightPx}px`,
                  borderRadius: "999px",
                  background: `linear-gradient(180deg, ${recordingOverlayHexToRgba(accent, 0.3)}, rgba(255,255,255,0.95), ${recordingOverlayHexToRgba(accent, 0.3)})`,
                  opacity: Math.max(0.35, opacity),
                  boxShadow: `0 0 12px ${recordingOverlayHexToRgba(accent, 0.24)}`,
                  transform: `scaleY(${0.22 + level * 0.78})`,
                  transformOrigin: "center",
                  transition,
                }}
              />
            </div>
          );
        }

        let style: React.CSSProperties = {
          width: `${effectiveWidth}px`,
          height: `${height}px`,
          minHeight: "4px",
          transition,
        };

        switch (normalizedStyle) {
          case "aurora":
            style = {
              ...style,
              background: `linear-gradient(180deg, rgba(255,255,255,0.94) 0%, ${recordingOverlayHexToRgba(accent, 0.86)} 34%, ${recordingOverlayHexToRgba(accent, 0.22)} 100%)`,
              borderRadius: "999px 999px 3px 3px",
              opacity: Math.max(0.4, opacity),
              boxShadow: `0 0 12px ${recordingOverlayHexToRgba(accent, 0.18)}`,
              transform: `scaleY(${0.95 + level * 0.08}) translateY(${pulseOffset(level, index)}px)`,
            };
            break;
          case "capsule":
            style = {
              ...style,
              background: `linear-gradient(180deg, ${recordingOverlayHexToRgba(accent, 0.98)}, ${recordingOverlayHexToRgba(accent, 0.44)})`,
              borderRadius: "999px",
              opacity: Math.max(0.35, opacity),
            };
            break;
          case "comet":
            style = {
              ...style,
              background: `linear-gradient(180deg, rgba(255,255,255,0.96) 0%, ${recordingOverlayHexToRgba(accent, 0.76)} 18%, ${recordingOverlayHexToRgba(accent, 0.18)} 100%)`,
              borderRadius: "999px 999px 4px 4px",
              opacity: Math.max(0.38, opacity),
              boxShadow: `0 -3px 10px ${recordingOverlayHexToRgba(accent, 0.24)}`,
            };
            break;
          case "crown":
            style = {
              ...style,
              background: `linear-gradient(180deg, rgba(255,255,255,0.92), ${recordingOverlayHexToRgba(accent, 0.72)})`,
              clipPath: "polygon(0% 100%, 0% 38%, 18% 0%, 38% 38%, 50% 14%, 62% 38%, 82% 0%, 100% 38%, 100% 100%)",
              opacity: Math.max(0.42, opacity),
              boxShadow: `0 0 8px ${recordingOverlayHexToRgba(accent, 0.16)}`,
            };
            break;
          case "ember":
            style = {
              ...style,
              background: `linear-gradient(180deg, rgba(255,255,255,0.2), ${recordingOverlayHexToRgba(accent, 0.66)} 38%, ${recordingOverlayHexToRgba(accent, 0.9)} 100%)`,
              borderRadius: "999px 999px 5px 5px",
              opacity: Math.max(0.48, opacity),
              boxShadow: `0 0 10px ${recordingOverlayHexToRgba(accent, 0.26)}, 0 6px 12px ${recordingOverlayHexToRgba(accent, 0.18)}`,
            };
            break;
          case "glow":
            style = {
              ...style,
              background: `linear-gradient(180deg, ${recordingOverlayHexToRgba(accent, 1)}, ${recordingOverlayHexToRgba(accent, 0.42)})`,
              borderRadius: "3px",
              opacity,
              boxShadow: `0 0 10px ${recordingOverlayHexToRgba(accent, 0.34)}, 0 0 3px ${recordingOverlayHexToRgba(accent, 0.66)}`,
              transform: `translateY(${index % 2 === 0 ? "0" : "0.5px"})`,
            };
            break;
          case "hologram":
            style = {
              ...style,
              background: `repeating-linear-gradient(180deg, rgba(255,255,255,0.82) 0px, rgba(255,255,255,0.82) 1px, ${recordingOverlayHexToRgba(accent, 0.66)} 1px, ${recordingOverlayHexToRgba(accent, 0.66)} 3px, ${recordingOverlayHexToRgba(accent, 0.16)} 3px, ${recordingOverlayHexToRgba(accent, 0.16)} 5px)`,
              borderRadius: "2px",
              opacity: Math.max(0.38, opacity),
              boxShadow: `0 0 8px ${recordingOverlayHexToRgba(accent, 0.18)}`,
            };
            break;
          case "prism":
            style = {
              ...style,
              background: `linear-gradient(180deg, ${recordingOverlayHexToRgba(accent, 0.98)} 0%, rgba(255,255,255,0.92) 34%, ${recordingOverlayHexToRgba(accent, 0.38)} 100%)`,
              borderRadius: "1px",
              opacity: Math.max(0.4, opacity),
              boxShadow: `inset 0 1px 0 rgba(255,255,255,0.45), 0 0 0 1px ${recordingOverlayHexToRgba(accent, 0.18)}`,
              transform: `skewX(${index % 2 === 0 ? "-5deg" : "5deg"})`,
            };
            break;
          case "shards":
            style = {
              ...style,
              background: `linear-gradient(180deg, rgba(255,255,255,0.9), ${recordingOverlayHexToRgba(accent, 0.78)} 55%, ${recordingOverlayHexToRgba(accent, 0.22)})`,
              clipPath:
                index % 2 === 0
                  ? "polygon(18% 0%, 100% 0%, 80% 100%, 0% 100%)"
                  : "polygon(0% 0%, 82% 0%, 100% 100%, 20% 100%)",
              opacity: Math.max(0.42, opacity),
              boxShadow: `0 0 10px ${recordingOverlayHexToRgba(accent, 0.18)}`,
              transform: `translateY(${(1 - level) * 1.8}px)`,
            };
            break;
          case "skyline":
            style = {
              ...style,
              background: `linear-gradient(180deg, rgba(255,255,255,0.1), ${recordingOverlayHexToRgba(accent, 0.8)})`,
              clipPath:
                index % 3 === 0
                  ? "polygon(0% 100%, 0% 32%, 22% 32%, 22% 18%, 58% 18%, 58% 0%, 100% 0%, 100% 100%)"
                  : index % 3 === 1
                    ? "polygon(0% 100%, 0% 20%, 36% 20%, 36% 4%, 72% 4%, 72% 28%, 100% 28%, 100% 100%)"
                    : "polygon(0% 100%, 0% 26%, 30% 26%, 30% 12%, 52% 12%, 52% 0%, 100% 0%, 100% 100%)",
              opacity: Math.max(0.42, opacity),
              boxShadow: `inset 0 1px 0 rgba(255,255,255,0.18)`,
            };
            break;
          case "needles":
            style = {
              ...style,
              width: `${Math.max(2, Math.round(effectiveWidth * 0.45))}px`,
              background: `linear-gradient(180deg, rgba(255,255,255,0.95), ${recordingOverlayHexToRgba(accent, 0.72)})`,
              borderRadius: "999px",
              opacity: Math.max(0.5, opacity),
              boxShadow: `0 0 8px ${recordingOverlayHexToRgba(accent, 0.28)}`,
            };
            break;
          case "solid":
            style = {
              ...style,
              background: recordingOverlayHexToRgba(accent, 0.9),
              borderRadius: "2px",
              opacity,
            };
            break;
          default:
            style = {
              ...style,
              background: recordingOverlayHexToRgba(accent, 0.9),
              borderRadius: "2px",
              opacity,
            };
            break;
        }

        return <div key={index} style={style} />;
      })}
    </div>
  );
};
