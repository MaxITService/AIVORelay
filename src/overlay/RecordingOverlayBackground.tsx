import React from "react";
import {
  mixRecordingOverlayHexColors,
  normalizeRecordingOverlayBackgroundMode,
  normalizeRecordingOverlayColor,
  recordingOverlayHexToRgba,
  shiftRecordingOverlayHue,
  type RecordingOverlayBackgroundMode,
} from "./recordingOverlayAppearance";
import "./RecordingOverlayDepthLayers.css";

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

/** Ground positions of the receding grid lines, where they meet the bottom edge. */
const HORIZON_RAY_ENDS = Array.from({ length: 25 }, (_, index) => 50 + (index - 12) * 18);
const HORIZON_LINE_COUNT = 6;
const HORIZON_CYCLE_S = 2.6;

/**
 * Warp stars along tracks from the center. Angles follow the golden angle and
 * are flattened toward the horizontal, where a wide overlay has the room.
 */
const WARP_STARS = Array.from({ length: 24 }, (_, index) => {
  const spread = (index * 137.508 * Math.PI) / 180;
  const duration = 1.1 + (((index * 7) % 10) / 10) * 1.1;
  return {
    angle: (Math.atan2(Math.sin(spread) * 0.4, Math.cos(spread)) * 180) / Math.PI,
    duration,
    delay: (index * 0.37) % duration,
    thickness: index % 5 === 0 ? 1.5 : 1,
    tint: index % 4 === 0 ? 1 : index % 7 === 0 ? 2 : 0,
    rest: 150 + ((index * 53) % 600),
  };
});

/** Tunnel rails run from the center to points around the frame edge. */
const TUNNEL_RAIL_ENDS: Array<[number, number]> = [
  [0, 0], [25, 0], [50, 0], [75, 0], [100, 0], [100, 50],
  [100, 100], [75, 100], [50, 100], [25, 100], [0, 100], [0, 50],
];
const TUNNEL_RING_COUNT = 6;
const TUNNEL_CYCLE_S = 3.2;

function fraction(value: number): number {
  return value - Math.floor(value);
}

/**
 * Stars of a two-armed logarithmic spiral in a -50..50 box, plus loose dust.
 * Weyl sequences (multiples of irrational numbers) jitter them evenly without
 * a random generator, so every render draws the same sky. The whole disc is a
 * single layer that one CSS rotation turns.
 */
const GALAXY_ARM_STARS = 34;
const GALAXY_STARS = [
  ...Array.from({ length: GALAXY_ARM_STARS * 2 }, (_, index) => {
    const arm = index % 2;
    const step = Math.floor(index / 2);
    const along = (step + 0.5) / GALAXY_ARM_STARS;
    const radius = 5 + along * 43 + (fraction(step * 0.7548777 + arm * 0.31) - 0.5) * 5;
    const angle =
      arm * Math.PI + 2.2 * Math.log(radius / 5) + (fraction(step * 0.618034 + arm * 0.5) - 0.5) * 0.5;
    return {
      x: Math.cos(angle) * radius,
      y: Math.sin(angle) * radius,
      size: 0.8 + fraction(step * 0.4142136) * 1.1 - along * 0.3,
      tint: along < 0.25 ? 0 : step % 3 === 0 ? 2 : 1,
    };
  }),
  ...Array.from({ length: 26 }, (_, index) => {
    const radius = 8 + fraction(index * 0.7548777) * 40;
    const angle = fraction(index * 0.5698403) * Math.PI * 2;
    return {
      x: Math.cos(angle) * radius,
      y: Math.sin(angle) * radius,
      size: 0.6 + fraction(index * 0.4142136) * 0.6,
      tint: 3,
    };
  }),
];

/**
 * Rows of the dot swell from the horizon forward. Spacing follows perspective,
 * so the rows crowd toward the horizon; each row bobs on its own delay, and
 * the delays roll a swell toward the viewer.
 */
const SWELL_ROWS = Array.from({ length: 7 }, (_, index) => {
  const near = (index + 1) / 7;
  return {
    top: 42 + Math.pow(near, 1.7) * 54,
    spacing: 5 + near * 11,
    size: 0.9 + near * 1.7,
    swing: 0.8 + near * 2.6,
    opacity: 0.25 + near * 0.65,
    offset: index % 2 === 0 ? 0 : 0.5,
    near,
  };
});
const SWELL_CYCLE_S = 2.8;

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
  const secondary = shiftRecordingOverlayHue(accent, 48);
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
      {normalizedMode === "horizon_grid" && (
        <div
          className="rod-layer"
          style={{
            "--rod-sky-mid": recordingOverlayHexToRgba(secondary, 0.08),
            "--rod-sky-low": recordingOverlayHexToRgba(accent, 0.3),
            "--rod-sun-top": mixRecordingOverlayHexColors("#ffe27a", accent, 0.2),
            "--rod-sun-bottom": accent,
            "--rod-sun-glow": recordingOverlayHexToRgba(accent, 0.5),
            "--rod-floor-top": recordingOverlayHexToRgba(accent, 0.16),
            "--rod-grid": recordingOverlayHexToRgba(mixRecordingOverlayHexColors(accent, "#ffffff", 0.25), 0.72),
            "--rod-horizon": recordingOverlayHexToRgba(mixRecordingOverlayHexColors(accent, "#ffffff", 0.3), 0.9),
            "--rod-horizon-core": "rgba(255,255,255,0.95)",
            opacity: 0.78 + energy * 0.22,
            transition: `opacity ${transitionMs}ms ease-out`,
          } as React.CSSProperties}
        >
          <div className="rod-horizon__sky">
            <div
              className="rod-horizon__sun-glow"
              style={{
                opacity: 0.5 + energy * 0.5,
                transition: `opacity ${transitionMs}ms ease-out`,
              }}
            />
            <div className="rod-horizon__sun" />
          </div>
          <div className="rod-horizon__floor">
            <svg className="rod-horizon__rays" viewBox="0 0 100 100" preserveAspectRatio="none">
              {HORIZON_RAY_ENDS.map((end) => (
                <line
                  key={end}
                  x1={50}
                  y1={0}
                  x2={end}
                  y2={100}
                  strokeWidth={1}
                  vectorEffect="non-scaling-stroke"
                  style={{ stroke: "var(--rod-grid)" }}
                />
              ))}
            </svg>
            {Array.from({ length: HORIZON_LINE_COUNT }, (_, index) => {
              const phase = index / HORIZON_LINE_COUNT;
              return (
                <div
                  key={index}
                  className="rod-horizon__line"
                  style={{
                    "--rod-delay": `${-phase * HORIZON_CYCLE_S}s`,
                    "--rod-rest": `${Math.pow(phase, 2.2) * 100}%`,
                  } as React.CSSProperties}
                />
              );
            })}
          </div>
          <div className="rod-horizon__haze" />
        </div>
      )}

      {normalizedMode === "starfield_warp" && (
        <div
          className="rod-layer"
          style={{
            "--rod-core": recordingOverlayHexToRgba(mixRecordingOverlayHexColors(accent, "#ffffff", 0.55), 0.5),
            "--rod-core-edge": recordingOverlayHexToRgba(accent, 0.16),
            opacity: 0.74 + energy * 0.26,
            transition: `opacity ${transitionMs}ms ease-out`,
          } as React.CSSProperties}
        >
          <div
            className="rod-warp__core"
            style={{
              transform: `translate(-50%, -50%) scale(${0.75 + energy * 0.55})`,
              transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1)`,
            }}
          />
          {WARP_STARS.map((star, index) => (
            <div
              key={index}
              className="rod-warp__track"
              style={{ transform: `rotate(${star.angle}deg)` }}
            >
              <span
                className="rod-warp__star"
                style={{
                  "--rod-star":
                    star.tint === 1
                      ? recordingOverlayHexToRgba(mixRecordingOverlayHexColors(accent, "#ffffff", 0.35), 0.95)
                      : star.tint === 2
                        ? recordingOverlayHexToRgba(mixRecordingOverlayHexColors(secondary, "#ffffff", 0.35), 0.95)
                        : "rgba(255,255,255,0.92)",
                  "--rod-duration": `${star.duration}s`,
                  "--rod-delay": `${-star.delay}s`,
                  "--rod-thickness": `${star.thickness}px`,
                  "--rod-rest": `${star.rest}%`,
                } as React.CSSProperties}
              />
            </div>
          ))}
        </div>
      )}

      {normalizedMode === "tunnel_rings" && (
        <div
          className="rod-layer"
          style={{
            "--rod-end": recordingOverlayHexToRgba(accent, 0.3),
            "--rod-end-core": recordingOverlayHexToRgba(mixRecordingOverlayHexColors(secondary, "#ffffff", 0.6), 0.8),
            opacity: 0.8 + energy * 0.2,
            transition: `opacity ${transitionMs}ms ease-out`,
          } as React.CSSProperties}
        >
          <svg className="rod-tunnel__rails" viewBox="0 0 100 100" preserveAspectRatio="none">
            {TUNNEL_RAIL_ENDS.map(([x, y]) => (
              <line
                key={`${x}-${y}`}
                x1={50}
                y1={50}
                x2={x}
                y2={y}
                strokeWidth={1}
                vectorEffect="non-scaling-stroke"
                style={{ stroke: recordingOverlayHexToRgba(accent, 0.2) }}
              />
            ))}
          </svg>
          <div
            className="rod-tunnel__end"
            style={{
              transform: `translate(-50%, -50%) scale(${0.85 + energy * 0.5})`,
              transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1)`,
            }}
          />
          {Array.from({ length: TUNNEL_RING_COUNT }, (_, index) => {
            const phase = index / TUNNEL_RING_COUNT;
            const ringColor = index % 2 === 0 ? accent : secondary;
            return (
              <div
                key={index}
                className="rod-tunnel__ring"
                style={{
                  "--rod-ring": recordingOverlayHexToRgba(ringColor, 0.85),
                  "--rod-ring-glow": recordingOverlayHexToRgba(ringColor, 0.3),
                  "--rod-delay": `${-phase * TUNNEL_CYCLE_S}s`,
                  "--rod-rest": `${0.12 + phase * 0.95}`,
                } as React.CSSProperties}
              />
            );
          })}
        </div>
      )}

      {normalizedMode === "galaxy_spiral" && (
        <div
          className="rod-layer"
          style={{
            "--rod-galaxy-glow": recordingOverlayHexToRgba(mixRecordingOverlayHexColors(accent, "#ffffff", 0.4), 0.32),
            "--rod-galaxy-haze": recordingOverlayHexToRgba(secondary, 0.1),
            "--rod-core": recordingOverlayHexToRgba(mixRecordingOverlayHexColors(accent, "#ffffff", 0.7), 0.85),
            "--rod-core-edge": recordingOverlayHexToRgba(accent, 0.25),
            opacity: 0.76 + energy * 0.24,
            transition: `opacity ${transitionMs}ms ease-out`,
          } as React.CSSProperties}
        >
          <div className="rod-galaxy__tilt">
            <svg className="rod-galaxy__disc" viewBox="-50 -50 100 100">
              {GALAXY_STARS.map((star, index) => (
                <circle
                  key={index}
                  cx={star.x.toFixed(2)}
                  cy={star.y.toFixed(2)}
                  r={star.size.toFixed(2)}
                  fill={
                    star.tint === 0
                      ? "rgba(255,255,255,0.95)"
                      : star.tint === 1
                        ? recordingOverlayHexToRgba(mixRecordingOverlayHexColors(accent, "#ffffff", 0.3), 0.9)
                        : star.tint === 2
                          ? recordingOverlayHexToRgba(mixRecordingOverlayHexColors(secondary, "#ffffff", 0.3), 0.9)
                          : recordingOverlayHexToRgba(mixRecordingOverlayHexColors(accent, "#ffffff", 0.6), 0.45)
                  }
                />
              ))}
            </svg>
          </div>
          <div
            className="rod-galaxy__core"
            style={{
              transform: `translate(-50%, -50%) scale(${0.8 + energy * 0.5})`,
              transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1)`,
            }}
          />
        </div>
      )}

      {normalizedMode === "dot_swell" && (
        <div
          className="rod-layer"
          style={{
            "--rod-horizon": recordingOverlayHexToRgba(mixRecordingOverlayHexColors(accent, "#ffffff", 0.3), 0.75),
            "--rod-sky-low": recordingOverlayHexToRgba(accent, 0.22),
            opacity: 0.78 + energy * 0.22,
            transition: `opacity ${transitionMs}ms ease-out`,
          } as React.CSSProperties}
        >
          <div className="rod-swell__glow" />
          <div className="rod-swell__horizon" />
          {SWELL_ROWS.map((row, index) => {
            const color = mixRecordingOverlayHexColors(secondary, accent, row.near);
            return (
              <div
                key={index}
                className="rod-swell__row"
                style={{
                  top: `${row.top}%`,
                  height: `${row.swing * 2}px`,
                  marginTop: `${-row.swing}px`,
                  opacity: row.opacity,
                  backgroundImage: `radial-gradient(circle, ${recordingOverlayHexToRgba(mixRecordingOverlayHexColors(color, "#ffffff", 0.25), 0.95)} 0 ${(row.size / 2).toFixed(2)}px, rgba(0,0,0,0) ${(row.size / 2 + 0.6).toFixed(2)}px)`,
                  backgroundSize: `${row.spacing.toFixed(2)}px 100%`,
                  backgroundPosition: `calc(50% + ${(row.offset * row.spacing).toFixed(2)}px) 50%`,
                  "--rod-duration": `${SWELL_CYCLE_S / 2}s`,
                  "--rod-delay": `${-(1 - row.near) * SWELL_CYCLE_S}s`,
                } as React.CSSProperties}
              />
            );
          })}
        </div>
      )}
    </div>
  );
};
