import React from "react";
import {
  mixRecordingOverlayHexColors,
  normalizeRecordingOverlayBarStyle,
  normalizeRecordingOverlayColor,
  recordingOverlayHexToRgba,
  shiftRecordingOverlayHue,
  type RecordingOverlayBarStyle,
} from "./recordingOverlayAppearance";
import "./RecordingOverlayDepthLayers.css";

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

/** How far a pillar's side face recedes: half its width, at least 2px. */
function pillarDepthPx(width: number): number {
  return Math.max(2, Math.round(width / 2));
}

// Keep the lane widths in sync with `recording_overlay_default_width` in overlay.rs.
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
    case "pillars":
      return effectiveWidth + pillarDepthPx(effectiveWidth);
    case "orbs":
    case "cubes":
      return Math.max(effectiveWidth + 4, 8);
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
    case "mirror":
    case "dot_matrix":
    case "spectrum":
      return true;
    default:
      return false;
  }
}

/**
 * Eases displayed levels toward the latest input every frame, so a continuous
 * line does not jump between audio level updates.
 */
function useSmoothedLevels(
  levels: number[],
  enabled: boolean,
  timeConstantMs: number,
): number[] {
  const [smoothed, setSmoothed] = React.useState(levels);
  const currentRef = React.useRef(levels);

  React.useEffect(() => {
    if (!enabled) {
      currentRef.current = levels;
      return;
    }
    let frameId = 0;
    let lastTime = performance.now();
    const step = (now: number) => {
      const elapsed = Math.min(64, now - lastTime);
      lastTime = now;
      const blend = 1 - Math.exp(-elapsed / timeConstantMs);
      let moving = false;
      const next = levels.map((target, index) => {
        const from = currentRef.current[index] ?? 0;
        const delta = clampUnit(target) - from;
        if (Math.abs(delta) > 0.002) {
          moving = true;
        }
        return from + delta * blend;
      });
      currentRef.current = next;
      setSmoothed(next);
      frameId = moving ? window.requestAnimationFrame(step) : 0;
    };
    frameId = window.requestAnimationFrame(step);
    return () => window.cancelAnimationFrame(frameId);
  }, [enabled, levels, timeConstantMs]);

  return enabled ? smoothed : levels;
}

/** Catmull-Rom spline through the points, written as cubic Bezier segments. */
function smoothPath(points: Array<[number, number]>): string {
  const format = (value: number) => value.toFixed(2);
  let path = `M ${format(points[0][0])} ${format(points[0][1])}`;
  for (let index = 0; index < points.length - 1; index += 1) {
    const [x0, y0] = points[index - 1] ?? points[index];
    const [x1, y1] = points[index];
    const [x2, y2] = points[index + 1];
    const [x3, y3] = points[index + 2] ?? points[index + 1];
    path += ` C ${format(x1 + (x2 - x0) / 6)} ${format(y1 + (y2 - y0) / 6)}, ${format(x2 - (x3 - x1) / 6)} ${format(y2 - (y3 - y1) / 6)}, ${format(x2)} ${format(y2)}`;
  }
  return path;
}

interface RecordingOverlayWaveLineProps {
  levels: number[];
  widthPx: number;
  heightPx: number;
  strokeWidthPx: number;
  accent: string;
  animated: boolean;
  softness: number;
}

const RecordingOverlayWaveLine: React.FC<RecordingOverlayWaveLineProps> = ({
  levels,
  widthPx,
  heightPx,
  strokeWidthPx,
  accent,
  animated,
  softness,
}) => {
  const gradientId = `recording-overlay-wave-${React.useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const displayed = useSmoothedLevels(levels, animated, 45 + softness * 110);
  const secondary = shiftRecordingOverlayHue(accent, 48);
  const mid = heightPx / 2;
  const amplitude = Math.max(2, mid - strokeWidthPx - 1);
  const count = displayed.length;
  // Neighbouring points swing to opposite sides, and an envelope keeps the
  // ends calm, so the spline reads as one wave instead of a row of peaks.
  const primaryPoints: Array<[number, number]> = [[0, mid]];
  displayed.forEach((level, index) => {
    const position = (index + 1) / (count + 1);
    const envelope = 0.35 + 0.65 * Math.sin(position * Math.PI);
    const lift = Math.max(0.06, Math.pow(clampUnit(level), 0.75)) * amplitude * envelope;
    primaryPoints.push([position * widthPx, mid + (index % 2 === 0 ? -lift : lift)]);
  });
  primaryPoints.push([widthPx, mid]);
  const echoPoints = primaryPoints.map(
    ([x, y]): [number, number] => [x, mid - (y - mid) * 0.55],
  );
  const primaryPath = smoothPath(primaryPoints);
  const echoPath = smoothPath(echoPoints);
  const stroke = `url(#${gradientId})`;

  return (
    <svg
      width={widthPx}
      height={heightPx}
      viewBox={`0 0 ${widthPx} ${heightPx}`}
      style={{ display: "block", overflow: "visible" }}
      aria-hidden="true"
    >
      <defs>
        <linearGradient id={gradientId} gradientUnits="userSpaceOnUse" x1={0} y1={0} x2={widthPx} y2={0}>
          <stop offset="0%" stopColor={accent} stopOpacity={0.2} />
          <stop offset="22%" stopColor={accent} />
          <stop offset="78%" stopColor={secondary} />
          <stop offset="100%" stopColor={secondary} stopOpacity={0.2} />
        </linearGradient>
      </defs>
      <path d={echoPath} fill="none" stroke={stroke} strokeWidth={Math.max(1, strokeWidthPx * 0.7)} strokeLinecap="round" opacity={0.42} />
      <path d={primaryPath} fill="none" stroke={stroke} strokeWidth={strokeWidthPx * 2.4} strokeLinecap="round" opacity={0.2} style={{ filter: "blur(2px)" }} />
      <path d={primaryPath} fill="none" stroke={stroke} strokeWidth={strokeWidthPx} strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
};

interface RecordingOverlayLiquidProps {
  levels: number[];
  widthPx: number;
  heightPx: number;
  blobWidthPx: number;
  accent: string;
  animated: boolean;
  softness: number;
}

/**
 * Drops on a thin stream that swell with each level. A blur plus an alpha
 * threshold merges neighbours into one liquid shape.
 */
const RecordingOverlayLiquid: React.FC<RecordingOverlayLiquidProps> = ({
  levels,
  widthPx,
  heightPx,
  blobWidthPx,
  accent,
  animated,
  softness,
}) => {
  const id = `recording-overlay-liquid-${React.useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const displayed = useSmoothedLevels(levels, animated, 60 + softness * 120);
  const secondary = shiftRecordingOverlayHue(accent, 32);
  const mid = heightPx / 2;
  const radius = Math.max(2, blobWidthPx / 2);
  const streamHeight = Math.max(3, radius * 1.3);
  const blur = Math.max(1.4, radius * 0.6);
  const lane = blobWidthPx + BAR_GAP_PX;
  const shapes = (
    <>
      <rect
        x={radius * 0.5}
        y={mid - streamHeight / 2}
        width={Math.max(0, widthPx - radius)}
        height={streamHeight}
        rx={streamHeight / 2}
      />
      {displayed.map((rawLevel, index) => {
        const level = Math.pow(clampUnit(rawLevel), 0.8);
        return (
          <ellipse
            key={index}
            cx={index * lane + blobWidthPx / 2}
            cy={mid}
            rx={radius * (1 + level * 0.35)}
            ry={radius + level * Math.max(0, mid - radius - 1)}
          />
        );
      })}
    </>
  );

  return (
    <svg
      width={widthPx}
      height={heightPx}
      viewBox={`0 0 ${widthPx} ${heightPx}`}
      style={{ display: "block", overflow: "visible" }}
      aria-hidden="true"
    >
      <defs>
        <filter id={`${id}-goo`} x="-20%" y="-40%" width="140%" height="180%">
          <feGaussianBlur in="SourceGraphic" stdDeviation={blur} />
          <feColorMatrix mode="matrix" values="1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 18 -7" />
        </filter>
        <linearGradient id={`${id}-fill`} gradientUnits="userSpaceOnUse" x1={0} y1={0} x2={widthPx} y2={0}>
          <stop offset="0%" stopColor={accent} />
          <stop offset="100%" stopColor={secondary} />
        </linearGradient>
        <linearGradient id={`${id}-shine`} gradientUnits="userSpaceOnUse" x1={0} y1={0} x2={0} y2={heightPx}>
          <stop offset="0%" stopColor="#ffffff" stopOpacity={0.7} />
          <stop offset="42%" stopColor="#ffffff" stopOpacity={0} />
        </linearGradient>
        {/* The merged shape as a mask, so the gloss keeps its soft gradient. */}
        <mask id={`${id}-shape`}>
          <g filter={`url(#${id}-goo)`} fill="#ffffff">
            {shapes}
          </g>
        </mask>
      </defs>
      <g filter={`url(#${id}-goo)`} fill={`url(#${id}-fill)`}>
        {shapes}
      </g>
      <rect
        x={0}
        y={0}
        width={widthPx}
        height={heightPx}
        fill={`url(#${id}-shine)`}
        mask={`url(#${id}-shape)`}
        opacity={0.55}
      />
    </svg>
  );
};

/** Camera tilt for the cubes: how much of the top face shows. */
const CUBE_ELEVATION_SIN = 0.42;
const CUBE_ELEVATION_COS = Math.sqrt(1 - CUBE_ELEVATION_SIN * CUBE_ELEVATION_SIN);
/** Key light from the upper front left, as a ground-plane direction. */
const CUBE_LIGHT_X = -0.62;
const CUBE_LIGHT_Z = -0.78;

function initialCubeAngle(index: number): number {
  return 0.35 + index * 0.61;
}

function shadeRecordingOverlayColor(color: string, brightness: number): string {
  return brightness <= 1
    ? mixRecordingOverlayHexColors(color, "#000000", 1 - Math.max(0, brightness))
    : mixRecordingOverlayHexColors(color, "#ffffff", Math.min(1, brightness - 1));
}

interface RecordingOverlayCubesProps {
  levels: number[];
  laneWidthPx: number;
  heightPx: number;
  accent: string;
  animated: boolean;
  softness: number;
}

/**
 * Flat-shaded cubes that turn on their axis and grow into towers with the
 * voice. Each frame projects the corners orthographically and lights every
 * face from its normal, so the 3D look costs a handful of SVG polygons.
 */
const RecordingOverlayCubes: React.FC<RecordingOverlayCubesProps> = ({
  levels,
  laneWidthPx,
  heightPx,
  accent,
  animated,
  softness,
}) => {
  const id = `recording-overlay-cubes-${React.useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const [reducedMotion] = React.useState(
    () => window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false,
  );
  const spinning = animated && !reducedMotion;
  const levelsRef = React.useRef(levels);
  React.useEffect(() => {
    levelsRef.current = levels;
  }, [levels]);
  const motionRef = React.useRef({
    angles: levels.map((_, index) => initialCubeAngle(index)),
    heights: levels.map(clampUnit),
  });
  const [motion, setMotion] = React.useState(motionRef.current);

  React.useEffect(() => {
    if (!spinning) {
      return;
    }
    let frameId = 0;
    let lastTime = performance.now();
    const timeConstantMs = 70 + softness * 130;
    const step = (now: number) => {
      const elapsedMs = Math.min(64, now - lastTime);
      lastTime = now;
      const blend = 1 - Math.exp(-elapsedMs / timeConstantMs);
      const previous = motionRef.current;
      const heights = levelsRef.current.map((target, index) => {
        const from = previous.heights[index] ?? 0;
        return from + (clampUnit(target) - from) * blend;
      });
      // A slow idle turn that speeds up with the voice.
      const angles = heights.map((height, index) => {
        const from = previous.angles[index] ?? initialCubeAngle(index);
        return (from + (elapsedMs / 1000) * (0.3 + height * 2.8)) % (Math.PI * 2);
      });
      motionRef.current = { angles, heights };
      setMotion(motionRef.current);
      frameId = window.requestAnimationFrame(step);
    };
    frameId = window.requestAnimationFrame(step);
    return () => window.cancelAnimationFrame(frameId);
  }, [spinning, softness]);

  const angles = spinning ? motion.angles : levels.map((_, index) => initialCubeAngle(index));
  const heights = spinning ? motion.heights : levels.map(clampUnit);
  const count = levels.length;
  const widthPx = count * laneWidthPx + Math.max(0, count - 1) * BAR_GAP_PX;
  const radius = laneWidthPx / 2;
  const k = CUBE_ELEVATION_SIN;
  const c = CUBE_ELEVATION_COS;
  const groundY = heightPx - 1 - radius * k;
  const edge = radius * Math.SQRT2;
  const minHeight = edge * 0.82;
  const maxHeight = Math.max(minHeight, (heightPx - 1 - 2 * radius * k) / c);
  const format = (value: number) => value.toFixed(2);

  return (
    <svg
      width={widthPx}
      height={heightPx}
      viewBox={`0 0 ${widthPx} ${heightPx}`}
      style={{ display: "block", overflow: "visible" }}
      aria-hidden="true"
    >
      <defs>
        <radialGradient id={`${id}-shadow`}>
          <stop offset="0%" stopColor="#000000" stopOpacity={0.5} />
          <stop offset="100%" stopColor="#000000" stopOpacity={0} />
        </radialGradient>
      </defs>
      {heights.map((rawHeight, index) => {
        const position = count > 1 ? index / (count - 1) : 0.5;
        const color = shiftRecordingOverlayHue(accent, (position - 0.5) * 48);
        const centerX = index * (laneWidthPx + BAR_GAP_PX) + laneWidthPx / 2;
        const towerHeight = minHeight + Math.pow(rawHeight, 0.8) * (maxHeight - minHeight);
        const turn = ((angles[index] ?? 0) % (Math.PI / 2) + Math.PI / 2) % (Math.PI / 2);
        const corners = [0, 1, 2, 3].map((corner) => {
          const angle = turn + Math.PI / 4 + corner * (Math.PI / 2);
          return { angle, x: radius * Math.cos(angle), z: radius * Math.sin(angle) };
        });
        const project = (x: number, z: number, y: number) =>
          `${format(centerX + x)},${format(groundY - y * c - z * k)}`;
        let front = 0;
        corners.forEach((corner, cornerIndex) => {
          if (corner.z < corners[front].z) front = cornerIndex;
        });
        const frontCorner = corners[front];
        const faces = [corners[(front + 3) % 4], corners[(front + 1) % 4]].map((neighbor, side) => {
          // The face normal points between the front corner and its neighbour.
          const normalAngle = frontCorner.angle + (side === 0 ? -1 : 1) * (Math.PI / 4);
          const facing =
            Math.cos(normalAngle) * CUBE_LIGHT_X + Math.sin(normalAngle) * CUBE_LIGHT_Z;
          return {
            points: [
              project(frontCorner.x, frontCorner.z, 0),
              project(neighbor.x, neighbor.z, 0),
              project(neighbor.x, neighbor.z, towerHeight),
              project(frontCorner.x, frontCorner.z, towerHeight),
            ].join(" "),
            fill: shadeRecordingOverlayColor(color, 0.38 + Math.max(0, facing) * 0.74),
          };
        });
        const top = corners.map((corner) => project(corner.x, corner.z, towerHeight)).join(" ");
        return (
          <g key={index}>
            {/* The key light sits front left, so the shadow falls back right. */}
            <ellipse
              cx={centerX + radius * 0.35}
              cy={groundY - radius * k * 0.2}
              rx={radius * 1.15}
              ry={Math.max(1.2, radius * k * 1.15)}
              fill={`url(#${id}-shadow)`}
            />
            {faces.map((face, faceIndex) => (
              <polygon key={faceIndex} points={face.points} fill={face.fill} />
            ))}
            <polygon
              points={top}
              fill={shadeRecordingOverlayColor(color, 1.32 + rawHeight * 0.18)}
              stroke="rgba(255,255,255,0.55)"
              strokeWidth={0.6}
              strokeLinejoin="round"
            />
            {/* A thin rim light on the leading vertical edge. */}
            <line
              x1={centerX + frontCorner.x}
              y1={groundY - frontCorner.z * k}
              x2={centerX + frontCorner.x}
              y2={groundY - towerHeight * c - frontCorner.z * k}
              stroke="rgba(255,255,255,0.32)"
              strokeWidth={0.6}
            />
          </g>
        );
      })}
    </svg>
  );
};

function usePrefersReducedMotion(): boolean {
  const [reducedMotion] = React.useState(
    () => window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false,
  );
  return reducedMotion;
}

/** History rows behind the live front ridge; one more fades out at the back. */
const RIDGE_HISTORY_ROWS = 6;
/**
 * A row recedes one step per sample, and its transition lasts exactly one
 * sample, so the terrain glides back at a steady speed.
 */
const RIDGE_SAMPLE_MS = 120;
/** Perspective divisor added per row: a row at depth d is drawn at 1 / (1 + d * step). */
const RIDGE_DEPTH_STEP = 0.34;

interface RidgeShape {
  line: string;
  area: string;
}

/** One ridge in depth-0 coordinates; depth is applied later as a transform. */
function ridgeShape(
  levels: number[],
  widthPx: number,
  baseY: number,
  amplitude: number,
): RidgeShape {
  const count = levels.length;
  const points: Array<[number, number]> = [[0, baseY]];
  levels.forEach((level, index) => {
    const position = (index + 1) / (count + 1);
    const envelope = 0.3 + 0.7 * Math.sin(position * Math.PI);
    const lift = Math.max(0.05, Math.pow(clampUnit(level), 0.8)) * amplitude * envelope;
    points.push([position * widthPx, baseY - lift]);
  });
  points.push([widthPx, baseY]);
  const line = smoothPath(points);
  return { line, area: `${line} L ${widthPx} ${baseY + 1} L 0 ${baseY + 1} Z` };
}

interface RecordingOverlayRidgelineProps {
  levels: number[];
  widthPx: number;
  heightPx: number;
  strokeWidthPx: number;
  accent: string;
  animated: boolean;
  softness: number;
}

/**
 * The last second of speech as a terrain of ridges receding to a vanishing
 * point. Each sampled ridge is built once in flat coordinates; after that only
 * the transform and opacity of its group change, and a CSS transition carries
 * it one row back per sample. The live front ridge is the only path redrawn as
 * the voice changes. Dark fills hide the ridges behind, painter's style.
 */
const RecordingOverlayRidgeline: React.FC<RecordingOverlayRidgelineProps> = ({
  levels,
  widthPx,
  heightPx,
  strokeWidthPx,
  accent,
  animated,
  softness,
}) => {
  const reducedMotion = usePrefersReducedMotion();
  const moving = animated && !reducedMotion;
  const displayed = useSmoothedLevels(levels, moving, 40 + softness * 90);
  const displayedRef = React.useRef(displayed);
  React.useEffect(() => {
    displayedRef.current = displayed;
  }, [displayed]);
  const nextIdRef = React.useRef(0);
  const [history, setHistory] = React.useState<Array<{ id: number; shape: RidgeShape }>>([]);

  const baseY = heightPx - 1;
  const amplitude = (heightPx - 2) * 0.78;

  React.useEffect(() => {
    setHistory([]);
    if (!moving) {
      return;
    }
    const timer = window.setInterval(() => {
      const shape = ridgeShape(displayedRef.current, widthPx, baseY, amplitude);
      const snapshot = { id: nextIdRef.current, shape };
      nextIdRef.current += 1;
      setHistory((previous) => [snapshot, ...previous].slice(0, RIDGE_HISTORY_ROWS + 1));
    }, RIDGE_SAMPLE_MS);
    return () => window.clearInterval(timer);
  }, [moving, widthPx, baseY, amplitude]);

  // Without motion, a still landscape: each row repeats the voice shifted by
  // one lane, so the ridges do not line up into a single wall.
  const rows = moving
    ? history.map((entry, index) => ({ key: `ridge-${entry.id}`, depth: index + 1, shape: entry.shape }))
    : Array.from({ length: RIDGE_HISTORY_ROWS }, (_, index) => ({
        key: `still-${index}`,
        depth: index + 1,
        shape: ridgeShape(
          displayed.map((_, lane) => displayed[(lane + index + 1) % displayed.length] * 0.85),
          widthPx,
          baseY,
          amplitude,
        ),
      }));
  const front = ridgeShape(displayed, widthPx, baseY, amplitude);
  const vanishX = widthPx / 2;
  const vanishY = heightPx * 0.06;
  const secondary = shiftRecordingOverlayHue(accent, 40);
  const fill = recordingOverlayHexToRgba(mixRecordingOverlayHexColors(accent, "#000000", 0.82), 0.92);
  const rowTransition = moving
    ? `transform ${RIDGE_SAMPLE_MS}ms linear, opacity ${RIDGE_SAMPLE_MS}ms linear`
    : "none";

  return (
    <svg
      width={widthPx}
      height={heightPx}
      viewBox={`0 0 ${widthPx} ${heightPx}`}
      style={{ display: "block", overflow: "visible" }}
      aria-hidden="true"
    >
      {rows
        .slice()
        .reverse()
        .map((row) => {
          const scale = 1 / (1 + row.depth * RIDGE_DEPTH_STEP);
          const fade = Math.max(0, 1 - row.depth / (RIDGE_HISTORY_ROWS + 1));
          return (
            <g
              key={row.key}
              className={moving ? "rod-ridge__row" : undefined}
              style={{
                transform: `translate(${(vanishX * (1 - scale)).toFixed(2)}px, ${(vanishY * (1 - scale)).toFixed(2)}px) scale(${scale.toFixed(4)})`,
                opacity: 0.25 + fade * 0.7,
                transition: rowTransition,
              }}
            >
              <path d={row.shape.area} fill={fill} />
              <path
                d={row.shape.line}
                fill="none"
                stroke={mixRecordingOverlayHexColors(accent, secondary, row.depth / RIDGE_HISTORY_ROWS)}
                strokeWidth={strokeWidthPx}
                strokeLinejoin="round"
                strokeOpacity={fade}
              />
            </g>
          );
        })}
      <path d={front.area} fill={fill} />
      <path d={front.line} fill="none" stroke={accent} strokeWidth={strokeWidthPx * 2.6} strokeLinecap="round" opacity={0.22} />
      <path
        d={front.line}
        fill="none"
        stroke={mixRecordingOverlayHexColors(accent, "#ffffff", 0.25)}
        strokeWidth={strokeWidthPx}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
};

interface RecordingOverlayCarouselProps {
  levels: number[];
  widthPx: number;
  heightPx: number;
  barWidthPx: number;
  accent: string;
  transition: string;
  animated: boolean;
}

/** One turn of the carousel; each CSS sweep covers half of it. */
const CAROUSEL_TURN_S = 9;
/** Bar scale at the back and front of the ring; the midpoint is the side scale. */
const CAROUSEL_FAR_SCALE = 0.58;
const CAROUSEL_NEAR_SCALE = 1.06;

/**
 * Bars standing on a turning ring, seen from a little above. Two eased
 * half-turn sweeps a quarter turn apart trace each bar's circle: one moves it
 * sideways, the other carries depth (scale, lift, and dimming). Both are plain
 * CSS animations on the compositor; the voice only changes each bar's height.
 */
const RecordingOverlayCarousel: React.FC<RecordingOverlayCarouselProps> = ({
  levels,
  widthPx,
  heightPx,
  barWidthPx,
  accent,
  transition,
  animated,
}) => {
  const count = levels.length;
  const sideScale = (CAROUSEL_FAR_SCALE + CAROUSEL_NEAR_SCALE) / 2;
  const radius = Math.max(4, (widthPx - barWidthPx) / 2);
  const tilt = Math.max(3, Math.round(heightPx * 0.2));
  const barHeight = (heightPx - 2) / CAROUSEL_NEAR_SCALE;
  const halfTurn = CAROUSEL_TURN_S / 2;

  return (
    <div
      className={`rod-carousel${animated ? "" : " rod-carousel--still"}`}
      style={{ width: `${widthPx}px`, height: `${heightPx}px` }}
      aria-hidden="true"
    >
      <div
        className="rod-carousel__floor"
        style={{
          width: `${radius * 2}px`,
          height: `${tilt}px`,
          borderColor: recordingOverlayHexToRgba(accent, 0.35),
          background: `radial-gradient(closest-side, ${recordingOverlayHexToRgba(accent, 0.18)}, ${recordingOverlayHexToRgba(accent, 0)})`,
        }}
      />
      {levels.map((rawLevel, index) => {
        const level = clampUnit(rawLevel);
        const phase = index / count;
        const color = shiftRecordingOverlayHue(accent, (phase - 0.5) * 60);
        // The resting pose of the still variant is the same circle, frozen.
        const angle = phase * Math.PI * 2;
        const depth = Math.cos(angle);
        const restScale = sideScale + depth * (CAROUSEL_NEAR_SCALE - sideScale);
        return (
          <div
            key={index}
            className="rod-carousel__orbit"
            style={{
              height: `${tilt}px`,
              "--rod-duration": `${halfTurn}s`,
              "--rod-delay": `${-(phase + 0.25) * CAROUSEL_TURN_S}s`,
              "--rod-rest": `translateY(${(-(1 - depth) / 2) * 100}%) scale(${restScale})`,
              "--rod-rest-opacity": `${0.3 + ((depth + 1) / 2) * 0.7}`,
            } as React.CSSProperties}
          >
            <div
              className="rod-carousel__sweep"
              style={{
                width: `${radius / sideScale}px`,
                "--rod-duration": `${halfTurn}s`,
                "--rod-delay": `${-phase * CAROUSEL_TURN_S}s`,
                "--rod-rest": `translateX(${-Math.cos(angle - Math.PI / 2) * 100}%)`,
              } as React.CSSProperties}
            >
              <div
                style={{
                  position: "absolute",
                  left: `${-barWidthPx / 2}px`,
                  bottom: 0,
                  width: `${barWidthPx}px`,
                  height: `${barHeight}px`,
                  borderRadius: `${Math.min(3, barWidthPx / 2)}px ${Math.min(3, barWidthPx / 2)}px 1px 1px`,
                  background: `linear-gradient(90deg, ${mixRecordingOverlayHexColors(color, "#ffffff", 0.35)} 0%, ${color} 45%, ${mixRecordingOverlayHexColors(color, "#000000", 0.45)} 100%)`,
                  boxShadow: `0 0 5px ${recordingOverlayHexToRgba(color, 0.4)}`,
                  transformOrigin: "50% 100%",
                  transform: `scaleY(${0.12 + Math.pow(level, 0.75) * 0.88})`,
                  transition,
                }}
              />
            </div>
          </div>
        );
      })}
    </div>
  );
};

/** One full twist of the ribbon, and how many twists fit along its length. */
const RIBBON_TWIST_S = 3.4;
const RIBBON_TWISTS_ALONG = 1.25;

interface RecordingOverlayTwistRibbonProps {
  levels: number[];
  widthPx: number;
  heightPx: number;
  accent: string;
  transition: string;
  animated: boolean;
}

/**
 * A ribbon twisting along its length. Each slice turns with rotateX and no
 * perspective, which projects exactly as a cosine squash, and later slices lag
 * behind, so the twist travels along the band. Front and back faces are
 * separate layers with hidden backfaces, so the ribbon shows two colors as it
 * turns over. The voice widens each slice; nothing runs on the main thread.
 */
const RecordingOverlayTwistRibbon: React.FC<RecordingOverlayTwistRibbonProps> = ({
  levels,
  widthPx,
  heightPx,
  accent,
  transition,
  animated,
}) => {
  const count = levels.length;
  const slices = Math.max(10, Math.min(28, count * 2));
  const sliceWidth = widthPx / slices;
  const secondary = shiftRecordingOverlayHue(accent, 150);
  const frontFace = `linear-gradient(180deg, ${mixRecordingOverlayHexColors(accent, "#ffffff", 0.55)} 0%, ${accent} 46%, ${mixRecordingOverlayHexColors(accent, "#000000", 0.4)} 100%)`;
  const backFace = `linear-gradient(180deg, ${mixRecordingOverlayHexColors(secondary, "#000000", 0.55)} 0%, ${mixRecordingOverlayHexColors(secondary, "#000000", 0.2)} 60%, ${mixRecordingOverlayHexColors(secondary, "#ffffff", 0.25)} 100%)`;

  return (
    <div
      className={`rod-ribbon${animated ? "" : " rod-ribbon--still"}`}
      style={{ width: `${widthPx}px`, height: `${heightPx}px` }}
      aria-hidden="true"
    >
      {Array.from({ length: slices }, (_, slice) => {
        // Sample the levels at the slice center, blending neighbouring lanes.
        const at = count > 1 ? (slice / (slices - 1)) * (count - 1) : 0;
        const lower = Math.floor(at);
        const upper = Math.min(count - 1, lower + 1);
        const level = clampUnit(
          (levels[lower] ?? 0) * (1 - (at - lower)) + (levels[upper] ?? 0) * (at - lower),
        );
        const envelope = 0.55 + 0.45 * Math.sin(((slice + 0.5) / slices) * Math.PI);
        const phase = (slice / slices) * RIBBON_TWISTS_ALONG;
        const restDegrees = phase * 360;
        const faceStyle = (offsetDegrees: number) =>
          ({
            "--rod-duration": `${RIBBON_TWIST_S}s`,
            "--rod-delay": `${-phase * RIBBON_TWIST_S}s`,
            "--rod-rest": `rotateX(${restDegrees + offsetDegrees}deg)`,
          }) as React.CSSProperties;
        return (
          <div
            key={slice}
            className="rod-ribbon__slice"
            style={{
              left: `${slice * sliceWidth}px`,
              width: `${sliceWidth + 0.6}px`,
              transform: `scaleY(${(0.2 + Math.pow(level, 0.8) * 0.8) * envelope})`,
              transition,
            }}
          >
            <div
              className="rod-ribbon__face rod-ribbon__face--front"
              style={{ ...faceStyle(0), background: frontFace }}
            />
            <div
              className="rod-ribbon__face rod-ribbon__face--back"
              style={{ ...faceStyle(180), background: backFace }}
            />
          </div>
        );
      })}
    </div>
  );
};

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
  const mirrorSecondary = shiftRecordingOverlayHue(accent, 36);
  const trackWidth = effectiveCount * effectiveWidth + (effectiveCount - 1) * BAR_GAP_PX;

  if (normalizedStyle === "ridgeline") {
    return (
      <RecordingOverlayRidgeline
        levels={levels.slice(0, effectiveCount)}
        widthPx={trackWidth}
        heightPx={maxHeightPx + 4}
        strokeWidthPx={Math.max(1.2, Math.min(2.2, effectiveWidth * 0.4))}
        accent={accent}
        animated={animated}
        softness={softness}
      />
    );
  }

  if (normalizedStyle === "carousel") {
    return (
      <RecordingOverlayCarousel
        levels={levels.slice(0, effectiveCount)}
        widthPx={trackWidth}
        heightPx={maxHeightPx + 4}
        barWidthPx={effectiveWidth}
        accent={accent}
        transition={transition}
        animated={animated}
      />
    );
  }

  if (normalizedStyle === "twist_ribbon") {
    return (
      <RecordingOverlayTwistRibbon
        levels={levels.slice(0, effectiveCount)}
        widthPx={trackWidth}
        heightPx={maxHeightPx + 4}
        accent={accent}
        transition={transition}
        animated={animated}
      />
    );
  }

  if (normalizedStyle === "cubes") {
    return (
      <RecordingOverlayCubes
        levels={levels.slice(0, effectiveCount)}
        laneWidthPx={laneWidth}
        heightPx={maxHeightPx + 4}
        accent={accent}
        animated={animated}
        softness={softness}
      />
    );
  }

  if (normalizedStyle === "liquid") {
    return (
      <RecordingOverlayLiquid
        levels={levels.slice(0, effectiveCount)}
        widthPx={effectiveCount * effectiveWidth + (effectiveCount - 1) * BAR_GAP_PX}
        heightPx={maxHeightPx + 4}
        blobWidthPx={effectiveWidth}
        accent={accent}
        animated={animated}
        softness={softness}
      />
    );
  }

  if (normalizedStyle === "wave_line") {
    // Same track width as the bars, so the window and preview sizing still fit.
    return (
      <RecordingOverlayWaveLine
        levels={levels.slice(0, effectiveCount)}
        widthPx={effectiveCount * effectiveWidth + (effectiveCount - 1) * BAR_GAP_PX}
        heightPx={maxHeightPx + 4}
        strokeWidthPx={Math.max(1.5, Math.min(3, effectiveWidth * 0.55))}
        accent={accent}
        animated={animated}
        softness={softness}
      />
    );
  }

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

        if (normalizedStyle === "pillars") {
          // Three faces of a block in oblique projection: the side face is
          // sheared up and back, the top face sheared sideways. Only transforms
          // change with the level, so the compositor animates them.
          const depth = laneWidth - effectiveWidth;
          const trackHeight = maxHeightPx + 4;
          const columnHeight = trackHeight - depth;
          const rise = 2 + Math.pow(level, 0.75) * (columnHeight - 2);
          const position = effectiveCount > 1 ? index / (effectiveCount - 1) : 0.5;
          const color = shiftRecordingOverlayHue(accent, (position - 0.5) * 28);
          const face: React.CSSProperties = {
            position: "absolute",
            bottom: 0,
            transformOrigin: "0 100%",
            transition,
          };
          return (
            <div
              key={index}
              style={{
                width: `${laneWidth}px`,
                height: `${trackHeight}px`,
                position: "relative",
              }}
            >
              <div
                style={{
                  ...face,
                  left: `${effectiveWidth - 0.5}px`,
                  width: `${depth + 0.5}px`,
                  height: `${columnHeight}px`,
                  background: `linear-gradient(180deg, ${mixRecordingOverlayHexColors(color, "#000000", 0.36)}, ${mixRecordingOverlayHexColors(color, "#000000", 0.66)})`,
                  transform: `skewY(-45deg) scaleY(${rise / columnHeight})`,
                }}
              />
              <div
                style={{
                  ...face,
                  left: 0,
                  width: `${effectiveWidth}px`,
                  height: `${columnHeight}px`,
                  background: `linear-gradient(180deg, ${mixRecordingOverlayHexColors(color, "#ffffff", 0.24)} 0%, ${color} 42%, ${mixRecordingOverlayHexColors(color, "#000000", 0.32)} 100%)`,
                  boxShadow: "inset 1px 0 0 rgba(255,255,255,0.22)",
                  transform: `scaleY(${rise / columnHeight})`,
                }}
              />
              <div
                style={{
                  ...face,
                  left: 0,
                  width: `${effectiveWidth}px`,
                  height: `${depth}px`,
                  background: `linear-gradient(90deg, ${mixRecordingOverlayHexColors(color, "#ffffff", 0.66)}, ${mixRecordingOverlayHexColors(color, "#ffffff", 0.4)})`,
                  transform: `translateY(${-(rise - 0.5)}px) skewX(-45deg)`,
                }}
              />
              {/* The lid lights up on loud syllables. */}
              <div
                style={{
                  ...face,
                  left: 0,
                  width: `${effectiveWidth}px`,
                  height: `${depth}px`,
                  background: "rgba(255,255,255,0.9)",
                  boxShadow: `0 0 6px ${recordingOverlayHexToRgba(color, 0.9)}`,
                  opacity: Math.max(0, level - 0.55) * 1.6,
                  transform: `translateY(${-(rise - 0.5)}px) skewX(-45deg)`,
                }}
              />
            </div>
          );
        }

        if (normalizedStyle === "orbs") {
          // Glossy marbles over their own contact shadows. They squash at
          // rest and stretch as they jump; the shadow shrinks and fades below.
          const trackHeight = maxHeightPx + 4;
          const diameter = Math.max(6, Math.min(laneWidth - 1, trackHeight * 0.6));
          const travel = Math.max(0, trackHeight - diameter - 4);
          const lift = Math.pow(level, 0.8);
          const position = effectiveCount > 1 ? index / (effectiveCount - 1) : 0.5;
          const color = shiftRecordingOverlayHue(accent, (position - 0.5) * 36);
          const left = (laneWidth - diameter) / 2;
          return (
            <div
              key={index}
              style={{
                width: `${laneWidth}px`,
                height: `${trackHeight}px`,
                position: "relative",
              }}
            >
              <div
                style={{
                  position: "absolute",
                  left: `${left - diameter * 0.15}px`,
                  bottom: "0.5px",
                  width: `${diameter * 1.3}px`,
                  height: "4px",
                  borderRadius: "999px",
                  background: `radial-gradient(closest-side, rgba(0,0,0,0.55) 0%, rgba(0,0,0,0) 70%), radial-gradient(closest-side, ${recordingOverlayHexToRgba(color, 0.5)} 0%, ${recordingOverlayHexToRgba(color, 0)} 100%)`,
                  opacity: 1 - lift * 0.65,
                  transform: `scaleX(${1 - lift * 0.45})`,
                  transition,
                }}
              />
              <div
                style={{
                  position: "absolute",
                  left: `${left}px`,
                  bottom: "2px",
                  width: `${diameter}px`,
                  height: `${diameter}px`,
                  borderRadius: "50%",
                  background: [
                    "radial-gradient(circle at 33% 27%, rgba(255,255,255,0.96) 0%, rgba(255,255,255,0.5) 9%, rgba(255,255,255,0) 22%)",
                    `radial-gradient(circle at 50% 108%, ${recordingOverlayHexToRgba(mixRecordingOverlayHexColors(color, "#ffffff", 0.4), 0.7)} 0%, ${recordingOverlayHexToRgba(color, 0)} 42%)`,
                    `radial-gradient(circle at 38% 32%, ${mixRecordingOverlayHexColors(color, "#ffffff", 0.42)} 0%, ${color} 44%, ${mixRecordingOverlayHexColors(color, "#000000", 0.62)} 100%)`,
                  ].join(", "),
                  boxShadow: `0 0 5px ${recordingOverlayHexToRgba(color, 0.35)}`,
                  transformOrigin: "50% 100%",
                  transform: `translateY(${-lift * travel}px) scale(${1.1 - lift * 0.16}, ${0.9 + lift * 0.16})`,
                  transition,
                }}
              />
            </div>
          );
        }

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

        if (normalizedStyle === "mirror") {
          // Silence shrinks every bar to a dot; color sweeps across the row.
          const color = mixRecordingOverlayHexColors(
            accent,
            mirrorSecondary,
            effectiveCount > 1 ? index / (effectiveCount - 1) : 0,
          );
          return (
            <div
              key={index}
              style={{
                width: `${effectiveWidth}px`,
                height: `${Math.max(effectiveWidth, 3 + Math.pow(level, 0.8) * (maxHeightPx - 3))}px`,
                borderRadius: "999px",
                background: recordingOverlayHexToRgba(color, 0.95),
                opacity: Math.max(0.5, Math.min(1, 0.5 + level)),
                transition,
              }}
            />
          );
        }

        if (normalizedStyle === "spectrum") {
          // The hue walks across the row, splitting the accent like a prism.
          const position = effectiveCount > 1 ? index / (effectiveCount - 1) : 0;
          const color = shiftRecordingOverlayHue(accent, position * 220 - 40);
          return (
            <div
              key={index}
              style={{
                width: `${effectiveWidth}px`,
                height: `${Math.max(effectiveWidth, 3 + Math.pow(level, 0.8) * (maxHeightPx - 3))}px`,
                borderRadius: "999px",
                background: `linear-gradient(180deg, ${mixRecordingOverlayHexColors(color, "#ffffff", 0.3)}, ${color})`,
                boxShadow: `0 0 8px ${recordingOverlayHexToRgba(color, 0.35)}`,
                opacity: Math.max(0.55, Math.min(1, 0.55 + level)),
                transition,
              }}
            />
          );
        }

        if (normalizedStyle === "dot_matrix") {
          const rows = 5;
          const gapPx = 1.25;
          const dotPx = Math.max(2, Math.min(effectiveWidth, (maxHeightPx - (rows - 1) * gapPx) / rows));
          const spread = level * ((rows - 1) / 2 + 0.6);
          return (
            <div
              key={index}
              style={{
                width: `${effectiveWidth}px`,
                height: `${maxHeightPx}px`,
                display: "flex",
                flexDirection: "column",
                alignItems: "center",
                justifyContent: "center",
                gap: `${gapPx}px`,
              }}
            >
              {Array.from({ length: rows }, (_, row) => {
                const distance = Math.abs(row - (rows - 1) / 2);
                const lit = distance <= spread;
                return (
                  <span
                    key={row}
                    style={{
                      width: `${dotPx}px`,
                      height: `${dotPx}px`,
                      borderRadius: "999px",
                      background: !lit
                        ? recordingOverlayHexToRgba(accent, 0.12)
                        : distance === 0
                          ? recordingOverlayHexToRgba(accent, 0.55 + Math.min(1, level * 2) * 0.45)
                          : recordingOverlayHexToRgba(accent, 0.92),
                      transition,
                    }}
                  />
                );
              })}
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
