import React from "react";
import "./RecordingOverlayPortal.css";

/** The portal plane is seen slightly from the side. */
export const OVERLAY_PORTAL_TILT_DEG = 4;

/**
 * Vertical rise of the tilted portal plane across half the frame width. The
 * overlay's clip line uses the same value so the frame passes exactly through
 * the portal surface.
 */
export function overlayPortalTiltPx(frameWidth: number): number {
  return Math.tan((OVERLAY_PORTAL_TILT_DEG * Math.PI) / 180) * (frameWidth / 2);
}

const PALETTES = {
  in: {
    tongue: "#0a3fd8",
    body: "#0a62ff",
    edge: "#3a9cff",
    hot: "#a9dcff",
  },
  out: {
    tongue: "#d84800",
    body: "#ff7c00",
    edge: "#ffa31c",
    hot: "#ffe39a",
  },
} as const;

// Circle-space radius and band thickness. The ring is drawn as a round portal
// and scaled to the visible ellipse, so band and flames foreshorten naturally.
const R = 100;
const BAND = 17;
const VISIBLE_RY = 13;
const GLOW_PAD = 22;

// Deterministic so every render (and both halves) shows the same flames and sparks.
function seededRandom(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state * 1664525 + 1013904223) >>> 0;
    return state / 4294967296;
  };
}

type Tongue = { angle: number; width: number; length: number };

function makeTongues(seed: number, count: number): Tongue[] {
  const random = seededRandom(seed);
  return Array.from({ length: count }, (_, index) => ({
    angle: (360 * (index + random() * 0.6)) / count,
    width: BAND * (0.28 + random() * 0.3),
    length: BAND * (0.9 + random() * 1.1),
  }));
}

const OUTER_TONGUES = makeTongues(7, 84);
const INNER_TONGUES = makeTongues(19, 70);

// Radii are fractions of the portal radius. They stop at the band's inner edge
// (about 0.87 R) so sparks die out in the rim instead of crossing it. Times are
// in ms from mount.
type Spark = {
  angle: number;
  from: number;
  to: number;
  length: number;
  width: number;
  delay: number;
  duration: number;
  hot: boolean;
};

function makeSparks(seed: number, count: number): Spark[] {
  const random = seededRandom(seed);
  return Array.from({ length: count }, (_, index) => ({
    angle: (2 * Math.PI * (index + random() * 0.8)) / count,
    from: 0.04 + random() * 0.26,
    to: 0.6 + random() * 0.28,
    length: 4 + random() * 7,
    width: 0.9 + random() * 1,
    delay: random() * 150,
    duration: 170 + random() * 160,
    hot: random() < 0.45,
  }));
}

const SPARKS = makeSparks(29, 30);

// The back layer draws the whole ring behind the frame; the front layer repeats
// only the near arc on top of it. Splitting the ring into two clipped halves
// instead leaves an anti-aliased seam where they meet at the ends. Seen from
// below, a ceiling portal's near arc is the upper one; a floor portal's is the lower one.
function nearArcClipY(direction: "in" | "out"): number {
  return direction === "in" ? -4 * R : 0;
}

interface PortalHalfProps {
  direction: "in" | "out";
  half: "back" | "front";
  rx: number;
  top: string;
  onAnimationEnd?: () => void;
}

const PortalHalf: React.FC<PortalHalfProps> = ({
  direction,
  half,
  rx,
  top,
  onAnimationEnd,
}) => {
  const palette = PALETTES[direction];
  const id = `aivo-portal-${half}`;
  const width = Math.ceil(2 * (rx + GLOW_PAD));
  const height = Math.ceil(2 * (VISIBLE_RY + GLOW_PAD));
  const clipPath = half === "front" ? `url(#${id}-clip)` : undefined;

  const tongues = (items: Tongue[], radius: number) =>
    items.map((tongue, index) => (
      <use
        key={index}
        href={`#${id}-tongue`}
        transform={`rotate(${tongue.angle.toFixed(2)}) translate(0 ${(-radius).toFixed(2)}) scale(${tongue.width.toFixed(2)} ${tongue.length.toFixed(2)})`}
      />
    ));

  return (
    <svg
      className={`overlay-portal overlay-portal--${direction} overlay-portal--${half}`}
      width={width}
      height={height}
      viewBox={`${-width / 2} ${-height / 2} ${width} ${height}`}
      style={{ top }}
      onAnimationEnd={(event) => {
        if (event.target === event.currentTarget) {
          onAnimationEnd?.();
        }
      }}
      aria-hidden="true"
    >
      <defs>
        <clipPath id={`${id}-clip`} clipPathUnits="userSpaceOnUse">
          <rect
            x={-4 * R}
            y={nearArcClipY(direction)}
            width={8 * R}
            height={4 * R}
          />
        </clipPath>
        <linearGradient id={`${id}-tongue-fill`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor={palette.body} stopOpacity={0.95} />
          <stop offset="0.45" stopColor={palette.tongue} stopOpacity={0.9} />
          <stop offset="1" stopColor={palette.tongue} stopOpacity={0} />
        </linearGradient>
        <path
          id={`${id}-tongue`}
          d="M -0.5 -0.25 L -0.5 0 C -0.5 0.35 -0.16 0.62 0 1 C 0.16 0.62 0.5 0.35 0.5 0 L 0.5 -0.25 Z"
          fill={`url(#${id}-tongue-fill)`}
        />
        {/* Flickering flame body */}
        <filter id={`${id}-flame`} x="-15%" y="-15%" width="130%" height="130%">
          <feTurbulence
            type="fractalNoise"
            baseFrequency="0.07"
            numOctaves={3}
            seed={3}
            result="noise"
          >
            <animate
              attributeName="seed"
              values="3;8;13;18;23;28;33;38;43"
              dur="0.6s"
              calcMode="discrete"
              repeatCount="indefinite"
            />
          </feTurbulence>
          <feDisplacementMap
            in="SourceGraphic"
            in2="noise"
            scale={9}
            xChannelSelector="R"
            yChannelSelector="G"
            result="displaced"
          />
          <feGaussianBlur in="displaced" stdDeviation={0.9} />
        </filter>
        <filter id={`${id}-edge`} x="-15%" y="-15%" width="130%" height="130%">
          <feTurbulence
            type="fractalNoise"
            baseFrequency="0.11"
            numOctaves={2}
            seed={5}
            result="noise"
          >
            <animate
              attributeName="seed"
              values="5;7;9;11;13;15"
              dur="0.42s"
              calcMode="discrete"
              repeatCount="indefinite"
            />
          </feTurbulence>
          <feDisplacementMap
            in="SourceGraphic"
            in2="noise"
            scale={3.5}
            xChannelSelector="R"
            yChannelSelector="G"
            result="displaced"
          />
          <feGaussianBlur in="displaced" stdDeviation={0.8} />
        </filter>
      </defs>

      <g transform={`rotate(${-OVERLAY_PORTAL_TILT_DEG})`}>
        <g clipPath={clipPath}>
          <PortalSparks
            id={id}
            palette={palette}
            rx={rx}
            width={width}
            height={height}
          />
        </g>
        <g transform={`scale(${(rx / R).toFixed(4)} ${(VISIBLE_RY / R).toFixed(4)})`}>
          <g clipPath={clipPath}>
            <g filter={`url(#${id}-flame)`}>
              {/* Two counter-rotating flame sets make the rim swirl. */}
              <g opacity={0.85}>
                {tongues(OUTER_TONGUES, R - 0.45 * BAND)}
                <animateTransform
                  attributeName="transform"
                  type="rotate"
                  from="0"
                  to="360"
                  dur="11s"
                  repeatCount="indefinite"
                />
              </g>
              <g opacity={0.6}>
                {tongues(INNER_TONGUES, R - 0.6 * BAND)}
                <animateTransform
                  attributeName="transform"
                  type="rotate"
                  from="0"
                  to="-360"
                  dur="16s"
                  repeatCount="indefinite"
                />
              </g>
              <circle
                r={R - 0.4 * BAND}
                fill="none"
                stroke={palette.body}
                strokeWidth={0.72 * BAND}
              />
            </g>
            <circle
              r={R - 0.2 * BAND}
              fill="none"
              stroke={palette.edge}
              strokeWidth={0.42 * BAND}
              filter={`url(#${id}-edge)`}
            />
            <circle
              r={R - 0.22 * BAND}
              fill="none"
              stroke={palette.hot}
              strokeWidth={0.08 * BAND}
              opacity={0.5}
              strokeDasharray="40 14 9 22 55 16 20 11"
              filter={`url(#${id}-edge)`}
            >
              <animate
                attributeName="stroke-dashoffset"
                from="0"
                to="-187"
                dur="2.2s"
                repeatCount="indefinite"
              />
            </circle>
          </g>
        </g>
      </g>
    </svg>
  );
};

interface PortalSparksProps {
  id: string;
  palette: (typeof PALETTES)[keyof typeof PALETTES];
  rx: number;
  width: number;
  height: number;
}

/**
 * Sparks that burst from the middle of the opening portal towards its rim.
 * Drawn inside the ring's SVG, under the band, so they grow with the opening
 * ring and never fly past it; positions follow the same foreshortened plane.
 */
const PortalSparks: React.FC<PortalSparksProps> = ({
  id,
  palette,
  rx,
  width,
  height,
}) => (
  <>
    <defs>
      <filter
        id={`${id}-spark-glow`}
        filterUnits="userSpaceOnUse"
        x={-width}
        y={-height}
        width={2 * width}
        height={2 * height}
      >
        <feGaussianBlur in="SourceAlpha" stdDeviation={2.2} result="blur" />
        <feFlood floodColor={palette.edge} />
        <feComposite in2="blur" operator="in" result="glow" />
        <feMerge>
          <feMergeNode in="glow" />
          <feMergeNode in="glow" />
          <feMergeNode in="glow" />
          <feMergeNode in="SourceGraphic" />
        </feMerge>
      </filter>
    </defs>
    <g filter={`url(#${id}-spark-glow)`}>
      {SPARKS.map((spark, index) => {
        const cos = Math.cos(spark.angle);
        const sin = Math.sin(spark.angle);
        const x0 = spark.from * rx * cos;
        const y0 = spark.from * VISIBLE_RY * sin;
        const x1 = spark.to * rx * cos;
        const y1 = spark.to * VISIBLE_RY * sin;
        // Tapered motion-blur tail behind a hot head; sparks flying
        // towards or away from the viewer foreshorten into short dots.
        const travel = Math.hypot(x1 - x0, y1 - y0) || 1;
        const ux = (x1 - x0) / travel;
        const uy = (y1 - y0) / travel;
        const tail = Math.min(spark.length, travel * 0.6);
        const halfWidth = spark.width / 2;
        const tailPath = `M ${(-uy * halfWidth).toFixed(2)} ${(ux * halfWidth).toFixed(2)} L ${(-ux * tail).toFixed(2)} ${(-uy * tail).toFixed(2)} L ${(uy * halfWidth).toFixed(2)} ${(-ux * halfWidth).toFixed(2)} Z`;
        return (
          <g
            key={index}
            className="overlay-portal-spark"
            style={
              {
                "--spark-x0": `${x0.toFixed(2)}px`,
                "--spark-y0": `${y0.toFixed(2)}px`,
                "--spark-x1": `${x1.toFixed(2)}px`,
                "--spark-y1": `${y1.toFixed(2)}px`,
                "--spark-delay": `${spark.delay.toFixed(0)}ms`,
                "--spark-duration": `${spark.duration.toFixed(0)}ms`,
              } as React.CSSProperties
            }
          >
            <path
              className="overlay-portal-spark-tail"
              d={tailPath}
              fill={spark.hot ? palette.hot : palette.edge}
            />
            <circle
              r={halfWidth.toFixed(2)}
              fill={spark.hot ? "#ffffff" : palette.hot}
            />
          </g>
        );
      })}
    </g>
  </>
);

interface RecordingOverlayPortalProps {
  direction: "in" | "out";
  frameWidth: number;
  frameHeight: number;
  onDone: () => void;
}

/**
 * Portal-style rings for the overlay "portal" animations: a blue ceiling
 * portal on the frame's top edge for the entrance and an orange floor portal
 * on its bottom edge for the exit. Rendered into document.body so they stay
 * put while the frame moves; the whole ring sits behind the frame and its near
 * arc is repeated in front of it.
 */
export const RecordingOverlayPortal: React.FC<RecordingOverlayPortalProps> = ({
  direction,
  frameWidth,
  frameHeight,
  onDone,
}) => {
  const rx = frameWidth / 2 + 6;
  const offset = frameHeight / 2 + overlayPortalTiltPx(frameWidth);
  const top =
    direction === "in" ? `calc(50% - ${offset}px)` : `calc(50% + ${offset}px)`;

  return (
    <>
      <PortalHalf direction={direction} half="back" rx={rx} top={top} />
      <PortalHalf
        direction={direction}
        half="front"
        rx={rx}
        top={top}
        onAnimationEnd={onDone}
      />
    </>
  );
};
