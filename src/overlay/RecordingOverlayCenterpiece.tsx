import React from "react";
import {
  mixRecordingOverlayHexColors,
  normalizeRecordingOverlayCenterpieceMode,
  normalizeRecordingOverlayColor,
  recordingOverlayHexToRgba,
  shiftRecordingOverlayHue,
  type RecordingOverlayCenterpieceMode,
} from "./recordingOverlayAppearance";
import "./RecordingOverlayDepthLayers.css";

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

/** Gimbal tilt, turn period, and phase; the rest pose is used without motion. */
const GYRO_GIMBALS = [
  { tilt: 0, hue: 0, duration: 3.6, delay: 0, rest: 0.55 },
  { tilt: 62, hue: 55, duration: 5.4, delay: 1.1, rest: -0.3 },
  { tilt: -58, hue: -40, duration: 7.2, delay: 2.9, rest: 0.85 },
];

/** Parallels of the globe, seen from a little above: centre, width, height in %. */
const GLOBE_PARALLELS = [-62, -32, 0, 32, 62].map((latitude) => {
  const radians = (latitude * Math.PI) / 180;
  const width = Math.cos(radians) * 100;
  return { y: 50 - Math.sin(radians) * 47, width, height: width * 0.32 };
});
const GLOBE_MERIDIAN_COUNT = 4;
const GLOBE_TURN_S = 9;

function fraction(value: number): number {
  return value - Math.floor(value);
}

/** Ice and rock in the planet's ring, in a -50..50 box, spread by Weyl sequences. */
const PLANET_RING_DUST = Array.from({ length: 40 }, (_, index) => {
  const radius = 26 + fraction(index * 0.7548777) * 21;
  const angle = fraction(index * 0.618034) * Math.PI * 2;
  return {
    x: Math.cos(angle) * radius,
    y: Math.sin(angle) * radius,
    size: 0.7 + fraction(index * 0.4142136) * 0.9,
  };
});

function ringShades(color: string): React.CSSProperties {
  return {
    "--rod-ring-near": recordingOverlayHexToRgba(mixRecordingOverlayHexColors(color, "#ffffff", 0.45), 0.95),
    "--rod-ring-mid": recordingOverlayHexToRgba(color, 0.55),
    "--rod-ring-far": recordingOverlayHexToRgba(color, 0.18),
  } as React.CSSProperties;
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


  if (
    normalizedMode === "gyroscope" ||
    normalizedMode === "holo_globe" ||
    normalizedMode === "ringed_planet" ||
    normalizedMode === "plasma_orb"
  ) {
    const coreStyle = {
      "--rod-core-light": mixRecordingOverlayHexColors(accent, "#ffffff", 0.55),
      "--rod-core": accent,
      "--rod-core-deep": mixRecordingOverlayHexColors(accent, "#000000", 0.6),
      "--rod-core-glow": recordingOverlayHexToRgba(accent, 0.5),
      ...ringShades(accent),
    } as React.CSSProperties;
    const placement: React.CSSProperties = {
      transform: `translate(calc(-50% + ${driftX}px), calc(-50% + ${driftY}px)) scale(${0.9 + energy * 0.2})`,
      transition: `transform ${transitionMs}ms cubic-bezier(0.22, 1, 0.36, 1)`,
    };

    if (normalizedMode === "gyroscope") {
      return (
        <div aria-hidden="true" style={shellStyle}>
          <div className="rod-gyro" style={{ ...coreStyle, ...placement }}>
            <div className="rod-gyro__axle" />
            {GYRO_GIMBALS.map((gimbal, index) => (
              <div
                key={index}
                className="rod-gyro__gimbal"
                style={{ transform: `rotate(${gimbal.tilt}deg)` }}
              >
                <div
                  className="rod-gyro__ring"
                  style={{
                    ...ringShades(shiftRecordingOverlayHue(accent, gimbal.hue)),
                    "--rod-duration": `${gimbal.duration}s`,
                    "--rod-delay": `${-gimbal.delay}s`,
                    transform: `scaleX(${gimbal.rest})`,
                  } as React.CSSProperties}
                />
              </div>
            ))}
            <div className="rod-gyro__core" />
          </div>
        </div>
      );
    }

    if (normalizedMode === "ringed_planet") {
      // The ring is drawn twice: the copy behind the planet keeps only its far
      // half, the copy in front only its near half. Both copies turn in step,
      // so the planet occludes the ring correctly without any depth sorting.
      const ringHalf = (half: "far" | "near") => (
        <div className="rod-planet__tilt">
          <div className={`rod-planet__half rod-planet__half--${half}`}>
            <div className="rod-planet__band" />
            <svg className="rod-planet__dust" viewBox="-50 -50 100 100">
              {PLANET_RING_DUST.map((dust, index) => (
                <circle
                  key={index}
                  cx={dust.x.toFixed(2)}
                  cy={dust.y.toFixed(2)}
                  r={dust.size.toFixed(2)}
                  style={{ fill: "var(--rod-ring-near)" }}
                />
              ))}
            </svg>
          </div>
        </div>
      );
      const bandLight = mixRecordingOverlayHexColors(accent, "#ffffff", 0.45);
      return (
        <div aria-hidden="true" style={shellStyle}>
          <div
            className="rod-planet"
            style={{
              ...coreStyle,
              ...placement,
              "--rod-ring-light": recordingOverlayHexToRgba(bandLight, 0.75),
              "--rod-ring-dark": recordingOverlayHexToRgba(accent, 0.28),
              "--rod-band-light": mixRecordingOverlayHexColors(accent, "#ffffff", 0.3),
              "--rod-band-dark": mixRecordingOverlayHexColors(accent, "#000000", 0.25),
            } as React.CSSProperties}
          >
            {ringHalf("far")}
            <div className="rod-planet__body" />
            {ringHalf("near")}
          </div>
        </div>
      );
    }

    if (normalizedMode === "plasma_orb") {
      return (
        <div aria-hidden="true" style={shellStyle}>
          <div
            className="rod-plasma"
            style={{
              ...coreStyle,
              ...placement,
              "--rod-plasma-a": recordingOverlayHexToRgba(accent, 0.95),
              "--rod-plasma-b": recordingOverlayHexToRgba(shiftRecordingOverlayHue(accent, 70), 0.9),
              "--rod-plasma-c": recordingOverlayHexToRgba(shiftRecordingOverlayHue(accent, -60), 0.9),
            } as React.CSSProperties}
          >
            <div
              className="rod-plasma__glow"
              style={{
                opacity: 0.35 + energy * 0.65,
                transition: `opacity ${transitionMs}ms ease-out`,
              }}
            />
            <div className="rod-plasma__sphere">
              <div className="rod-plasma__swirl rod-plasma__swirl--a" />
              <div className="rod-plasma__swirl rod-plasma__swirl--b" />
              <div className="rod-plasma__shade" />
            </div>
          </div>
        </div>
      );
    }

    return (
      <div aria-hidden="true" style={shellStyle}>
        <div
          className="rod-globe"
          style={{
            ...coreStyle,
            ...placement,
            "--rod-globe-fill": recordingOverlayHexToRgba(accent, 0.07),
            "--rod-globe-rim": recordingOverlayHexToRgba(accent, 0.34),
            "--rod-scan": recordingOverlayHexToRgba(mixRecordingOverlayHexColors(accent, "#ffffff", 0.5), 0.5),
          } as React.CSSProperties}
        >
          <div className="rod-globe__body">
            {GLOBE_PARALLELS.map((parallel) => (
              <div
                key={parallel.y}
                className="rod-globe__parallel"
                style={{
                  left: `${(100 - parallel.width) / 2}%`,
                  width: `${parallel.width}%`,
                  top: `${parallel.y - parallel.height / 2}%`,
                  height: `${parallel.height}%`,
                }}
              />
            ))}
            {Array.from({ length: GLOBE_MERIDIAN_COUNT }, (_, index) => {
              // Each ellipse is a full great circle, so four of them spaced an
              // eighth of a turn apart draw eight meridians.
              const phase = index / (GLOBE_MERIDIAN_COUNT * 2);
              return (
                <div
                  key={index}
                  className="rod-globe__meridian"
                  style={{
                    "--rod-duration": `${GLOBE_TURN_S}s`,
                    "--rod-delay": `${-phase * GLOBE_TURN_S}s`,
                    transform: `scaleX(${Math.cos(phase * Math.PI * 2)})`,
                  } as React.CSSProperties}
                />
              );
            })}
            <div className="rod-globe__scan" />
          </div>
          <div className="rod-globe__orbit">
            <div className="rod-globe__orbit-x">
              <div className="rod-globe__satellite" />
            </div>
          </div>
        </div>
      </div>
    );
  }

  return null;
};
