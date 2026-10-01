import type { CSSProperties } from "react";

export type RecordingOverlayTheme = "classic" | "minimal" | "glass";
export type RecordingOverlayMaterialMode =
  | "liquid_glass"
  | "pearl"
  | "velvet_neon"
  | "frost"
  | "candy_chrome"
  | "graphite"
  | "obsidian"
  | "gradient_mesh"
  | "porcelain"
  | "clay"
  | "keycap";
export type RecordingOverlayBackgroundMode =
  | "none"
  | "mist"
  | "petals_haze"
  | "soft_glow_field"
  | "stardust"
  | "silk_fog"
  | "firefly_veil"
  | "rose_sparks"
  | "horizon_grid"
  | "starfield_warp"
  | "tunnel_rings"
  | "galaxy_spiral"
  | "dot_swell";
export type RecordingOverlayCenterpieceMode =
  | "none"
  | "halo_core"
  | "aurora_ribbon"
  | "orbital_beads"
  | "bloom_heart"
  | "signal_crown"
  | "gyroscope"
  | "holo_globe"
  | "ringed_planet"
  | "plasma_orb";
export type RecordingOverlayAnimatedBorderMode =
  | "none"
  | "shimmer_edge"
  | "traveling_highlight"
  | "breathing_contour"
  | "spectrum_edge";
export type RecordingOverlayStatusIconStyle =
  | "auto"
  | "capsule"
  | "bare"
  | "ring"
  | "tile"
  | "dot"
  | "orb"
  | "coin";
export type RecordingOverlayBarStyle =
  | "aurora"
  | "bloom_bounce"
  | "constellation"
  | "comet"
  | "crown"
  | "daisy"
  | "ember"
  | "fireflies"
  | "garden_sway"
  | "hologram"
  | "helix"
  | "lotus"
  | "matrix"
  | "morse"
  | "needles"
  | "orbit"
  | "petals"
  | "petal_rain"
  | "pulse_rings"
  | "retro"
  | "radar"
  | "shards"
  | "skyline"
  | "solid"
  | "capsule"
  | "glow"
  | "prism"
  | "tuner"
  | "vinyl"
  | "wave_line"
  | "mirror"
  | "dot_matrix"
  | "spectrum"
  | "liquid"
  | "pillars"
  | "orbs"
  | "cubes"
  | "ridgeline"
  | "carousel"
  | "twist_ribbon";

export const LEGACY_RECORDING_OVERLAY_BAR_STYLES: RecordingOverlayBarStyle[] = [
  "solid",
  "capsule",
  "glow",
  "prism",
];

export function normalizeRecordingOverlayColor(
  value: string | undefined,
  fallback = "#ff4d8d",
): string {
  if (typeof value !== "string") {
    return fallback;
  }
  const trimmed = value.trim().toLowerCase();
  if (/^#[0-9a-f]{6}$/.test(trimmed)) {
    return trimmed;
  }
  return fallback;
}

export function recordingOverlayHexToRgba(hex: string, alpha: number): string {
  const normalized = normalizeRecordingOverlayColor(hex);
  const red = Number.parseInt(normalized.slice(1, 3), 16);
  const green = Number.parseInt(normalized.slice(3, 5), 16);
  const blue = Number.parseInt(normalized.slice(5, 7), 16);
  return `rgba(${red}, ${green}, ${blue}, ${alpha})`;
}

export function mixRecordingOverlayHexColors(
  first: string,
  second: string,
  secondWeight: number,
): string {
  const a = normalizeRecordingOverlayColor(first);
  const b = normalizeRecordingOverlayColor(second);
  const weight = Math.max(0, Math.min(1, secondWeight));
  const aRed = Number.parseInt(a.slice(1, 3), 16);
  const aGreen = Number.parseInt(a.slice(3, 5), 16);
  const aBlue = Number.parseInt(a.slice(5, 7), 16);
  const bRed = Number.parseInt(b.slice(1, 3), 16);
  const bGreen = Number.parseInt(b.slice(3, 5), 16);
  const bBlue = Number.parseInt(b.slice(5, 7), 16);
  const mixChannel = (aChannel: number, bChannel: number) =>
    Math.round((aChannel * (1 - weight)) + (bChannel * weight))
      .toString(16)
      .padStart(2, "0");

  return `#${mixChannel(aRed, bRed)}${mixChannel(aGreen, bGreen)}${mixChannel(aBlue, bBlue)}`;
}

/** Rotates the hue of a hex color, keeping its saturation and lightness. */
export function shiftRecordingOverlayHue(hex: string, degrees: number): string {
  const normalized = normalizeRecordingOverlayColor(hex);
  const red = Number.parseInt(normalized.slice(1, 3), 16) / 255;
  const green = Number.parseInt(normalized.slice(3, 5), 16) / 255;
  const blue = Number.parseInt(normalized.slice(5, 7), 16) / 255;
  const max = Math.max(red, green, blue);
  const min = Math.min(red, green, blue);
  const lightness = (max + min) / 2;
  const delta = max - min;
  if (delta === 0) {
    return normalized;
  }
  const saturation = delta / (1 - Math.abs(2 * lightness - 1));
  let hue =
    max === red
      ? ((green - blue) / delta) % 6
      : max === green
        ? (blue - red) / delta + 2
        : (red - green) / delta + 4;
  hue = (((hue * 60 + degrees) % 360) + 360) % 360;
  const chroma = (1 - Math.abs(2 * lightness - 1)) * saturation;
  const x = chroma * (1 - Math.abs(((hue / 60) % 2) - 1));
  const m = lightness - chroma / 2;
  const [r, g, b] =
    hue < 60 ? [chroma, x, 0]
    : hue < 120 ? [x, chroma, 0]
    : hue < 180 ? [0, chroma, x]
    : hue < 240 ? [0, x, chroma]
    : hue < 300 ? [x, 0, chroma]
    : [chroma, 0, x];
  const channel = (value: number) =>
    Math.round(Math.max(0, Math.min(1, value + m)) * 255)
      .toString(16)
      .padStart(2, "0");
  return `#${channel(r)}${channel(g)}${channel(b)}`;
}

export function normalizeRecordingOverlayBackgroundMode(
  value: string | undefined,
): RecordingOverlayBackgroundMode {
  switch (value) {
    case "mist":
      return "mist";
    case "petals_haze":
      return "petals_haze";
    case "soft_glow_field":
      return "soft_glow_field";
    case "stardust":
      return "stardust";
    case "silk_fog":
      return "silk_fog";
    case "firefly_veil":
      return "firefly_veil";
    case "rose_sparks":
      return "rose_sparks";
    case "horizon_grid":
      return "horizon_grid";
    case "starfield_warp":
      return "starfield_warp";
    case "tunnel_rings":
      return "tunnel_rings";
    case "galaxy_spiral":
      return "galaxy_spiral";
    case "dot_swell":
      return "dot_swell";
    default:
      return "none";
  }
}

export function normalizeRecordingOverlayMaterialMode(
  value: string | undefined,
): RecordingOverlayMaterialMode {
  switch (value) {
    case "pearl":
      return "pearl";
    case "velvet_neon":
      return "velvet_neon";
    case "frost":
      return "frost";
    case "candy_chrome":
      return "candy_chrome";
    case "graphite":
      return "graphite";
    case "obsidian":
      return "obsidian";
    case "gradient_mesh":
      return "gradient_mesh";
    case "porcelain":
      return "porcelain";
    case "clay":
      return "clay";
    case "keycap":
      return "keycap";
    case "liquid_glass":
    default:
      return "liquid_glass";
  }
}

export function normalizeRecordingOverlayCenterpieceMode(
  value: string | undefined,
): RecordingOverlayCenterpieceMode {
  switch (value) {
    case "halo_core":
      return "halo_core";
    case "aurora_ribbon":
      return "aurora_ribbon";
    case "orbital_beads":
      return "orbital_beads";
    case "bloom_heart":
      return "bloom_heart";
    case "signal_crown":
      return "signal_crown";
    case "gyroscope":
      return "gyroscope";
    case "holo_globe":
      return "holo_globe";
    case "ringed_planet":
      return "ringed_planet";
    case "plasma_orb":
      return "plasma_orb";
    default:
      return "none";
  }
}

export function normalizeRecordingOverlayAnimatedBorderMode(
  value: string | undefined,
): RecordingOverlayAnimatedBorderMode {
  switch (value) {
    case "shimmer_edge":
      return "shimmer_edge";
    case "traveling_highlight":
      return "traveling_highlight";
    case "breathing_contour":
      return "breathing_contour";
    case "spectrum_edge":
      return "spectrum_edge";
    default:
      return "none";
  }
}

export function normalizeRecordingOverlayStatusIconStyle(
  value: string | undefined,
): RecordingOverlayStatusIconStyle {
  switch (value) {
    case "capsule":
      return "capsule";
    case "bare":
      return "bare";
    case "ring":
      return "ring";
    case "tile":
      return "tile";
    case "dot":
      return "dot";
    case "orb":
      return "orb";
    case "coin":
      return "coin";
    default:
      return "auto";
  }
}

/**
 * The frame actually drawn. Auto keeps the glass capsule on the original
 * materials and drops it on the flat modern ones, where it looks dated. The
 * dimensional materials get a frame with matching volume.
 */
export function resolveRecordingOverlayStatusIconStyle(
  style: RecordingOverlayStatusIconStyle,
  materialMode: RecordingOverlayMaterialMode,
): Exclude<RecordingOverlayStatusIconStyle, "auto"> {
  if (style !== "auto") return style;
  if (materialMode === "clay") return "orb";
  if (materialMode === "keycap") return "tile";
  return materialMode === "graphite" ||
    materialMode === "obsidian" ||
    materialMode === "gradient_mesh" ||
    materialMode === "porcelain"
    ? "bare"
    : "capsule";
}

/** WCAG relative luminance of a hex color, from 0 (black) to 1 (white). */
export function recordingOverlayRelativeLuminance(hex: string): number {
  const normalized = normalizeRecordingOverlayColor(hex);
  const channel = (offset: number) => {
    const value = Number.parseInt(normalized.slice(offset, offset + 2), 16) / 255;
    return value <= 0.04045 ? value / 12.92 : Math.pow((value + 0.055) / 1.055, 2.4);
  };
  return 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
}

/** Dark text for surfaces too light for the default white. */
const DARK_INK = "rgba(24, 26, 32, 0.86)";

export function getRecordingOverlaySurfaceStyle(
  theme: RecordingOverlayTheme,
  accentColor: string,
  barWidthPx: number,
  opacityPercent = 100,
  materialMode: RecordingOverlayMaterialMode = "liquid_glass",
  surfaceBaseColor = "#101216",
  bodyBackgroundColor = "#101216",
): CSSProperties {
  const accent = normalizeRecordingOverlayColor(accentColor);
  const normalizedSurfaceBaseColor = normalizeRecordingOverlayColor(
    surfaceBaseColor,
    "#101216",
  );
  const normalizedBodyBackgroundColor = normalizeRecordingOverlayColor(
    bodyBackgroundColor,
    "#101216",
  );
  const surfaceOpacity = Math.max(20, Math.min(100, Math.round(opacityPercent))) / 100;
  const opaqueBaseAlpha = Math.min(1, 0.22 + (surfaceOpacity * 0.78));
  const normalizedMaterialMode = normalizeRecordingOverlayMaterialMode(materialMode);
  const baseGlow = recordingOverlayHexToRgba(accent, 0.18);
  const strongGlow = recordingOverlayHexToRgba(accent, 0.32);
  const sheen = recordingOverlayHexToRgba(accent, 0.14);
  const bodyHighlightColor = mixRecordingOverlayHexColors(
    normalizedBodyBackgroundColor,
    "#ffffff",
    0.12,
  );
  const bodyDeepColor = mixRecordingOverlayHexColors(
    normalizedBodyBackgroundColor,
    "#000000",
    0.22,
  );
  const bodyEdgeColor = mixRecordingOverlayHexColors(
    normalizedBodyBackgroundColor,
    "#000000",
    0.34,
  );
  const stackSurfaceLayers = (
    decorativeLayer: string,
    baseColor: string = normalizedSurfaceBaseColor,
  ): string => `
      ${decorativeLayer},
      linear-gradient(180deg, ${recordingOverlayHexToRgba(baseColor, opaqueBaseAlpha)} 0%, ${recordingOverlayHexToRgba(baseColor, opaqueBaseAlpha)} 100%)
    `;
  const baseStyle: CSSProperties = {
    "--recording-overlay-accent": accent,
    "--recording-overlay-accent-soft": recordingOverlayHexToRgba(accent, 0.22),
    "--recording-overlay-accent-border": recordingOverlayHexToRgba(accent, 0.34),
    "--recording-overlay-accent-glow": baseGlow,
    "--recording-overlay-accent-glow-strong": strongGlow,
    "--recording-overlay-sheen": sheen,
    "--recording-overlay-bar-color": recordingOverlayHexToRgba(accent, 0.9),
    "--recording-overlay-bar-width": `${Math.max(2, Math.min(12, Math.round(barWidthPx)))}px`,
  } as CSSProperties;

  const flatMaterialLayers = (
    sheenTopAlpha: number,
    grainOpacity: number,
  ): CSSProperties =>
    ({
      "--recording-overlay-accent-glow": "rgba(0, 0, 0, 0)",
      "--recording-overlay-accent-glow-strong": "rgba(0, 0, 0, 0)",
      "--recording-overlay-sheen": "rgba(0, 0, 0, 0)",
      "--recording-overlay-sheen-top-alpha": String(sheenTopAlpha),
      "--recording-overlay-grain-opacity": String(grainOpacity),
      "--recording-overlay-vignette-opacity": "0",
    }) as CSSProperties;
  const meshSecondaryColor = shiftRecordingOverlayHue(accent, 42);
  // Porcelain is always light; the body color only tints it.
  const porcelainBodyColor = mixRecordingOverlayHexColors(
    "#f6f6f8",
    normalizedBodyBackgroundColor,
    0.1,
  );
  const porcelainTopColor = mixRecordingOverlayHexColors(porcelainBodyColor, "#ffffff", 0.6);
  const porcelainBottomColor = mixRecordingOverlayHexColors(porcelainBodyColor, "#000000", 0.05);
  // Clay is a soft pastel of the body color with a hint of the accent; its
  // shadows take the same hue, as light bounced off colored clay would.
  const clayToneColor = mixRecordingOverlayHexColors(
    mixRecordingOverlayHexColors(normalizedBodyBackgroundColor, "#ffffff", 0.5),
    accent,
    0.1,
  );
  const clayTopColor = mixRecordingOverlayHexColors(clayToneColor, "#ffffff", 0.35);
  const clayBottomColor = mixRecordingOverlayHexColors(clayToneColor, "#000000", 0.1);
  const clayShadeColor = mixRecordingOverlayHexColors(clayToneColor, "#000000", 0.55);
  const clayIsLight = recordingOverlayRelativeLuminance(clayToneColor) > 0.4;
  // A keycap: the body color is the cap, a darker wall shows below it, and
  // the accent leaks out underneath like a keyboard backlight.
  const keycapTopColor = mixRecordingOverlayHexColors(normalizedBodyBackgroundColor, "#ffffff", 0.08);
  const keycapWallColor = mixRecordingOverlayHexColors(normalizedBodyBackgroundColor, "#000000", 0.45);
  const keycapIsLight = recordingOverlayRelativeLuminance(normalizedBodyBackgroundColor) > 0.4;

  const materialByMode: Record<RecordingOverlayMaterialMode, CSSProperties> = {
    liquid_glass: {
      "--recording-overlay-accent-glow": baseGlow,
      "--recording-overlay-accent-glow-strong": strongGlow,
      "--recording-overlay-sheen": sheen,
    } as CSSProperties,
    pearl: {
      "--recording-overlay-accent-glow": recordingOverlayHexToRgba(accent, 0.12),
      "--recording-overlay-accent-glow-strong": recordingOverlayHexToRgba(accent, 0.2),
      "--recording-overlay-sheen": "rgba(255,255,255,0.18)",
    } as CSSProperties,
    velvet_neon: {
      "--recording-overlay-accent-glow": recordingOverlayHexToRgba(accent, 0.24),
      "--recording-overlay-accent-glow-strong": recordingOverlayHexToRgba(accent, 0.42),
      "--recording-overlay-sheen": recordingOverlayHexToRgba(accent, 0.18),
    } as CSSProperties,
    frost: {
      "--recording-overlay-accent-glow": recordingOverlayHexToRgba(accent, 0.1),
      "--recording-overlay-accent-glow-strong": recordingOverlayHexToRgba(accent, 0.18),
      "--recording-overlay-sheen": "rgba(255,255,255,0.14)",
    } as CSSProperties,
    candy_chrome: {
      "--recording-overlay-accent-glow": recordingOverlayHexToRgba(accent, 0.22),
      "--recording-overlay-accent-glow-strong": recordingOverlayHexToRgba(accent, 0.36),
      "--recording-overlay-sheen": recordingOverlayHexToRgba(accent, 0.2),
    } as CSSProperties,
    // The modern materials drop the accent haze, sheen, and grain so the
    // surface reads as one clean, flat object.
    graphite: flatMaterialLayers(0.05, 0),
    obsidian: flatMaterialLayers(0.04, 0),
    gradient_mesh: flatMaterialLayers(0.06, 0.16),
    porcelain: {
      ...flatMaterialLayers(0, 0),
      // Text drawn on the surface switches to dark ink.
      "--recording-overlay-ink": DARK_INK,
    } as CSSProperties,
    clay: {
      ...flatMaterialLayers(0, 0),
      ...(clayIsLight ? { "--recording-overlay-ink": DARK_INK } : {}),
    } as CSSProperties,
    keycap: {
      ...flatMaterialLayers(0, 0),
      ...(keycapIsLight ? { "--recording-overlay-ink": DARK_INK } : {}),
    } as CSSProperties,
  };

  const themedBase = (() => {
    switch (theme) {
      case "minimal":
        return {
          background: stackSurfaceLayers(
            `linear-gradient(180deg, ${recordingOverlayHexToRgba(bodyHighlightColor, 0.82 * surfaceOpacity)} 0%, ${recordingOverlayHexToRgba(bodyDeepColor, 0.92 * surfaceOpacity)} 100%)`,
          ),
          border: `1px solid ${recordingOverlayHexToRgba(accent, 0.18)}`,
          borderRadius: "12px",
          boxShadow: `inset 0 1px 0 rgba(255, 255, 255, 0.04), 0 8px 18px rgba(0, 0, 0, 0.22), 0 0 0 1px ${recordingOverlayHexToRgba(accent, 0.04)}`,
        };
      case "glass":
        return {
          background: stackSurfaceLayers(
            `linear-gradient(180deg, ${recordingOverlayHexToRgba(accent, 0.22 * surfaceOpacity)} 0%, ${recordingOverlayHexToRgba(bodyDeepColor, 0.66 * surfaceOpacity)} 52%, ${recordingOverlayHexToRgba(bodyEdgeColor, 0.76 * surfaceOpacity)} 100%)`,
          ),
          border: `1px solid ${recordingOverlayHexToRgba(accent, 0.26)}`,
          borderRadius: "18px",
          backdropFilter:
            surfaceOpacity >= 1 ? "none" : "blur(14px) saturate(160%)",
          WebkitBackdropFilter:
            surfaceOpacity >= 1 ? "none" : "blur(14px) saturate(160%)",
          boxShadow: `inset 0 1px 0 rgba(255, 255, 255, 0.1), 0 12px 30px rgba(0, 0, 0, 0.35), 0 0 22px ${recordingOverlayHexToRgba(accent, 0.16)}`,
        };
      case "classic":
      default:
        return {
          background: stackSurfaceLayers(
            `linear-gradient(180deg, ${recordingOverlayHexToRgba(bodyHighlightColor, 0.74 * surfaceOpacity)} 0%, ${recordingOverlayHexToRgba(bodyEdgeColor, 0.84 * surfaceOpacity)} 100%)`,
          ),
          borderRadius: "18px",
          boxShadow: `inset 0 1px 0 rgba(255, 255, 255, 0.05), 0 10px 24px rgba(0, 0, 0, 0.28), 0 0 0 1px ${recordingOverlayHexToRgba(accent, 0.05)}`,
        };
    }
  })();

  const materialTuning: Record<RecordingOverlayMaterialMode, CSSProperties> = {
    liquid_glass: {},
    pearl: {
      background: stackSurfaceLayers(
        `linear-gradient(180deg, rgba(255,255,255,${0.18 * surfaceOpacity}) 0%, rgba(248,243,255,${0.52 * surfaceOpacity}) 28%, ${recordingOverlayHexToRgba(bodyDeepColor, 0.74 * surfaceOpacity)} 100%)`,
      ),
      boxShadow: `inset 0 1px 0 rgba(255,255,255,0.16), inset 0 -10px 24px rgba(255,255,255,0.03), 0 12px 28px rgba(0,0,0,0.24), 0 0 18px rgba(255,255,255,0.08)`,
    },
    velvet_neon: {
      background: stackSurfaceLayers(
        `linear-gradient(180deg, ${recordingOverlayHexToRgba(bodyHighlightColor, 0.84 * surfaceOpacity)} 0%, ${recordingOverlayHexToRgba(bodyEdgeColor, 0.92 * surfaceOpacity)} 100%)`,
      ),
      border: `1px solid ${recordingOverlayHexToRgba(accent, 0.36)}`,
      boxShadow: `inset 0 1px 0 rgba(255,255,255,0.08), 0 12px 30px rgba(0,0,0,0.4), 0 0 30px ${recordingOverlayHexToRgba(accent, 0.22)}`,
    },
    frost: {
      background: stackSurfaceLayers(
        `linear-gradient(180deg, rgba(248,252,255,${0.16 * surfaceOpacity}) 0%, rgba(170,188,212,${0.1 * surfaceOpacity}) 26%, ${recordingOverlayHexToRgba(bodyDeepColor, 0.7 * surfaceOpacity)} 100%)`,
      ),
      border: `1px solid rgba(255,255,255,0.14)`,
      boxShadow: `inset 0 1px 0 rgba(255,255,255,0.18), 0 10px 24px rgba(0,0,0,0.24), 0 0 14px rgba(255,255,255,0.06)`,
      backdropFilter:
        surfaceOpacity >= 1 ? "none" : "blur(18px) saturate(130%)",
      WebkitBackdropFilter:
        surfaceOpacity >= 1 ? "none" : "blur(18px) saturate(130%)",
    },
    candy_chrome: {
      background: stackSurfaceLayers(
        `linear-gradient(180deg, rgba(255,255,255,${0.22 * surfaceOpacity}) 0%, ${recordingOverlayHexToRgba(accent, 0.18 * surfaceOpacity)} 18%, ${recordingOverlayHexToRgba(bodyEdgeColor, 0.8 * surfaceOpacity)} 100%)`,
      ),
      border: `1px solid ${recordingOverlayHexToRgba(accent, 0.3)}`,
      boxShadow: `inset 0 1px 0 rgba(255,255,255,0.18), 0 12px 28px rgba(0,0,0,0.32), 0 0 24px ${recordingOverlayHexToRgba(accent, 0.18)}`,
    },
    // Shadows stay tight: the overlay window only leaves a few pixels around the frame.
    graphite: {
      background: stackSurfaceLayers(
        `linear-gradient(180deg, ${recordingOverlayHexToRgba(bodyHighlightColor, 0.94 * surfaceOpacity)} 0%, ${recordingOverlayHexToRgba(normalizedBodyBackgroundColor, 0.97 * surfaceOpacity)} 55%, ${recordingOverlayHexToRgba(bodyDeepColor, 0.98 * surfaceOpacity)} 100%)`,
      ),
      border: "1px solid rgba(255,255,255,0.09)",
      boxShadow:
        "inset 0 1px 0 rgba(255,255,255,0.07), 0 1px 2px rgba(0,0,0,0.42), 0 3px 8px rgba(0,0,0,0.28)",
      backdropFilter: "none",
      WebkitBackdropFilter: "none",
    },
    obsidian: {
      background: stackSurfaceLayers(
        `linear-gradient(180deg, rgba(255,255,255,${0.05 * surfaceOpacity}) 0%, rgba(255,255,255,0) 48%), linear-gradient(180deg, ${recordingOverlayHexToRgba(bodyDeepColor, 0.98 * surfaceOpacity)} 0%, ${recordingOverlayHexToRgba(bodyEdgeColor, surfaceOpacity)} 100%)`,
      ),
      border: "1px solid rgba(255,255,255,0.07)",
      boxShadow: "inset 0 0 0 1px rgba(0,0,0,0.6), 0 2px 8px rgba(0,0,0,0.5)",
      backdropFilter: "none",
      WebkitBackdropFilter: "none",
    },
    gradient_mesh: {
      background: stackSurfaceLayers(
        `radial-gradient(120% 160% at 0% 0%, ${recordingOverlayHexToRgba(accent, 0.34 * surfaceOpacity)} 0%, rgba(0,0,0,0) 58%), radial-gradient(120% 160% at 100% 100%, ${recordingOverlayHexToRgba(meshSecondaryColor, 0.3 * surfaceOpacity)} 0%, rgba(0,0,0,0) 62%), linear-gradient(180deg, ${recordingOverlayHexToRgba(bodyHighlightColor, 0.92 * surfaceOpacity)} 0%, ${recordingOverlayHexToRgba(bodyDeepColor, 0.97 * surfaceOpacity)} 100%)`,
      ),
      border: "1px solid rgba(255,255,255,0.1)",
      boxShadow: "inset 0 1px 0 rgba(255,255,255,0.08), 0 2px 8px rgba(0,0,0,0.38)",
      backdropFilter: "none",
      WebkitBackdropFilter: "none",
    },
    porcelain: {
      background: stackSurfaceLayers(
        `linear-gradient(180deg, ${recordingOverlayHexToRgba(porcelainTopColor, 0.97 * surfaceOpacity)} 0%, ${recordingOverlayHexToRgba(porcelainBodyColor, 0.97 * surfaceOpacity)} 58%, ${recordingOverlayHexToRgba(porcelainBottomColor, 0.98 * surfaceOpacity)} 100%)`,
        porcelainBodyColor,
      ),
      border: "1px solid rgba(20,22,30,0.1)",
      boxShadow:
        "inset 0 1px 0 rgba(255,255,255,0.95), inset 0 -1px 0 rgba(20,22,30,0.05), 0 1px 2px rgba(0,0,0,0.2), 0 3px 8px rgba(0,0,0,0.16)",
      backdropFilter: "none",
      WebkitBackdropFilter: "none",
    },
    // Volume comes from light: a broad highlight upper left, a soft inner
    // shade lower right, and a hue-tinted shadow under the slab.
    clay: {
      background: stackSurfaceLayers(
        `radial-gradient(140% 130% at 26% 0%, rgba(255,255,255,${0.42 * surfaceOpacity}) 0%, rgba(255,255,255,0) 56%), linear-gradient(180deg, ${recordingOverlayHexToRgba(clayTopColor, 0.98 * surfaceOpacity)} 0%, ${recordingOverlayHexToRgba(clayToneColor, 0.98 * surfaceOpacity)} 52%, ${recordingOverlayHexToRgba(clayBottomColor, surfaceOpacity)} 100%)`,
        clayToneColor,
      ),
      border: "1px solid rgba(255,255,255,0.3)",
      // Half the 36px frame height: a full pill that the animated border can trace.
      borderRadius: "18px",
      boxShadow: `inset 2px 2px 3px rgba(255,255,255,0.55), inset -2px -3px 6px ${recordingOverlayHexToRgba(clayShadeColor, 0.3)}, 0 2px 3px ${recordingOverlayHexToRgba(clayShadeColor, 0.32)}, 0 4px 9px ${recordingOverlayHexToRgba(clayShadeColor, 0.28)}`,
      backdropFilter: "none",
      WebkitBackdropFilter: "none",
    },
    // The 3px wall is a hard offset shadow, so the cap reads as a raised key.
    keycap: {
      background: stackSurfaceLayers(
        `linear-gradient(180deg, rgba(0,0,0,${0.12 * surfaceOpacity}) 0%, rgba(0,0,0,0) 38%, rgba(255,255,255,${0.05 * surfaceOpacity}) 100%), linear-gradient(180deg, ${recordingOverlayHexToRgba(keycapTopColor, 0.98 * surfaceOpacity)} 0%, ${recordingOverlayHexToRgba(normalizedBodyBackgroundColor, surfaceOpacity)} 100%)`,
        normalizedBodyBackgroundColor,
      ),
      border: `1px solid ${recordingOverlayHexToRgba(keycapWallColor, 0.9)}`,
      borderRadius: "11px",
      boxShadow: `inset 0 1px 0 rgba(255,255,255,${keycapIsLight ? 0.7 : 0.14}), inset 0 -2px 0 rgba(0,0,0,0.14), 0 3px 0 ${keycapWallColor}, 0 3px 9px ${recordingOverlayHexToRgba(accent, 0.38)}, 0 5px 6px rgba(0,0,0,0.4)`,
      backdropFilter: "none",
      WebkitBackdropFilter: "none",
    },
  };

  return {
    ...baseStyle,
    ...materialByMode[normalizedMaterialMode],
    ...themedBase,
    ...materialTuning[normalizedMaterialMode],
  };
}

export function getRecordingOverlayBarStyle(
  barStyle: RecordingOverlayBarStyle,
  accentColor: string,
  level: number,
  index: number,
): CSSProperties {
  const accent = normalizeRecordingOverlayColor(accentColor);
  const baseOpacity = Math.max(0.24, Math.min(1, level * 1.7));

  switch (barStyle) {
    case "capsule":
      return {
        background: `linear-gradient(180deg, ${recordingOverlayHexToRgba(accent, 0.98)}, ${recordingOverlayHexToRgba(accent, 0.44)})`,
        borderRadius: "999px",
        opacity: Math.max(0.35, baseOpacity),
      };
    case "glow":
      return {
        background: `linear-gradient(180deg, ${recordingOverlayHexToRgba(accent, 1)}, ${recordingOverlayHexToRgba(accent, 0.42)})`,
        borderRadius: "3px",
        opacity: baseOpacity,
        boxShadow: `0 0 10px ${recordingOverlayHexToRgba(accent, 0.34)}, 0 0 3px ${recordingOverlayHexToRgba(accent, 0.66)}`,
        transform: `translateY(${index % 2 === 0 ? "0" : "0.5px"})`,
      };
    case "prism":
      return {
        background: `linear-gradient(180deg, ${recordingOverlayHexToRgba(accent, 0.98)} 0%, rgba(255,255,255,0.92) 34%, ${recordingOverlayHexToRgba(accent, 0.38)} 100%)`,
        borderRadius: "1px",
        opacity: Math.max(0.4, baseOpacity),
        boxShadow: `inset 0 1px 0 rgba(255,255,255,0.45), 0 0 0 1px ${recordingOverlayHexToRgba(accent, 0.18)}`,
        transform: `skewX(${index % 2 === 0 ? "-5deg" : "5deg"})`,
      };
    case "solid":
    default:
      return {
        background: recordingOverlayHexToRgba(accent, 0.9),
        borderRadius: "2px",
        opacity: baseOpacity,
      };
  }
}

export function getRecordingOverlayErrorStateStyle(
  opacityPercent = 100,
): CSSProperties {
  const surfaceOpacity = Math.max(20, Math.min(100, Math.round(opacityPercent))) / 100;
  const opaqueBaseAlpha = Math.min(1, 0.22 + (surfaceOpacity * 0.78));
  return {
    background: `
      radial-gradient(circle at 18% 18%, rgba(255, 138, 138, ${0.18 * surfaceOpacity}) 0%, rgba(255, 138, 138, 0) 30%),
      linear-gradient(180deg, rgba(78, 10, 14, ${0.92 * surfaceOpacity}) 0%, rgba(28, 4, 7, ${0.98 * surfaceOpacity}) 100%),
      linear-gradient(180deg, rgba(78, 10, 14, ${opaqueBaseAlpha}) 0%, rgba(28, 4, 7, ${opaqueBaseAlpha}) 100%)
    `,
    border: "1px solid rgba(255, 115, 115, 0.3)",
    boxShadow:
      "inset 0 1px 0 rgba(255, 218, 218, 0.1), inset 0 -10px 24px rgba(255, 120, 120, 0.04), 0 14px 32px rgba(0, 0, 0, 0.34), 0 0 24px rgba(255, 94, 94, 0.12)",
  };
}

export function normalizeRecordingOverlayBarStyle(
  value: string | undefined,
): RecordingOverlayBarStyle {
  switch (value) {
    case "aurora":
      return "aurora";
    case "bloom_bounce":
      return "bloom_bounce";
    case "capsule":
      return "capsule";
    case "comet":
      return "comet";
    case "constellation":
      return "constellation";
    case "crown":
      return "crown";
    case "daisy":
      return "daisy";
    case "ember":
      return "ember";
    case "fireflies":
      return "fireflies";
    case "garden_sway":
      return "garden_sway";
    case "glow":
      return "glow";
    case "hologram":
      return "hologram";
    case "helix":
      return "helix";
    case "lotus":
      return "lotus";
    case "matrix":
      return "matrix";
    case "morse":
      return "morse";
    case "needles":
      return "needles";
    case "orbit":
      return "orbit";
    case "petals":
      return "petals";
    case "petal_rain":
      return "petal_rain";
    case "prism":
      return "prism";
    case "pulse_rings":
      return "pulse_rings";
    case "radar":
      return "radar";
    case "retro":
      return "retro";
    case "shards":
      return "shards";
    case "skyline":
      return "skyline";
    case "solid":
      return "solid";
    case "tuner":
      return "tuner";
    case "vinyl":
      return "vinyl";
    case "wave_line":
      return "wave_line";
    case "mirror":
      return "mirror";
    case "dot_matrix":
      return "dot_matrix";
    case "spectrum":
      return "spectrum";
    case "liquid":
      return "liquid";
    case "pillars":
      return "pillars";
    case "orbs":
      return "orbs";
    case "cubes":
      return "cubes";
    case "ridgeline":
      return "ridgeline";
    case "carousel":
      return "carousel";
    case "twist_ribbon":
      return "twist_ribbon";
    default:
      return "solid";
  }
}

export function normalizeLegacyRecordingOverlayBarStyle(
  value: string | undefined,
): RecordingOverlayBarStyle {
  const normalized = normalizeRecordingOverlayBarStyle(value);
  return LEGACY_RECORDING_OVERLAY_BAR_STYLES.includes(normalized)
    ? normalized
    : "solid";
}
