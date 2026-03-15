import type { CSSProperties } from "react";

export type RecordingOverlayTheme = "classic" | "minimal" | "glass";
export type RecordingOverlayMaterialMode =
  | "liquid_glass"
  | "pearl"
  | "velvet_neon"
  | "frost"
  | "candy_chrome";
export type RecordingOverlayBackgroundMode =
  | "none"
  | "mist"
  | "petals_haze"
  | "soft_glow_field"
  | "stardust"
  | "silk_fog"
  | "firefly_veil"
  | "rose_sparks";
export type RecordingOverlayCenterpieceMode =
  | "none"
  | "halo_core"
  | "aurora_ribbon"
  | "orbital_beads"
  | "bloom_heart"
  | "signal_crown";
export type RecordingOverlayAnimatedBorderMode =
  | "none"
  | "shimmer_edge"
  | "traveling_highlight"
  | "breathing_contour";
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
  | "vinyl";

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
    default:
      return "none";
  }
}

export function getRecordingOverlaySurfaceStyle(
  theme: RecordingOverlayTheme,
  accentColor: string,
  barWidthPx: number,
  opacityPercent = 100,
  materialMode: RecordingOverlayMaterialMode = "liquid_glass",
): CSSProperties {
  const accent = normalizeRecordingOverlayColor(accentColor);
  const surfaceOpacity = Math.max(20, Math.min(100, Math.round(opacityPercent))) / 100;
  const normalizedMaterialMode = normalizeRecordingOverlayMaterialMode(materialMode);
  const baseGlow = recordingOverlayHexToRgba(accent, 0.18);
  const strongGlow = recordingOverlayHexToRgba(accent, 0.32);
  const sheen = recordingOverlayHexToRgba(accent, 0.14);
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
  };

  const themedBase = (() => {
    switch (theme) {
      case "minimal":
        return {
          background: `linear-gradient(180deg, rgba(30, 30, 30, ${0.82 * surfaceOpacity}) 0%, rgba(12, 12, 12, ${0.92 * surfaceOpacity}) 100%)`,
          border: `1px solid ${recordingOverlayHexToRgba(accent, 0.18)}`,
          borderRadius: "12px",
          boxShadow: `inset 0 1px 0 rgba(255, 255, 255, 0.04), 0 8px 18px rgba(0, 0, 0, 0.22), 0 0 0 1px ${recordingOverlayHexToRgba(accent, 0.04)}`,
        };
      case "glass":
        return {
          background: `linear-gradient(180deg, ${recordingOverlayHexToRgba(accent, 0.22 * surfaceOpacity)} 0%, rgba(18, 18, 18, ${0.66 * surfaceOpacity}) 52%, rgba(10, 10, 10, ${0.76 * surfaceOpacity}) 100%)`,
          border: `1px solid ${recordingOverlayHexToRgba(accent, 0.26)}`,
          borderRadius: "18px",
          backdropFilter: "blur(14px) saturate(160%)",
          WebkitBackdropFilter: "blur(14px) saturate(160%)",
          boxShadow: `inset 0 1px 0 rgba(255, 255, 255, 0.1), 0 12px 30px rgba(0, 0, 0, 0.35), 0 0 22px ${recordingOverlayHexToRgba(accent, 0.16)}`,
        };
      case "classic":
      default:
        return {
          background: `linear-gradient(180deg, rgba(20, 20, 20, ${0.74 * surfaceOpacity}) 0%, rgba(0, 0, 0, ${0.84 * surfaceOpacity}) 100%)`,
          borderRadius: "18px",
          boxShadow: `inset 0 1px 0 rgba(255, 255, 255, 0.05), 0 10px 24px rgba(0, 0, 0, 0.28), 0 0 0 1px ${recordingOverlayHexToRgba(accent, 0.05)}`,
        };
    }
  })();

  const materialTuning: Record<RecordingOverlayMaterialMode, CSSProperties> = {
    liquid_glass: {},
    pearl: {
      background: `linear-gradient(180deg, rgba(255,255,255,${0.18 * surfaceOpacity}) 0%, rgba(248,243,255,${0.52 * surfaceOpacity}) 28%, rgba(22,22,24,${0.74 * surfaceOpacity}) 100%)`,
      boxShadow: `inset 0 1px 0 rgba(255,255,255,0.16), inset 0 -10px 24px rgba(255,255,255,0.03), 0 12px 28px rgba(0,0,0,0.24), 0 0 18px rgba(255,255,255,0.08)`,
    },
    velvet_neon: {
      background: `linear-gradient(180deg, rgba(28,18,26,${0.84 * surfaceOpacity}) 0%, rgba(12,8,14,${0.92 * surfaceOpacity}) 100%)`,
      border: `1px solid ${recordingOverlayHexToRgba(accent, 0.36)}`,
      boxShadow: `inset 0 1px 0 rgba(255,255,255,0.08), 0 12px 30px rgba(0,0,0,0.4), 0 0 30px ${recordingOverlayHexToRgba(accent, 0.22)}`,
    },
    frost: {
      background: `linear-gradient(180deg, rgba(248,252,255,${0.16 * surfaceOpacity}) 0%, rgba(170,188,212,${0.1 * surfaceOpacity}) 26%, rgba(16,18,20,${0.7 * surfaceOpacity}) 100%)`,
      border: `1px solid rgba(255,255,255,0.14)`,
      boxShadow: `inset 0 1px 0 rgba(255,255,255,0.18), 0 10px 24px rgba(0,0,0,0.24), 0 0 14px rgba(255,255,255,0.06)`,
      backdropFilter: "blur(18px) saturate(130%)",
      WebkitBackdropFilter: "blur(18px) saturate(130%)",
    },
    candy_chrome: {
      background: `linear-gradient(180deg, rgba(255,255,255,${0.22 * surfaceOpacity}) 0%, ${recordingOverlayHexToRgba(accent, 0.18 * surfaceOpacity)} 18%, rgba(28,16,24,${0.8 * surfaceOpacity}) 100%)`,
      border: `1px solid ${recordingOverlayHexToRgba(accent, 0.3)}`,
      boxShadow: `inset 0 1px 0 rgba(255,255,255,0.18), 0 12px 28px rgba(0,0,0,0.32), 0 0 24px ${recordingOverlayHexToRgba(accent, 0.18)}`,
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
  return {
    background: `
      radial-gradient(circle at 18% 18%, rgba(255, 138, 138, ${0.18 * surfaceOpacity}) 0%, rgba(255, 138, 138, 0) 30%),
      linear-gradient(180deg, rgba(78, 10, 14, ${0.92 * surfaceOpacity}) 0%, rgba(28, 4, 7, ${0.98 * surfaceOpacity}) 100%)
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
