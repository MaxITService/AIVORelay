import type { CSSProperties } from "react";

export type RecordingOverlayTheme = "classic" | "minimal" | "glass";
export type RecordingOverlayBarStyle = "solid" | "capsule" | "glow" | "prism";

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

export function getRecordingOverlaySurfaceStyle(
  theme: RecordingOverlayTheme,
  accentColor: string,
  barWidthPx: number,
): CSSProperties {
  const accent = normalizeRecordingOverlayColor(accentColor);
  const baseStyle: CSSProperties = {
    "--recording-overlay-accent": accent,
    "--recording-overlay-accent-soft": recordingOverlayHexToRgba(accent, 0.22),
    "--recording-overlay-accent-border": recordingOverlayHexToRgba(accent, 0.34),
    "--recording-overlay-bar-color": recordingOverlayHexToRgba(accent, 0.9),
    "--recording-overlay-bar-width": `${Math.max(2, Math.min(12, Math.round(barWidthPx)))}px`,
  } as CSSProperties;

  switch (theme) {
    case "minimal":
      return {
        ...baseStyle,
        background: "rgba(15, 15, 15, 0.9)",
        border: `1px solid ${recordingOverlayHexToRgba(accent, 0.18)}`,
        borderRadius: "12px",
        boxShadow: "0 8px 18px rgba(0, 0, 0, 0.22)",
      };
    case "glass":
      return {
        ...baseStyle,
        background: `linear-gradient(180deg, ${recordingOverlayHexToRgba(accent, 0.18)}, rgba(12, 12, 12, 0.72))`,
        border: `1px solid ${recordingOverlayHexToRgba(accent, 0.26)}`,
        borderRadius: "18px",
        backdropFilter: "blur(14px) saturate(160%)",
        WebkitBackdropFilter: "blur(14px) saturate(160%)",
        boxShadow: `0 12px 30px rgba(0, 0, 0, 0.35), 0 0 22px ${recordingOverlayHexToRgba(accent, 0.16)}`,
      };
    case "classic":
    default:
      return {
        ...baseStyle,
        background: "rgba(0, 0, 0, 0.8)",
        borderRadius: "18px",
      };
  }
}

export function normalizeRecordingOverlayBarStyle(
  value: string | undefined,
): RecordingOverlayBarStyle {
  switch (value) {
    case "capsule":
      return "capsule";
    case "glow":
      return "glow";
    case "prism":
      return "prism";
    case "solid":
    default:
      return "solid";
  }
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
