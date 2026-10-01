import type {
  RecordingOverlayAnimatedBorderMode,
  RecordingOverlayAppearanceSettings,
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
  normalizeRecordingOverlayStatusIconStyle,
  type RecordingOverlayStatusIconStyle,
} from "./recordingOverlayAppearance";

export interface RecordingOverlayStyleConfig {
  theme: RecordingOverlayTheme;
  backgroundMode: RecordingOverlayBackgroundMode;
  materialMode: RecordingOverlayMaterialMode;
  centerpieceMode: RecordingOverlayCenterpieceMode;
  animatedBorderMode: RecordingOverlayAnimatedBorderMode;
  surfaceBaseColor: string;
  bodyBackgroundColor: string;
  showStatusIcon: boolean;
  statusIconStyle: RecordingOverlayStatusIconStyle;
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
  showCancelButton: boolean;
  cancelButtonInvisible: boolean;
  widthPx: number;
  statusIconColor: string;
  cancelIconColor: string;
  decapitalizeIndicatorMode: "text" | "custom" | "hidden";
  decapitalizeIndicatorCustomText: string;
  decapitalizeIndicatorFontFamily: string;
  decapitalizeIndicatorFontSizePx: number;
  decapitalizeIndicatorColor: string;
}

export interface RecordingOverlayStylePreset {
  id: string;
  name: string;
  description: string;
  /**
   * Modern and 3D depth presets are listed in their own groups, before the
   * original packs, which have no collection.
   */
  collection?: "modern" | "depth";
  config: Partial<RecordingOverlayStyleConfig>;
}

/**
 * Fields built-in presets never set. Applying a built-in preset keeps the
 * user's values for them; saved user presets restore them.
 */
export const RECORDING_OVERLAY_PRESET_PRESERVED_FIELDS = [
  "showCancelButton",
  "cancelButtonInvisible",
  "widthPx",
  "statusIconColor",
  "cancelIconColor",
  "decapitalizeIndicatorMode",
  "decapitalizeIndicatorCustomText",
  "decapitalizeIndicatorFontFamily",
  "decapitalizeIndicatorFontSizePx",
  "decapitalizeIndicatorColor",
] as const satisfies ReadonlyArray<keyof RecordingOverlayStyleConfig>;

export const RECORDING_OVERLAY_INDICATOR_FONT_FAMILIES = [
  "Segoe UI", "Segoe UI Emoji", "Bahnschrift", "Arial", "Verdana", "Tahoma",
  "Trebuchet MS", "Georgia", "Times New Roman", "Consolas", "Cascadia Mono",
] as const;

const STYLE_CODE_PREFIX = "aivo-overlay:";
const STYLE_CODE_VERSION = 4;
/** The version that added each later field. Older complete codes may omit them. */
const STYLE_FIELD_ADDED_IN_VERSION: Partial<Record<keyof RecordingOverlayStyleConfig, number>> = {
  statusIconStyle: 3,
  cancelButtonInvisible: 4,
};

export const DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG: RecordingOverlayStyleConfig = {
  theme: "classic",
  backgroundMode: "none",
  materialMode: "liquid_glass",
  centerpieceMode: "none",
  animatedBorderMode: "none",
  surfaceBaseColor: "#101216",
  bodyBackgroundColor: "#101216",
  showStatusIcon: true,
  statusIconStyle: "auto",
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
  showCancelButton: true,
  cancelButtonInvisible: false,
  widthPx: 172,
  statusIconColor: "#faa2ca",
  cancelIconColor: "#faa2ca",
  decapitalizeIndicatorMode: "text",
  decapitalizeIndicatorCustomText: "",
  decapitalizeIndicatorFontFamily: "Segoe UI",
  decapitalizeIndicatorFontSizePx: 11,
  decapitalizeIndicatorColor: "#72f29a",
};

/** Shared calm defaults: no ambient layers, no scaling, a gentle silence fade. */
const MODERN_PRESET_BASE: Partial<RecordingOverlayStyleConfig> = {
  theme: "classic",
  backgroundMode: "none",
  centerpieceMode: "none",
  animatedBorderMode: "none",
  showStatusIcon: true,
  barCount: 12,
  barWidthPx: 3,
  showDragGrip: false,
  audioReactiveScale: false,
  audioReactiveScaleMaxPercent: 6,
  voiceSensitivityPercent: 55,
  animationSoftnessPercent: 50,
  depthParallaxPercent: 0,
  opacityPercent: 100,
  silenceFade: true,
  silenceOpacityPercent: 78,
};

export const RECORDING_OVERLAY_STYLE_PRESETS: RecordingOverlayStylePreset[] = [
  {
    id: "modern_graphite",
    name: "Graphite",
    description: "Matte graphite capsule with crisp mirrored bars. Quiet and neutral.",
    collection: "modern",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "graphite",
      surfaceBaseColor: "#141518",
      bodyBackgroundColor: "#16171a",
      barStyle: "mirror",
      barCount: 13,
      accentColor: "#e8eaee",
    },
  },
  {
    id: "modern_obsidian_wave",
    name: "Obsidian Wave",
    description: "Pure black capsule with a smooth, layered voice wave.",
    collection: "modern",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "obsidian",
      surfaceBaseColor: "#050506",
      bodyBackgroundColor: "#0a0a0c",
      barStyle: "wave_line",
      accentColor: "#8fa3ff",
    },
  },
  {
    id: "modern_spectrum",
    name: "Spectrum",
    description: "A slow spectrum edge turns around a black capsule, like a modern voice assistant.",
    collection: "modern",
    config: {
      ...MODERN_PRESET_BASE,
      statusIconStyle: "ring",
      materialMode: "obsidian",
      animatedBorderMode: "spectrum_edge",
      surfaceBaseColor: "#050506",
      bodyBackgroundColor: "#0a0a0c",
      barStyle: "wave_line",
      accentColor: "#a184ff",
    },
  },
  {
    id: "modern_dot_matrix",
    name: "Dot Matrix",
    description: "Monochrome dot-matrix meter on a matte graphite tile.",
    collection: "modern",
    config: {
      ...MODERN_PRESET_BASE,
      statusIconStyle: "dot",
      theme: "minimal",
      materialMode: "graphite",
      surfaceBaseColor: "#0e0e0f",
      bodyBackgroundColor: "#111113",
      barStyle: "dot_matrix",
      barCount: 14,
      accentColor: "#f2f2f2",
    },
  },
  {
    id: "modern_signal_red",
    name: "Signal Red",
    description: "Black dot-matrix meter with a single red signal color.",
    collection: "modern",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "obsidian",
      surfaceBaseColor: "#050506",
      bodyBackgroundColor: "#0a0a0c",
      barStyle: "dot_matrix",
      accentColor: "#ff453a",
    },
  },
  {
    id: "modern_mint_mesh",
    name: "Mint Mesh",
    description: "Soft teal gradient mesh with mirrored bars fading into blue.",
    collection: "modern",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "gradient_mesh",
      surfaceBaseColor: "#0a1213",
      bodyBackgroundColor: "#0e1718",
      barStyle: "mirror",
      barCount: 11,
      accentColor: "#5eead4",
    },
  },
  {
    id: "modern_sunset_mesh",
    name: "Sunset Mesh",
    description: "Warm gradient mesh with a peach-to-gold wave.",
    collection: "modern",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "gradient_mesh",
      surfaceBaseColor: "#140e0d",
      bodyBackgroundColor: "#1a1211",
      barStyle: "wave_line",
      accentColor: "#ff9f6e",
    },
  },
  {
    id: "modern_ice_line",
    name: "Ice Line",
    description: "Cool blue mirrored bars with one light tracing the edge.",
    collection: "modern",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "graphite",
      animatedBorderMode: "traveling_highlight",
      surfaceBaseColor: "#101318",
      bodyBackgroundColor: "#12161c",
      barStyle: "mirror",
      barCount: 13,
      accentColor: "#9ccbff",
      animationSoftnessPercent: 62,
    },
  },
  {
    id: "modern_porcelain",
    name: "Porcelain",
    description: "A light porcelain pill with crisp ink bars. Calm and bright.",
    collection: "modern",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "porcelain",
      surfaceBaseColor: "#ffffff",
      bodyBackgroundColor: "#f4f4f6",
      barStyle: "mirror",
      barCount: 13,
      accentColor: "#1d1d1f",
    },
  },
  {
    id: "modern_paper_wave",
    name: "Paper Wave",
    description: "Warm paper surface with a cobalt voice wave and a thin ring around the icon.",
    collection: "modern",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "porcelain",
      statusIconStyle: "ring",
      surfaceBaseColor: "#fbf8f4",
      bodyBackgroundColor: "#f3ece4",
      barStyle: "wave_line",
      accentColor: "#3b5bff",
    },
  },
  {
    id: "modern_e_ink",
    name: "E-Ink",
    description: "Monochrome dot matrix printed on light paper.",
    collection: "modern",
    config: {
      ...MODERN_PRESET_BASE,
      theme: "minimal",
      materialMode: "porcelain",
      surfaceBaseColor: "#f7f7f5",
      bodyBackgroundColor: "#ecebe6",
      barStyle: "dot_matrix",
      barCount: 14,
      accentColor: "#141414",
    },
  },
  {
    id: "modern_prism_voice",
    name: "Prism Voice",
    description: "Black capsule with glowing bars that fan out across the spectrum.",
    collection: "modern",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "obsidian",
      statusIconStyle: "ring",
      surfaceBaseColor: "#050506",
      bodyBackgroundColor: "#0a0a0c",
      barStyle: "spectrum",
      barCount: 14,
      accentColor: "#ff4d8d",
    },
  },
  {
    id: "modern_daylight_prism",
    name: "Daylight Prism",
    description: "Spectrum bars on a light porcelain pill.",
    collection: "modern",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "porcelain",
      surfaceBaseColor: "#ffffff",
      bodyBackgroundColor: "#f2f2f7",
      barStyle: "spectrum",
      barCount: 14,
      accentColor: "#7c5cff",
    },
  },
  {
    id: "modern_mercury",
    name: "Mercury",
    description: "Liquid chrome drops that swell and merge as you speak.",
    collection: "modern",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "obsidian",
      surfaceBaseColor: "#050506",
      bodyBackgroundColor: "#0b0b0d",
      barStyle: "liquid",
      barCount: 10,
      barWidthPx: 5,
      accentColor: "#d6dbe3",
      animationSoftnessPercent: 62,
    },
  },
  {
    id: "modern_lava_lamp",
    name: "Lava Lamp",
    description: "Warm liquid drops in a glowing gradient mesh, with the icon on a soft tile.",
    collection: "modern",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "gradient_mesh",
      statusIconStyle: "tile",
      surfaceBaseColor: "#100807",
      bodyBackgroundColor: "#160c0a",
      barStyle: "liquid",
      barCount: 9,
      barWidthPx: 6,
      accentColor: "#ff5e3a",
      animationSoftnessPercent: 70,
    },
  },
  {
    id: "modern_aqua_drop",
    name: "Aqua Drop",
    description: "Cyan liquid on matte graphite, framed by a pulsing ring.",
    collection: "modern",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "graphite",
      statusIconStyle: "ring",
      surfaceBaseColor: "#0c1215",
      bodyBackgroundColor: "#0f171b",
      barStyle: "liquid",
      barCount: 11,
      barWidthPx: 4,
      accentColor: "#22d3ee",
    },
  },
  {
    id: "modern_acid_drop",
    name: "Acid Drop",
    description: "Acid-lime liquid on pure black with a softly breathing edge.",
    collection: "modern",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "obsidian",
      animatedBorderMode: "breathing_contour",
      surfaceBaseColor: "#050506",
      bodyBackgroundColor: "#09090a",
      barStyle: "liquid",
      barCount: 10,
      barWidthPx: 5,
      accentColor: "#b4ff39",
    },
  },
  {
    id: "depth_synthwave",
    name: "Synthwave '84",
    description: "Neon pillars on a sunset grid that runs straight at you.",
    collection: "depth",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "obsidian",
      backgroundMode: "horizon_grid",
      animatedBorderMode: "traveling_highlight",
      statusIconStyle: "coin",
      surfaceBaseColor: "#0b0618",
      bodyBackgroundColor: "#120a26",
      barStyle: "pillars",
      barCount: 11,
      barWidthPx: 5,
      accentColor: "#ff3cac",
    },
  },
  {
    id: "depth_miami_drive",
    name: "Miami Drive",
    description: "A cyan wave racing over a chrome-sun horizon.",
    collection: "depth",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "gradient_mesh",
      backgroundMode: "horizon_grid",
      statusIconStyle: "ring",
      surfaceBaseColor: "#06101a",
      bodyBackgroundColor: "#0a1624",
      barStyle: "wave_line",
      barCount: 12,
      barWidthPx: 4,
      accentColor: "#2de2e6",
    },
  },
  {
    id: "depth_hyperdrive",
    name: "Hyperdrive",
    description: "Crystal cubes spinning while the stars streak past at warp speed.",
    collection: "depth",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "obsidian",
      backgroundMode: "starfield_warp",
      statusIconStyle: "orb",
      surfaceBaseColor: "#04060c",
      bodyBackgroundColor: "#070b16",
      barStyle: "cubes",
      barCount: 9,
      barWidthPx: 6,
      accentColor: "#5ad7ff",
    },
  },
  {
    id: "depth_wormhole",
    name: "Wormhole",
    description: "Glass marbles bouncing at the mouth of a tunnel of light.",
    collection: "depth",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "gradient_mesh",
      backgroundMode: "tunnel_rings",
      statusIconStyle: "orb",
      surfaceBaseColor: "#0a0618",
      bodyBackgroundColor: "#0d0720",
      barStyle: "orbs",
      barCount: 9,
      barWidthPx: 6,
      accentColor: "#a66bff",
    },
  },
  {
    id: "depth_neon_tunnel",
    name: "Neon Tunnel",
    description: "Cubes turning inside a rushing tunnel, framed by a spectrum edge.",
    collection: "depth",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "obsidian",
      backgroundMode: "tunnel_rings",
      animatedBorderMode: "spectrum_edge",
      statusIconStyle: "coin",
      surfaceBaseColor: "#060508",
      bodyBackgroundColor: "#0b0910",
      barStyle: "cubes",
      barCount: 10,
      barWidthPx: 5,
      accentColor: "#ff4d8d",
    },
  },
  {
    id: "depth_isometric_city",
    name: "Isometric City",
    description: "A tiny skyline of lit blocks that rises with every word.",
    collection: "depth",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "graphite",
      statusIconStyle: "tile",
      surfaceBaseColor: "#0b1220",
      bodyBackgroundColor: "#0f1724",
      barStyle: "pillars",
      barCount: 14,
      barWidthPx: 6,
      accentColor: "#3ba8ff",
    },
  },
  {
    id: "depth_gyroscope",
    name: "Gyroscope",
    description: "Gimbal rings turning on three axes around a glowing core.",
    collection: "depth",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "obsidian",
      centerpieceMode: "gyroscope",
      statusIconStyle: "ring",
      surfaceBaseColor: "#07070b",
      bodyBackgroundColor: "#0a0a10",
      barStyle: "mirror",
      barCount: 12,
      barWidthPx: 3,
      accentColor: "#c7b6ff",
      depthParallaxPercent: 35,
    },
  },
  {
    id: "depth_holo_globe",
    name: "Holo Globe",
    description: "A wireframe planet turns under a scanning beam, a satellite in orbit.",
    collection: "depth",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "graphite",
      centerpieceMode: "holo_globe",
      statusIconStyle: "dot",
      surfaceBaseColor: "#041009",
      bodyBackgroundColor: "#06120f",
      barStyle: "dot_matrix",
      barCount: 14,
      barWidthPx: 3,
      accentColor: "#35ffb8",
      depthParallaxPercent: 35,
    },
  },
  {
    id: "depth_claymation",
    name: "Claymation",
    description: "Soft lavender clay with squishy marbles. Pleasantly tactile.",
    collection: "depth",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "clay",
      surfaceBaseColor: "#d9cfff",
      bodyBackgroundColor: "#b7a4ff",
      barStyle: "orbs",
      barCount: 9,
      barWidthPx: 6,
      accentColor: "#6a3dff",
    },
  },
  {
    id: "depth_bubblegum_blocks",
    name: "Bubblegum Blocks",
    description: "Pink clay with chunky 3D blocks, like a toy synthesizer.",
    collection: "depth",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "clay",
      surfaceBaseColor: "#ffd9e6",
      bodyBackgroundColor: "#ffb3cf",
      barStyle: "pillars",
      barCount: 10,
      barWidthPx: 6,
      accentColor: "#ff3d7f",
    },
  },
  {
    id: "depth_mechanical_key",
    name: "Mechanical Key",
    description: "A raised keycap with amber backlight spilling out from under it.",
    collection: "depth",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "keycap",
      surfaceBaseColor: "#22252d",
      bodyBackgroundColor: "#2a2e37",
      barStyle: "cubes",
      barCount: 8,
      barWidthPx: 6,
      accentColor: "#ffb22e",
    },
  },
  {
    id: "depth_retro_keyboard",
    name: "Retro Keyboard",
    description: "A beige vintage keycap with orange blocks. Very 1984 office.",
    collection: "depth",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "keycap",
      surfaceBaseColor: "#f1ebdc",
      bodyBackgroundColor: "#e9e2d0",
      barStyle: "pillars",
      barCount: 10,
      barWidthPx: 5,
      accentColor: "#e2572b",
    },
  },
  {
    id: "depth_marble_run",
    name: "Marble Run",
    description: "Glossy marbles hopping across white porcelain.",
    collection: "depth",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "porcelain",
      statusIconStyle: "orb",
      surfaceBaseColor: "#ffffff",
      bodyBackgroundColor: "#f3f1ee",
      barStyle: "orbs",
      barCount: 10,
      barWidthPx: 6,
      accentColor: "#ff5a36",
    },
  },
  {
    id: "depth_pulsar_ridge",
    name: "Pulsar Ridge",
    description: "The last second of your voice as ridgelines fading into the dark.",
    collection: "depth",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "obsidian",
      statusIconStyle: "bare",
      surfaceBaseColor: "#050505",
      bodyBackgroundColor: "#08080a",
      barStyle: "ridgeline",
      barCount: 14,
      barWidthPx: 4,
      accentColor: "#f2f2f2",
    },
  },
  {
    id: "depth_saturn",
    name: "Saturn",
    description: "A golden ringed planet behind a carousel of bars turning in orbit.",
    collection: "depth",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "obsidian",
      centerpieceMode: "ringed_planet",
      statusIconStyle: "coin",
      surfaceBaseColor: "#07060a",
      bodyBackgroundColor: "#0c0a10",
      barStyle: "carousel",
      barCount: 12,
      barWidthPx: 3,
      accentColor: "#ffc46b",
      depthParallaxPercent: 35,
    },
  },
  {
    id: "depth_plasma_core",
    name: "Plasma Core",
    description: "Colored light swirling inside a glass sphere, a thin wave across it.",
    collection: "depth",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "gradient_mesh",
      centerpieceMode: "plasma_orb",
      statusIconStyle: "orb",
      surfaceBaseColor: "#08061a",
      bodyBackgroundColor: "#0c0a22",
      barStyle: "wave_line",
      barCount: 12,
      barWidthPx: 4,
      accentColor: "#8b5cff",
      depthParallaxPercent: 30,
    },
  },
  {
    id: "depth_andromeda",
    name: "Andromeda",
    description: "A twisting ribbon in front of a slowly turning spiral galaxy.",
    collection: "depth",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "obsidian",
      backgroundMode: "galaxy_spiral",
      statusIconStyle: "ring",
      surfaceBaseColor: "#05040c",
      bodyBackgroundColor: "#080614",
      barStyle: "twist_ribbon",
      barCount: 12,
      barWidthPx: 5,
      accentColor: "#b48cff",
    },
  },
  {
    id: "depth_ocean_swell",
    name: "Ocean Swell",
    description: "Marbles riding a swell of glowing dots that rolls toward you.",
    collection: "depth",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "graphite",
      backgroundMode: "dot_swell",
      statusIconStyle: "orb",
      surfaceBaseColor: "#03101a",
      bodyBackgroundColor: "#061722",
      barStyle: "orbs",
      barCount: 9,
      barWidthPx: 6,
      accentColor: "#2de2e6",
    },
  },
  {
    id: "depth_paper_ribbon",
    name: "Paper Ribbon",
    description: "A coral ribbon turning over and over on white porcelain.",
    collection: "depth",
    config: {
      ...MODERN_PRESET_BASE,
      materialMode: "porcelain",
      surfaceBaseColor: "#ffffff",
      bodyBackgroundColor: "#f3f1ee",
      barStyle: "twist_ribbon",
      barCount: 10,
      barWidthPx: 5,
      accentColor: "#ff5a36",
    },
  },
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
      silenceFade: false,
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
      silenceFade: false,
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
      silenceFade: false,
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
      silenceFade: false,
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
      silenceFade: false,
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
      silenceFade: false,
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
      silenceFade: false,
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
      silenceFade: false,
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
      silenceFade: false,
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
      silenceFade: false,
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
      silenceFade: false,
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
      silenceFade: false,
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
      silenceFade: false,
      silenceOpacityPercent: 46,
    },
  },
  {
    id: "starlight_taffy",
    name: "Starlight Taffy",
    description: "Glossy candy-pop energy with extra shine and airy motion.",
    config: {
      theme: "classic",
      backgroundMode: "soft_glow_field",
      showStatusIcon: true,
      barCount: 6,
      barWidthPx: 4,
      barStyle: "fireflies",
      accentColor: "#7fd6ff",
      showDragGrip: false,
      audioReactiveScale: true,
      audioReactiveScaleMaxPercent: 14,
      animationSoftnessPercent: 58,
      opacityPercent: 100,
      silenceFade: false,
      silenceOpacityPercent: 72,
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
      silenceFade: false,
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
      silenceFade: false,
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
      silenceFade: false,
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
      silenceFade: false,
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
      silenceFade: false,
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
      silenceFade: false,
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
  const raw = { ...source } as Record<string, unknown>;
  for (const key of Object.keys(DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG)) {
    const alias = key.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
    if (raw[key] === undefined && raw[alias] !== undefined) raw[key] = raw[alias];
  }
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
    surfaceBaseColor: normalizeRecordingOverlayColor(
      typeof raw.surfaceBaseColor === "string"
        ? raw.surfaceBaseColor
        : typeof raw.surface_base_color === "string"
          ? raw.surface_base_color
          : undefined,
      DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG.surfaceBaseColor,
    ),
    bodyBackgroundColor: normalizeRecordingOverlayColor(
      typeof raw.bodyBackgroundColor === "string"
        ? raw.bodyBackgroundColor
        : typeof raw.body_background_color === "string"
          ? raw.body_background_color
          : undefined,
      DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG.bodyBackgroundColor,
    ),
    showStatusIcon:
      typeof raw.showStatusIcon === "boolean"
        ? raw.showStatusIcon
        : typeof raw.show_status_icon === "boolean"
          ? raw.show_status_icon
        : DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG.showStatusIcon,
    statusIconStyle: normalizeRecordingOverlayStatusIconStyle(
      typeof raw.statusIconStyle === "string" ? raw.statusIconStyle : undefined,
    ),
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
    showCancelButton: typeof raw.showCancelButton === "boolean" ? raw.showCancelButton : true,
    cancelButtonInvisible:
      typeof raw.cancelButtonInvisible === "boolean" ? raw.cancelButtonInvisible : false,
    widthPx: clampInteger(raw.widthPx, 172, 420, 172),
    statusIconColor: normalizeRecordingOverlayColor(raw.statusIconColor as string | undefined, "#faa2ca"),
    cancelIconColor: normalizeRecordingOverlayColor(raw.cancelIconColor as string | undefined, "#faa2ca"),
    decapitalizeIndicatorMode: raw.decapitalizeIndicatorMode === "custom" || raw.decapitalizeIndicatorMode === "hidden"
      ? raw.decapitalizeIndicatorMode : "text",
    decapitalizeIndicatorCustomText: typeof raw.decapitalizeIndicatorCustomText === "string"
      ? Array.from(raw.decapitalizeIndicatorCustomText.replace(/[\r\n]/g, " ").trim()).slice(0, 24).join("") : "",
    decapitalizeIndicatorFontFamily: RECORDING_OVERLAY_INDICATOR_FONT_FAMILIES.find(
      (font) => font === raw.decapitalizeIndicatorFontFamily,
    ) ?? "Segoe UI",
    decapitalizeIndicatorFontSizePx: clampInteger(raw.decapitalizeIndicatorFontSizePx, 6, 48, 11),
    decapitalizeIndicatorColor: normalizeRecordingOverlayColor(raw.decapitalizeIndicatorColor as string | undefined, "#72f29a"),
  };
}

/** Reads the appearance from app settings or from a saved user preset. */
export function getRecordingOverlayStyleConfigFromSettings(
  settings: Partial<RecordingOverlayAppearanceSettings> | null,
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
    surfaceBaseColor:
      (settings as any).recording_overlay_surface_base_color ??
      DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG.surfaceBaseColor,
    bodyBackgroundColor:
      (settings as any).recording_overlay_body_background_color ??
      DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG.bodyBackgroundColor,
    showStatusIcon: settings.recording_overlay_show_status_icon,
    statusIconStyle: (settings as any).recording_overlay_status_icon_style,
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
    showCancelButton: settings.recording_overlay_show_cancel_button,
    cancelButtonInvisible: (settings as any).recording_overlay_cancel_button_invisible,
    widthPx: settings.recording_overlay_width_px,
    statusIconColor: settings.recording_overlay_status_icon_color,
    cancelIconColor: settings.recording_overlay_cancel_icon_color,
    decapitalizeIndicatorMode: settings.recording_overlay_decapitalize_indicator_mode,
    decapitalizeIndicatorCustomText: settings.recording_overlay_decapitalize_indicator_custom_text,
    decapitalizeIndicatorFontFamily: settings.recording_overlay_decapitalize_indicator_font_family,
    decapitalizeIndicatorFontSizePx: settings.recording_overlay_decapitalize_indicator_font_size_px,
    decapitalizeIndicatorColor: settings.recording_overlay_decapitalize_indicator_color,
  });
}

/**
 * The look a built-in preset produces: its own fields over the defaults, with
 * the user's values kept for the fields presets never set.
 */
export function resolveRecordingOverlayPresetConfig(
  preset: RecordingOverlayStylePreset,
  current: RecordingOverlayStyleConfig,
): RecordingOverlayStyleConfig {
  const preserved = Object.fromEntries(
    RECORDING_OVERLAY_PRESET_PRESERVED_FIELDS.map((key) => [key, current[key]]),
  );
  return normalizeRecordingOverlayStyleConfig({
    ...DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG,
    ...preserved,
    ...preset.config,
  });
}

export function serializeRecordingOverlayStyleConfig(
  config: RecordingOverlayStyleConfig,
): string {
  const normalized = normalizeRecordingOverlayStyleConfig(config);
  return JSON.stringify(
    {
      version: STYLE_CODE_VERSION,
      style: normalized,
    },
    null,
    2,
  );
}

export function parseRecordingOverlayStyleConfig(
  input: string,
  defaults: RecordingOverlayStyleConfig = DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG,
): RecordingOverlayStyleConfig {
  const trimmed = input.trim();
  if (!trimmed) {
    throw new Error("Style code is empty.");
  }

  let rawPayload = trimmed;
  if (trimmed.startsWith(STYLE_CODE_PREFIX)) {
    try {
      rawPayload = fromBase64Utf8(trimmed.slice(STYLE_CODE_PREFIX.length));
    } catch {
      throw new Error("Style code contains invalid encoded data.");
    }
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(rawPayload);
  } catch {
    throw new Error("Style code is not valid JSON or Aivo overlay code.");
  }

  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error("Style code payload must be an object.");
  }

  const payload = parsed as Record<string, unknown>;
  const wrapped = "style" in payload || "version" in payload;
  if (wrapped && ![1, 2, 3, 4].includes(payload.version as number)) {
    throw new Error("Unsupported overlay style version. Supported versions are 1 to 4.");
  }
  if (wrapped && Object.keys(payload).some((key) => key !== "version" && key !== "style")) {
    throw new Error("Style code contains unknown envelope fields.");
  }
  const source = wrapped ? payload.style : payload;
  if (!source || typeof source !== "object" || Array.isArray(source)) {
    throw new Error("Style must be an object.");
  }
  const fields = Object.keys(DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG);
  const aliases = new Map(fields.map((key) => [
    key.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`), key,
  ]));
  const canonical: Record<string, unknown> = {};
  for (const [name, value] of Object.entries(source)) {
    const key = fields.includes(name) ? name : aliases.get(name);
    if (!key) throw new Error(`Unknown style field: ${name}.`);
    if (key in canonical) throw new Error(`Duplicate style field: ${key}.`);
    canonical[key] = value;
  }
  if (Object.keys(canonical).length === 0) {
    throw new Error("Style must contain at least one appearance field.");
  }
  if (wrapped && typeof payload.version === "number" && payload.version >= 2) {
    const version = payload.version;
    const addedLater = (key: string) =>
      (STYLE_FIELD_ADDED_IN_VERSION[key as keyof RecordingOverlayStyleConfig] ?? 0) > version;
    const missing = fields.filter((key) => !(key in canonical) && !addedLater(key));
    if (missing.length) throw new Error(`Missing style fields: ${missing.join(", ")}.`);
    // A complete code from before a field existed describes a look without
    // it; the default reproduces how that look rendered (an Auto icon frame,
    // a visible cancel button).
    for (const key of fields) {
      if (!(key in canonical) && addedLater(key)) {
        canonical[key] =
          DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG[key as keyof RecordingOverlayStyleConfig];
      }
    }
  }
  const normalized = normalizeRecordingOverlayStyleConfig({ ...defaults, ...canonical });
  for (const [key, value] of Object.entries(canonical)) {
    const expected = normalized[key as keyof RecordingOverlayStyleConfig];
    if (typeof value !== typeof expected) {
      throw new Error(`Invalid ${key}: expected ${typeof expected}.`);
    }
    if (typeof value === "number" && (!Number.isInteger(value) || value !== expected)) {
      throw new Error(`Invalid ${key}: use a whole number within the supported range.`);
    }
    if (typeof value === "string" && key.endsWith("Color")) {
      if (!/^#[0-9a-f]{6}$/i.test(value.trim())) {
        throw new Error(`Invalid ${key}: use a six-digit hex color such as #ff4d8d.`);
      }
    } else if (typeof value === "string" && value !== expected) {
      throw new Error(`Invalid ${key}: unsupported or malformed value.`);
    }
  }
  return normalized;
}

export const RECORDING_OVERLAY_STYLE_SETTING_ENTRIES = (
  config: RecordingOverlayStyleConfig,
): Array<[string, unknown]> => {
  const normalized = normalizeRecordingOverlayStyleConfig(config);
  return [
    ["recording_overlay_theme", normalized.theme],
    ["recording_overlay_show_cancel_button", normalized.showCancelButton],
    ["recording_overlay_cancel_button_invisible", normalized.cancelButtonInvisible],
    ["recording_overlay_width_px", normalized.widthPx],
    ["recording_overlay_status_icon_color", normalized.statusIconColor],
    ["recording_overlay_cancel_icon_color", normalized.cancelIconColor],
    ["recording_overlay_decapitalize_indicator_mode", normalized.decapitalizeIndicatorMode],
    ["recording_overlay_decapitalize_indicator_custom_text", normalized.decapitalizeIndicatorCustomText],
    ["recording_overlay_decapitalize_indicator_font_family", normalized.decapitalizeIndicatorFontFamily],
    ["recording_overlay_decapitalize_indicator_font_size_px", normalized.decapitalizeIndicatorFontSizePx],
    ["recording_overlay_decapitalize_indicator_color", normalized.decapitalizeIndicatorColor],
    ["recording_overlay_background_mode", normalized.backgroundMode],
    ["recording_overlay_material_mode", normalized.materialMode],
    ["recording_overlay_centerpiece_mode", normalized.centerpieceMode],
    ["recording_overlay_animated_border_mode", normalized.animatedBorderMode],
    ["recording_overlay_surface_base_color", normalized.surfaceBaseColor],
    ["recording_overlay_body_background_color", normalized.bodyBackgroundColor],
    ["recording_overlay_show_status_icon", normalized.showStatusIcon],
    ["recording_overlay_status_icon_style", normalized.statusIconStyle],
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
