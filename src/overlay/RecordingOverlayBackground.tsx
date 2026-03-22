import React from "react";
import {
  normalizeRecordingOverlayBackgroundMode,
  normalizeRecordingOverlayColor,
  recordingOverlayHexToRgba,
  type RecordingOverlayBackgroundMode,
} from "./recordingOverlayAppearance";

interface RecordingOverlayBackgroundProps {
  mode: RecordingOverlayBackgroundMode;
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
    levels.reduce((total, value) => total + clampUnit(value), 0) / levels.length,
  );
}

function glowCircle(
  accent: string,
  alpha: number,
  edgeAlpha: number,
): string {
  return `radial-gradient(circle at 50% 50%, ${recordingOverlayHexToRgba(accent, alpha)} 0%, ${recordingOverlayHexToRgba(accent, edgeAlpha)} 42%, ${recordingOverlayHexToRgba(accent, 0)} 74%)`;
}

export const RecordingOverlayBackground: React.FC<
  RecordingOverlayBackgroundProps
> = ({
  mode,
  accentColor,
  levels,
  animationSoftnessPercent = 55,
  depthParallaxPercent = 40,
}) => {
  const normalizedMode = normalizeRecordingOverlayBackgroundMode(mode);
  if (normalizedMode === "none") {
    return null;
  }

  const accent = normalizeRecordingOverlayColor(accentColor);
  const energy = averageEnergy(levels);
  const softness = Math.max(0, Math.min(100, Math.round(animationSoftnessPercent))) / 100;
  const parallax = Math.max(0, Math.min(100, Math.round(depthParallaxPercent))) / 100;
  const transitionMs = Math.round(180 + (softness * 220));
  const driftScale = (1.08 - (softness * 0.3)) * (0.65 + (parallax * 0.55));
  const bloomScale = 0.82 + (energy * (0.42 - (softness * 0.12)));

  return (
    <div
      aria-hidden="true"
      style={{
        position: "absolute",
        inset: 0,
        overflow: "hidden",
        borderRadius: "inherit",
        pointerEvents: "none",
        zIndex: 0,
      }}
    >
      <div
        style={{
          position: "absolute",
          inset: "1px",
          borderRadius: "inherit",
          background: `linear-gradient(180deg, ${recordingOverlayHexToRgba(accent, 0.14 + (energy * 0.06))} 0%, ${recordingOverlayHexToRgba(accent, 0.03)} 24%, rgba(255,255,255,0) 58%)`,
          opacity: 0.8 - (softness * 0.18),
          transform: `translateY(${(0.5 - energy) * 3 * driftScale}px)`,
          transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1), opacity ${transitionMs}ms ease-out`,
        }}
      />

      <div
        style={{
          position: "absolute",
          inset: 0,
          borderRadius: "inherit",
          background: "linear-gradient(180deg, rgba(255,255,255,0.08) 0%, rgba(255,255,255,0) 24%, rgba(0,0,0,0.14) 100%)",
          mixBlendMode: "screen",
          opacity: 0.42,
        }}
      />

      {normalizedMode === "mist" &&
        <>
          {[
            { top: "-10%", left: "-8%", size: "58%", alpha: 0.18 },
            { top: "12%", left: "34%", size: "52%", alpha: 0.14 },
            { top: "32%", left: "62%", size: "46%", alpha: 0.12 },
          ].map((blob, index) => (
            <div
              key={index}
              style={{
                position: "absolute",
                top: blob.top,
                left: blob.left,
                width: blob.size,
                aspectRatio: "1 / 1",
                borderRadius: "999px",
                background: glowCircle(accent, blob.alpha + (energy * 0.08), 0),
                filter: `blur(${24 + (index * 6)}px)`,
                opacity: 0.86,
                transform: `translate(${(index - 1) * energy * 14 * driftScale}px, ${(1 - index) * energy * 6 * driftScale}px) scale(${bloomScale - (index * 0.08)})`,
                transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1), opacity ${transitionMs}ms ease-out`,
              }}
            />
          ))}
          <div
            style={{
              position: "absolute",
              top: "16%",
              left: "-18%",
              width: "136%",
              height: "36%",
              borderRadius: "999px",
              background: `linear-gradient(90deg, ${recordingOverlayHexToRgba(accent, 0)} 0%, ${recordingOverlayHexToRgba(accent, 0.09 + (energy * 0.05))} 24%, ${recordingOverlayHexToRgba(accent, 0.13 + (energy * 0.07))} 50%, ${recordingOverlayHexToRgba(accent, 0.08)} 76%, ${recordingOverlayHexToRgba(accent, 0)} 100%)`,
              filter: "blur(18px)",
              opacity: 0.9,
              transform: `translate(${(energy - 0.4) * 16 * driftScale}px, ${Math.sin(energy * Math.PI) * 4}px) rotate(${-6 + (energy * 10)}deg)`,
              transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1), opacity ${transitionMs}ms ease-out`,
            }}
          />
        </>}

      {normalizedMode === "petals_haze" &&
        <>
          {[
            { top: "8%", left: "10%", rotate: -24, scale: 1 },
            { top: "22%", left: "36%", rotate: 18, scale: 0.92 },
            { top: "6%", left: "60%", rotate: 34, scale: 0.88 },
            { top: "38%", left: "18%", rotate: -12, scale: 0.9 },
            { top: "34%", left: "66%", rotate: 22, scale: 0.84 },
          ].map((petal, index) => (
            <div
              key={index}
              style={{
                position: "absolute",
                top: petal.top,
                left: petal.left,
                width: `${18 + (index * 3)}px`,
                height: `${38 + (index * 5)}px`,
                borderRadius: "75% 75% 35% 35% / 92% 92% 28% 28%",
                background: `linear-gradient(180deg, ${recordingOverlayHexToRgba(accent, 0.14 + (energy * 0.08))}, ${recordingOverlayHexToRgba(accent, 0.03)})`,
                boxShadow: `0 0 ${8 + (index * 2)}px ${recordingOverlayHexToRgba(accent, 0.08)}`,
                filter: `blur(${1 + (index % 2)}px)`,
                opacity: 0.74 - (index * 0.08),
                transform: `translate(${Math.sin(index + energy * 2.4) * 10 * driftScale}px, ${Math.cos(index + energy * 1.8) * 6 * driftScale}px) rotate(${petal.rotate + (energy * 18 * (index % 2 === 0 ? 1 : -1))}deg) scale(${petal.scale * bloomScale})`,
                transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1), opacity ${transitionMs}ms ease-out`,
              }}
            />
          ))}
          {[
            { top: "14%", left: "26%", size: 4 },
            { top: "24%", left: "74%", size: 5 },
            { top: "46%", left: "54%", size: 3 },
          ].map((sparkle, index) => (
            <div
              key={`sparkle-${index}`}
              style={{
                position: "absolute",
                top: sparkle.top,
                left: sparkle.left,
                width: `${sparkle.size}px`,
                height: `${sparkle.size}px`,
                borderRadius: "999px",
                background: recordingOverlayHexToRgba("#ffffff", 0.55 + (energy * 0.2)),
                boxShadow: `0 0 10px ${recordingOverlayHexToRgba(accent, 0.18 + (energy * 0.1))}`,
                opacity: 0.42 + (energy * 0.2),
                transform: `scale(${0.8 + (energy * 0.3)})`,
                transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1), opacity ${transitionMs}ms ease-out`,
              }}
            />
          ))}
        </>}

      {normalizedMode === "soft_glow_field" &&
        <>
          {[
            { top: "16%", left: "8%", size: "24%" },
            { top: "6%", left: "34%", size: "18%" },
            { top: "20%", left: "52%", size: "22%" },
            { top: "36%", left: "26%", size: "20%" },
            { top: "30%", left: "68%", size: "18%" },
          ].map((glow, index) => (
            <div
              key={index}
              style={{
                position: "absolute",
                top: glow.top,
                left: glow.left,
                width: glow.size,
                aspectRatio: "1 / 1",
                borderRadius: "999px",
                background: glowCircle(accent, 0.18 + (energy * 0.1), 0.04),
                filter: `blur(${10 + (index * 2)}px)`,
                opacity: 0.78 - (index * 0.08),
                transform: `translate(${(index - 2) * energy * 8 * driftScale}px, ${(2 - index) * energy * 4 * driftScale}px) scale(${bloomScale - (index * 0.04)})`,
                transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1), opacity ${transitionMs}ms ease-out`,
              }}
            />
          ))}
          <div
            style={{
              position: "absolute",
              top: "-18%",
              left: "10%",
              width: "84%",
              height: "70%",
              borderRadius: "999px",
              background: `linear-gradient(120deg, ${recordingOverlayHexToRgba(accent, 0)} 0%, ${recordingOverlayHexToRgba(accent, 0.18 + (energy * 0.08))} 42%, ${recordingOverlayHexToRgba("#ffffff", 0.12)} 55%, ${recordingOverlayHexToRgba(accent, 0.06)} 70%, ${recordingOverlayHexToRgba(accent, 0)} 100%)`,
              filter: "blur(16px)",
              opacity: 0.75,
              transform: `translate(${(energy - 0.35) * 24 * driftScale}px, ${(0.4 - energy) * 8}px) rotate(${8 - (energy * 14)}deg)`,
              transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1), opacity ${transitionMs}ms ease-out`,
            }}
          />
        </>}

      {normalizedMode === "stardust" &&
        <>
          {Array.from({ length: 16 }).map((_, index) => {
            const x = ((index * 17) % 88) + 6;
            const y = ((index * 11) % 62) + 10;
            const size = 1.8 + ((index % 3) * 0.8);
            return (
              <div
                key={index}
                style={{
                  position: "absolute",
                  left: `${x}%`,
                  top: `${y}%`,
                  width: `${size}px`,
                  height: `${size}px`,
                  borderRadius: "999px",
                  background: recordingOverlayHexToRgba("#ffffff", 0.34 + (energy * 0.18)),
                  boxShadow: `0 0 ${6 + (size * 2)}px ${recordingOverlayHexToRgba(accent, 0.14 + (energy * 0.08))}`,
                  opacity: 0.28 + ((index % 4) * 0.08) + (energy * 0.12),
                  transform: `translate(${Math.sin(index + energy * 2.2) * 8 * driftScale}px, ${Math.cos(index + energy * 1.7) * 5 * driftScale}px) scale(${0.8 + (energy * 0.28)})`,
                  transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1), opacity ${transitionMs}ms ease-out`,
                }}
              />
            );
          })}
        </>}

      {normalizedMode === "silk_fog" &&
        <>
          {[
            { top: "12%", left: "-12%", width: "132%", rotate: -5 },
            { top: "42%", left: "-6%", width: "118%", rotate: 4 },
          ].map((band, index) => (
            <div
              key={index}
              style={{
                position: "absolute",
                top: band.top,
                left: band.left,
                width: band.width,
                height: "28%",
                borderRadius: "999px",
                background: `linear-gradient(90deg, ${recordingOverlayHexToRgba(accent, 0)} 0%, ${recordingOverlayHexToRgba(accent, 0.1 + (energy * 0.06))} 20%, ${recordingOverlayHexToRgba("#ffffff", 0.09)} 50%, ${recordingOverlayHexToRgba(accent, 0.08)} 80%, ${recordingOverlayHexToRgba(accent, 0)} 100%)`,
                filter: `blur(${18 + (index * 4)}px)`,
                opacity: 0.82 - (index * 0.14),
                transform: `translate(${(index === 0 ? 1 : -1) * (energy - 0.45) * 18 * driftScale}px, ${(index - 0.5) * 6 * driftScale}px) rotate(${band.rotate + ((energy - 0.5) * 8)}deg)`,
                transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1), opacity ${transitionMs}ms ease-out`,
              }}
            />
          ))}
        </>}

      {normalizedMode === "firefly_veil" &&
        <>
          {Array.from({ length: 10 }).map((_, index) => {
            const x = ((index * 13) % 84) + 8;
            const y = ((index * 9) % 60) + 12;
            const size = 3 + ((index % 3) * 1.5);
            return (
              <div
                key={index}
                style={{
                  position: "absolute",
                  left: `${x}%`,
                  top: `${y}%`,
                  width: `${size}px`,
                  height: `${size}px`,
                  borderRadius: "999px",
                  background: `radial-gradient(circle at 35% 35%, rgba(255,255,255,0.96), ${recordingOverlayHexToRgba(accent, 0.84)} 58%, ${recordingOverlayHexToRgba(accent, 0.1)} 100%)`,
                  boxShadow: `0 0 ${8 + (size * 2)}px ${recordingOverlayHexToRgba(accent, 0.2 + (energy * 0.08))}`,
                  opacity: 0.26 + ((index % 4) * 0.08) + (energy * 0.18),
                  transform: `translate(${Math.sin(index + energy * 3.1) * 12 * driftScale}px, ${Math.cos(index + energy * 2.4) * 10 * driftScale}px) scale(${0.7 + (energy * 0.36)})`,
                  transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1), opacity ${transitionMs}ms ease-out`,
                }}
              />
            );
          })}
        </>}

      {normalizedMode === "rose_sparks" &&
        <>
          {Array.from({ length: 8 }).map((_, index) => {
            const x = ((index * 19) % 82) + 8;
            const y = ((index * 14) % 56) + 14;
            return (
              <div
                key={index}
                style={{
                  position: "absolute",
                  left: `${x}%`,
                  top: `${y}%`,
                  width: `${14 + ((index % 3) * 4)}px`,
                  height: `${6 + ((index % 2) * 2)}px`,
                  borderRadius: "999px",
                  background: `linear-gradient(90deg, ${recordingOverlayHexToRgba(accent, 0)} 0%, ${recordingOverlayHexToRgba(accent, 0.18 + (energy * 0.08))} 44%, ${recordingOverlayHexToRgba("#ffffff", 0.18)} 50%, ${recordingOverlayHexToRgba(accent, 0.12)} 56%, ${recordingOverlayHexToRgba(accent, 0)} 100%)`,
                  boxShadow: `0 0 10px ${recordingOverlayHexToRgba(accent, 0.16 + (energy * 0.08))}`,
                  opacity: 0.38 + ((index % 3) * 0.08),
                  transform: `translate(${Math.sin(index + energy * 2.7) * 10 * driftScale}px, ${Math.cos(index + energy * 2.2) * 7 * driftScale}px) rotate(${((index % 2 === 0 ? -1 : 1) * (20 + (energy * 16)))}deg)`,
                  transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1), opacity ${transitionMs}ms ease-out`,
                }}
              />
            );
          })}
        </>}
    </div>
  );
};
