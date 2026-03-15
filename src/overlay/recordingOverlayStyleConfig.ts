import type {
  AppSettings,
  RecordingOverlayAnimatedBorderMode,
  RecordingOverlayBackgroundMode,
  RecordingOverlayBarStyle,
  RecordingOverlayCenterpieceMode,
  RecordingOverlayMaterialMode,
  RecordingOverlayTheme,
} from "@/bindings";
import {
  normalizeRecordingOverlayAnimatedBorderMode,
  normalizeRecordingOverlayBackgroundMode,
  normalizeRecordingOverlayBarStyle,
  normalizeRecordingOverlayCenterpieceMode,
  normalizeRecordingOverlayColor,
  normalizeRecordingOverlayMaterialMode,
} from "./recordingOverlayAppearance";

export interface RecordingOverlayStyleConfig {
  theme: RecordingOverlayTheme;
  backgroundMode: RecordingOverlayBackgroundMode;
  materialMode: RecordingOverlayMaterialMode;
  centerpieceMode: RecordingOverlayCenterpieceMode;
  animatedBorderMode: RecordingOverlayAnimatedBorderMode;
  showStatusIcon: boolean;
  barCount: number;
  barWidthPx: number;
  barStyle: RecordingOverlayBarStyle;
  accentColor: string;
  showDragGrip: boolean;
  audioReactiveScale: boolean;
  audioReactiveScaleMaxPercent: number;
  voiceSensitivityPercent: number;
  animationSoftnessPercent: number;
  depthParallaxPercent: number;
  opacityPercent: number;
  silenceFade: boolean;
  silenceOpacityPercent: number;
}

export interface RecordingOverlayStylePreset {
  id: string;
  name: string;
  description: string;
  config: Partial<RecordingOverlayStyleConfig>;
}

const STYLE_CODE_PREFIX = "aivo-overlay:";

export const DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG: RecordingOverlayStyleConfig = {
  theme: "classic",
  backgroundMode: "none",
  materialMode: "liquid_glass",
  centerpieceMode: "none",
  animatedBorderMode: "none",
  showStatusIcon: true,
  barCount: 9,
  barWidthPx: 6,
  barStyle: "solid",
  accentColor: "#ff4d8d",
  showDragGrip: false,
  audioReactiveScale: false,
  audioReactiveScaleMaxPercent: 12,
  voiceSensitivityPercent: 50,
  animationSoftnessPercent: 55,
  depthParallaxPercent: 40,
  opacityPercent: 100,
  silenceFade: false,
  silenceOpacityPercent: 58,
};

export const RECORDING_OVERLAY_STYLE_PRESETS: RecordingOverlayStylePreset[] = [
  {
    id: "broadcast_glow",
    name: "Broadcast Glow",
    description: "Clean, bright, confident. Looks premium immediately.",
    config: {
      theme: "glass",
      backgroundMode: "soft_glow_field",
      showStatusIcon: true,
      barCount: 11,
      barWidthPx: 5,
      barStyle: "glow",
      accentColor: "#ff5aa5",
      showDragGrip: false,
      audioReactiveScale: true,
      audioReactiveScaleMaxPercent: 8,
      animationSoftnessPercent: 42,
      opacityPercent: 100,
      silenceFade: true,
      silenceOpacityPercent: 74,
    },
  },
  {
    id: "command_center",
    name: "Command Center",
    description: "Sharper, colder, more technical. Feels like expensive software.",
    config: {
      theme: "minimal",
      backgroundMode: "soft_glow_field",
      showStatusIcon: true,
      barCount: 10,
      barWidthPx: 4,
      barStyle: "constellation",
      accentColor: "#4dd6ff",
      showDragGrip: false,
      audioReactiveScale: true,
      audioReactiveScaleMaxPercent: 10,
      animationSoftnessPercent: 58,
      opacityPercent: 100,
      silenceFade: true,
      silenceOpacityPercent: 68,
    },
  },
  {
    id: "neon_dna",
    name: "Neon DNA",
    description: "Organic and futuristic. Very strong identity on stream.",
    config: {
      theme: "glass",
      backgroundMode: "soft_glow_field",
      showStatusIcon: true,
      barCount: 9,
      barWidthPx: 7,
      barStyle: "helix",
      accentColor: "#8d7dff",
      showDragGrip: false,
      audioReactiveScale: true,
      audioReactiveScaleMaxPercent: 14,
      animationSoftnessPercent: 36,
      opacityPercent: 100,
      silenceFade: true,
      silenceOpacityPercent: 60,
    },
  },
  {
    id: "quiet_luxury",
    name: "Quiet Luxury",
    description: "Soft, restrained, expensive-looking, never noisy.",
    config: {
      theme: "minimal",
      backgroundMode: "mist",
      showStatusIcon: false,
      barCount: 8,
      barWidthPx: 5,
      barStyle: "pulse_rings",
      accentColor: "#d6b37a",
      showDragGrip: false,
      audioReactiveScale: false,
      audioReactiveScaleMaxPercent: 8,
      animationSoftnessPercent: 82,
      opacityPercent: 92,
      silenceFade: true,
      silenceOpacityPercent: 42,
    },
  },
  {
    id: "firefly_stage",
    name: "Firefly Stage",
    description: "Lighter, playful, memorable. Great when you want a signature look.",
    config: {
      theme: "classic",
      backgroundMode: "soft_glow_field",
      showStatusIcon: true,
      barCount: 12,
      barWidthPx: 4,
      barStyle: "fireflies",
      accentColor: "#4dffb8",
      showDragGrip: false,
      audioReactiveScale: true,
      audioReactiveScaleMaxPercent: 16,
      animationSoftnessPercent: 34,
      opacityPercent: 100,
      silenceFade: true,
      silenceOpacityPercent: 52,
    },
  },
  {
    id: "petal_flux",
    name: "Petal Flux",
    description: "More artistic and unmistakable. Looks custom, not template-made.",
    config: {
      theme: "glass",
      backgroundMode: "petals_haze",
      showStatusIcon: false,
      barCount: 7,
      barWidthPx: 8,
      barStyle: "lotus",
      accentColor: "#ff7a5c",
      showDragGrip: false,
      audioReactiveScale: true,
      audioReactiveScaleMaxPercent: 18,
      animationSoftnessPercent: 60,
      opacityPercent: 100,
      silenceFade: false,
      silenceOpacityPercent: 58,
    },
  },
  {
    id: "sakura_bloom",
    name: "Sakura Bloom",
    description: "Soft pink petals with a dreamy blossom vibe.",
    config: {
      theme: "glass",
      backgroundMode: "petals_haze",
      showStatusIcon: false,
      barCount: 8,
      barWidthPx: 8,
      barStyle: "bloom_bounce",
      accentColor: "#ff9bc7",
      showDragGrip: false,
      audioReactiveScale: true,
      audioReactiveScaleMaxPercent: 12,
      animationSoftnessPercent: 76,
      opacityPercent: 96,
      silenceFade: true,
      silenceOpacityPercent: 66,
    },
  },
  {
    id: "rose_mist",
    name: "Rose Mist",
    description: "Rosy glow, lighter motion, very soft and elegant.",
    config: {
      theme: "glass",
      backgroundMode: "mist",
      showStatusIcon: false,
      barCount: 10,
      barWidthPx: 5,
      barStyle: "petal_rain",
      accentColor: "#ff7fb8",
      showDragGrip: false,
      audioReactiveScale: true,
      audioReactiveScaleMaxPercent: 10,
      animationSoftnessPercent: 84,
      opacityPercent: 92,
      silenceFade: true,
      silenceOpacityPercent: 72,
    },
  },
  {
    id: "peony_pop",
    name: "Peony Pop",
    description: "Bigger pink floral energy with a stronger stage presence.",
    config: {
      theme: "classic",
      backgroundMode: "petals_haze",
      showStatusIcon: true,
      barCount: 7,
      barWidthPx: 9,
      barStyle: "daisy",
      accentColor: "#ff5ea8",
      showDragGrip: false,
      audioReactiveScale: true,
      audioReactiveScaleMaxPercent: 18,
      animationSoftnessPercent: 48,
      opacityPercent: 100,
      silenceFade: false,
      silenceOpacityPercent: 58,
    },
  },
  {
    id: "pink_camellia",
    name: "Pink Camellia",
    description: "More polished and expensive, like floral luxury branding.",
    config: {
      theme: "minimal",
      backgroundMode: "mist",
      showStatusIcon: false,
      barCount: 9,
      barWidthPx: 6,
      barStyle: "garden_sway",
      accentColor: "#f28cc0",
      showDragGrip: false,
      audioReactiveScale: false,
      audioReactiveScaleMaxPercent: 8,
      animationSoftnessPercent: 88,
      opacityPercent: 88,
      silenceFade: true,
      silenceOpacityPercent: 44,
    },
  },
  {
    id: "velvet_mist",
    name: "Velvet Mist",
    description: "Soft haze, premium glow, and calmer motion for a luxe presence.",
    config: {
      theme: "glass",
      backgroundMode: "mist",
      showStatusIcon: false,
      barCount: 9,
      barWidthPx: 6,
      barStyle: "aurora",
      accentColor: "#f3a6c7",
      showDragGrip: false,
      audioReactiveScale: true,
      audioReactiveScaleMaxPercent: 8,
      animationSoftnessPercent: 90,
      opacityPercent: 90,
      silenceFade: true,
      silenceOpacityPercent: 48,
    },
  },
  {
    id: "rose_nebula",
    name: "Rose Nebula",
    description: "Dreamier, brighter, and more cinematic with bloom-heavy ambience.",
    config: {
      theme: "glass",
      backgroundMode: "soft_glow_field",
      showStatusIcon: true,
      barCount: 10,
      barWidthPx: 5,
      barStyle: "hologram",
      accentColor: "#ff78b9",
      showDragGrip: false,
      audioReactiveScale: true,
      audioReactiveScaleMaxPercent: 14,
      animationSoftnessPercent: 72,
      opacityPercent: 96,
      silenceFade: true,
      silenceOpacityPercent: 58,
    },
  },
  {
    id: "moonlit_orchid",
    name: "Moonlit Orchid",
    description: "Glossy orchid tones with a calmer, floating studio feel.",
    config: {
      theme: "glass",
      backgroundMode: "mist",
      showStatusIcon: true,
      barCount: 9,
      barWidthPx: 6,
      barStyle: "lotus",
      accentColor: "#c49bff",
      showDragGrip: false,
      audioReactiveScale: true,
      audioReactiveScaleMaxPercent: 10,
      animationSoftnessPercent: 86,
      opacityPercent: 94,
      silenceFade: true,
      silenceOpacityPercent: 56,
    },
  },
  {
    id: "candy_halo",
    name: "Candy Halo",
    description: "Brighter and sweeter, with glossy motion and playful glow.",
    config: {
      theme: "classic",
      backgroundMode: "soft_glow_field",
      showStatusIcon: true,
      barCount: 11,
      barWidthPx: 5,
      barStyle: "aurora",
      accentColor: "#ff71b5",
      showDragGrip: false,
      audioReactiveScale: true,
      audioReactiveScaleMaxPercent: 12,
      animationSoftnessPercent: 64,
      opacityPercent: 100,
      silenceFade: true,
      silenceOpacityPercent: 62,
    },
  },
  {
    id: "champagne_drift",
    name: "Champagne Drift",
    description: "Soft gold glow with restrained motion and a polished finish.",
    config: {
      theme: "glass",
      backgroundMode: "mist",
      showStatusIcon: false,
      barCount: 8,
      barWidthPx: 5,
      barStyle: "pulse_rings",
      accentColor: "#e5c07b",
      showDragGrip: false,
      audioReactiveScale: false,
      audioReactiveScaleMaxPercent: 6,
      animationSoftnessPercent: 92,
      opacityPercent: 90,
      silenceFade: true,
      silenceOpacityPercent: 46,
    },
  },
  {
    id: "starlight_taffy",
    name: "Starlight Taffy",
    description: "Glossy candy-pop energy with extra shine and airy motion.",
    config: {
      theme: "glass",
      backgroundMode: "soft_glow_field",
      showStatusIcon: true,
      barCount: 6,
      barWidthPx: 4,
      barStyle: "fireflies",
      accentColor: "#ff8fd2",
      showDragGrip: false,
      audioReactiveScale: true,
      audioReactiveScaleMaxPercent: 14,
      animationSoftnessPercent: 58,
      opacityPercent: 100,
      silenceFade: true,
      silenceOpacityPercent: 60,
    },
  },
  {
    id: "liquid_crown",
    name: "Liquid Crown",
    description: "Glass hero preset with a regal signal crown and premium edge shimmer.",
    config: {
      theme: "glass",
      backgroundMode: "stardust",
      materialMode: "liquid_glass",
      centerpieceMode: "signal_crown",
      animatedBorderMode: "traveling_highlight",
      showStatusIcon: true,
      barCount: 11,
      barWidthPx: 5,
      barStyle: "aurora",
      accentColor: "#ff6fae",
      showDragGrip: false,
      audioReactiveScale: true,
      audioReactiveScaleMaxPercent: 12,
      animationSoftnessPercent: 68,
      depthParallaxPercent: 58,
      opacityPercent: 98,
      silenceFade: true,
      silenceOpacityPercent: 60,
    },
  },
  {
    id: "pearl_bloom",
    name: "Pearl Bloom",
    description: "Soft pearl surface, floral core, and floating haze for luxury branding energy.",
    config: {
      theme: "glass",
      backgroundMode: "silk_fog",
      materialMode: "pearl",
      centerpieceMode: "bloom_heart",
      animatedBorderMode: "breathing_contour",
      showStatusIcon: false,
      barCount: 8,
      barWidthPx: 7,
      barStyle: "lotus",
      accentColor: "#f6a9cb",
      showDragGrip: false,
      audioReactiveScale: true,
      audioReactiveScaleMaxPercent: 10,
      animationSoftnessPercent: 88,
      depthParallaxPercent: 46,
      opacityPercent: 92,
      silenceFade: true,
      silenceOpacityPercent: 48,
    },
  },
  {
    id: "velvet_ribbon",
    name: "Velvet Ribbon",
    description: "Dark couture neon with a rich aurora ribbon moving through the center.",
    config: {
      theme: "classic",
      backgroundMode: "rose_sparks",
      materialMode: "velvet_neon",
      centerpieceMode: "aurora_ribbon",
      animatedBorderMode: "shimmer_edge",
      showStatusIcon: true,
      barCount: 10,
      barWidthPx: 5,
      barStyle: "hologram",
      accentColor: "#d86dff",
      showDragGrip: false,
      audioReactiveScale: true,
      audioReactiveScaleMaxPercent: 14,
      animationSoftnessPercent: 56,
      depthParallaxPercent: 66,
      opacityPercent: 100,
      silenceFade: true,
      silenceOpacityPercent: 64,
    },
  },
  {
    id: "frost_orbit",
    name: "Frost Orbit",
    description: "Cold premium material with orbital beads and a restrained sci-fi calm.",
    config: {
      theme: "minimal",
      backgroundMode: "stardust",
      materialMode: "frost",
      centerpieceMode: "orbital_beads",
      animatedBorderMode: "traveling_highlight",
      showStatusIcon: true,
      barCount: 9,
      barWidthPx: 4,
      barStyle: "orbit",
      accentColor: "#8ed8ff",
      showDragGrip: false,
      audioReactiveScale: false,
      audioReactiveScaleMaxPercent: 8,
      animationSoftnessPercent: 78,
      depthParallaxPercent: 72,
      opacityPercent: 90,
      silenceFade: true,
      silenceOpacityPercent: 44,
    },
  },
  {
    id: "candy_supernova",
    name: "Candy Supernova",
    description: "Glossy chrome pop with firefly depth and a high-energy hero silhouette.",
    config: {
      theme: "glass",
      backgroundMode: "firefly_veil",
      materialMode: "candy_chrome",
      centerpieceMode: "halo_core",
      animatedBorderMode: "breathing_contour",
      showStatusIcon: true,
      barCount: 6,
      barWidthPx: 4,
      barStyle: "fireflies",
      accentColor: "#ff78be",
      showDragGrip: false,
      audioReactiveScale: true,
      audioReactiveScaleMaxPercent: 16,
      animationSoftnessPercent: 52,
      depthParallaxPercent: 64,
      opacityPercent: 100,
      silenceFade: true,
      silenceOpacityPercent: 58,
    },
  },
  {
    id: "ember_fault",
    name: "Ember Fault",
    description: "A dramatic signal-forward preset that makes the eventual error state feel intentional too.",
    config: {
      theme: "classic",
      backgroundMode: "rose_sparks",
      materialMode: "velvet_neon",
      centerpieceMode: "signal_crown",
      animatedBorderMode: "traveling_highlight",
      showStatusIcon: true,
      barCount: 10,
      barWidthPx: 5,
      barStyle: "ember",
      accentColor: "#ff8a6b",
      showDragGrip: false,
      audioReactiveScale: true,
      audioReactiveScaleMaxPercent: 10,
      animationSoftnessPercent: 48,
      depthParallaxPercent: 54,
      opacityPercent: 98,
      silenceFade: true,
      silenceOpacityPercent: 62,
    },
  },
];

function clampInteger(value: unknown, min: number, max: number, fallback: number): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return fallback;
  }
  return Math.max(min, Math.min(max, Math.round(value)));
}

function normalizeTheme(value: unknown): RecordingOverlayTheme {
  if (value === "minimal" || value === "glass") {
    return value;
  }
  return "classic";
}

function fromBase64Utf8(value: string): string {
  const binary = atob(value);
  const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
  return new TextDecoder().decode(bytes);
}

export function normalizeRecordingOverlayStyleConfig(
  value: Partial<RecordingOverlayStyleConfig> | Record<string, unknown> | null | undefined,
): RecordingOverlayStyleConfig {
  const source = value ?? {};
  const raw = source as Record<string, unknown>;
  return {
    theme: normalizeTheme(raw.theme),
    backgroundMode: normalizeRecordingOverlayBackgroundMode(
      typeof raw.backgroundMode === "string"
        ? raw.backgroundMode
        : typeof raw.background_mode === "string"
          ? raw.background_mode
          : undefined,
    ),
    materialMode: normalizeRecordingOverlayMaterialMode(
      typeof raw.materialMode === "string"
        ? raw.materialMode
        : typeof raw.material_mode === "string"
          ? raw.material_mode
          : undefined,
    ),
    centerpieceMode: normalizeRecordingOverlayCenterpieceMode(
      typeof raw.centerpieceMode === "string"
        ? raw.centerpieceMode
        : typeof raw.centerpiece_mode === "string"
          ? raw.centerpiece_mode
          : undefined,
    ),
    animatedBorderMode: normalizeRecordingOverlayAnimatedBorderMode(
      typeof raw.animatedBorderMode === "string"
        ? raw.animatedBorderMode
        : typeof raw.animated_border_mode === "string"
          ? raw.animated_border_mode
          : undefined,
    ),
    showStatusIcon:
      typeof raw.showStatusIcon === "boolean"
        ? raw.showStatusIcon
        : typeof raw.show_status_icon === "boolean"
          ? raw.show_status_icon
        : DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG.showStatusIcon,
    barCount: clampInteger(
      raw.barCount ?? raw.bar_count,
      3,
      16,
      DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG.barCount,
    ),
    barWidthPx: clampInteger(
      raw.barWidthPx ?? raw.bar_width_px,
      2,
      12,
      DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG.barWidthPx,
    ),
    barStyle: normalizeRecordingOverlayBarStyle(
      typeof raw.barStyle === "string"
        ? raw.barStyle
        : typeof raw.bar_style === "string"
          ? raw.bar_style
          : undefined,
    ),
    accentColor: normalizeRecordingOverlayColor(
      typeof raw.accentColor === "string"
        ? raw.accentColor
        : typeof raw.accent_color === "string"
          ? raw.accent_color
          : undefined,
      DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG.accentColor,
    ),
    showDragGrip:
      typeof raw.showDragGrip === "boolean"
        ? raw.showDragGrip
        : typeof raw.show_drag_grip === "boolean"
          ? raw.show_drag_grip
        : DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG.showDragGrip,
    audioReactiveScale:
      typeof raw.audioReactiveScale === "boolean"
        ? raw.audioReactiveScale
        : typeof raw.audio_reactive_scale === "boolean"
          ? raw.audio_reactive_scale
        : DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG.audioReactiveScale,
    audioReactiveScaleMaxPercent: clampInteger(
      raw.audioReactiveScaleMaxPercent ?? raw.audio_reactive_scale_max_percent,
      0,
      24,
      DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG.audioReactiveScaleMaxPercent,
    ),
    voiceSensitivityPercent: clampInteger(
      raw.voiceSensitivityPercent ?? raw.voice_sensitivity_percent,
      0,
      100,
      DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG.voiceSensitivityPercent,
    ),
    animationSoftnessPercent: clampInteger(
      raw.animationSoftnessPercent ?? raw.animation_softness_percent,
      0,
      100,
      DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG.animationSoftnessPercent,
    ),
    depthParallaxPercent: clampInteger(
      raw.depthParallaxPercent ?? raw.depth_parallax_percent,
      0,
      100,
      DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG.depthParallaxPercent,
    ),
    opacityPercent: clampInteger(
      raw.opacityPercent ?? raw.opacity_percent,
      20,
      100,
      DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG.opacityPercent,
    ),
    silenceFade:
      typeof raw.silenceFade === "boolean"
        ? raw.silenceFade
        : typeof raw.silence_fade === "boolean"
          ? raw.silence_fade
        : DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG.silenceFade,
    silenceOpacityPercent: clampInteger(
      raw.silenceOpacityPercent ?? raw.silence_opacity_percent,
      20,
      100,
      DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG.silenceOpacityPercent,
    ),
  };
}

export function getRecordingOverlayStyleConfigFromSettings(
  settings: AppSettings | null,
): RecordingOverlayStyleConfig {
  if (!settings) {
    return DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG;
  }

  return normalizeRecordingOverlayStyleConfig({
    theme: settings.recording_overlay_theme,
    backgroundMode: settings.recording_overlay_background_mode,
    materialMode: settings.recording_overlay_material_mode,
    centerpieceMode: settings.recording_overlay_centerpiece_mode,
    animatedBorderMode: settings.recording_overlay_animated_border_mode,
    showStatusIcon: settings.recording_overlay_show_status_icon,
    barCount: settings.recording_overlay_bar_count,
    barWidthPx: settings.recording_overlay_bar_width_px,
    barStyle: settings.recording_overlay_bar_style,
    accentColor: settings.recording_overlay_accent_color,
    showDragGrip: settings.recording_overlay_show_drag_grip,
    audioReactiveScale: settings.recording_overlay_audio_reactive_scale,
    audioReactiveScaleMaxPercent:
      settings.recording_overlay_audio_reactive_scale_max_percent,
    voiceSensitivityPercent:
      settings.recording_overlay_voice_sensitivity_percent,
    animationSoftnessPercent:
      settings.recording_overlay_animation_softness_percent,
    depthParallaxPercent: settings.recording_overlay_depth_parallax_percent,
    opacityPercent: settings.recording_overlay_opacity_percent,
    silenceFade: settings.recording_overlay_silence_fade,
    silenceOpacityPercent: settings.recording_overlay_silence_opacity_percent,
  });
}

export function serializeRecordingOverlayStyleConfig(
  config: RecordingOverlayStyleConfig,
): string {
  const normalized = normalizeRecordingOverlayStyleConfig(config);
  return JSON.stringify(
    {
      version: 1,
      style: normalized,
    },
    null,
    2,
  );
}

export function parseRecordingOverlayStyleConfig(
  input: string,
): RecordingOverlayStyleConfig {
  const trimmed = input.trim();
  if (!trimmed) {
    throw new Error("Style code is empty.");
  }

  let rawPayload = trimmed;
  if (trimmed.startsWith(STYLE_CODE_PREFIX)) {
    rawPayload = fromBase64Utf8(trimmed.slice(STYLE_CODE_PREFIX.length));
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(rawPayload);
  } catch {
    throw new Error("Style code is not valid JSON or Aivo overlay code.");
  }

  if (!parsed || typeof parsed !== "object") {
    throw new Error("Style code payload is invalid.");
  }

  const parsedRecord = parsed as Record<string, unknown>;
  const style =
    typeof parsedRecord.style === "object" && parsedRecord.style
      ? (parsedRecord.style as Record<string, unknown>)
      : parsedRecord;

  return normalizeRecordingOverlayStyleConfig(style);
}

export const RECORDING_OVERLAY_STYLE_SETTING_ENTRIES = (
  config: RecordingOverlayStyleConfig,
): Array<[keyof AppSettings, AppSettings[keyof AppSettings]]> => {
  const normalized = normalizeRecordingOverlayStyleConfig(config);
  return [
    ["recording_overlay_theme", normalized.theme],
    ["recording_overlay_background_mode", normalized.backgroundMode],
    ["recording_overlay_material_mode", normalized.materialMode],
    ["recording_overlay_centerpiece_mode", normalized.centerpieceMode],
    ["recording_overlay_animated_border_mode", normalized.animatedBorderMode],
    ["recording_overlay_show_status_icon", normalized.showStatusIcon],
    ["recording_overlay_bar_count", normalized.barCount],
    ["recording_overlay_bar_width_px", normalized.barWidthPx],
    ["recording_overlay_bar_style", normalized.barStyle],
    ["recording_overlay_accent_color", normalized.accentColor],
    ["recording_overlay_show_drag_grip", normalized.showDragGrip],
    ["recording_overlay_audio_reactive_scale", normalized.audioReactiveScale],
    [
      "recording_overlay_audio_reactive_scale_max_percent",
      normalized.audioReactiveScaleMaxPercent,
    ],
    [
      "recording_overlay_voice_sensitivity_percent",
      normalized.voiceSensitivityPercent,
    ],
    [
      "recording_overlay_animation_softness_percent",
      normalized.animationSoftnessPercent,
    ],
    [
      "recording_overlay_depth_parallax_percent",
      normalized.depthParallaxPercent,
    ],
    ["recording_overlay_opacity_percent", normalized.opacityPercent],
    ["recording_overlay_silence_fade", normalized.silenceFade],
    [
      "recording_overlay_silence_opacity_percent",
      normalized.silenceOpacityPercent,
    ],
  ];
};
