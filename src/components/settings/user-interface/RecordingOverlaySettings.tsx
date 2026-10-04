import React from "react";
import { useTranslation } from "react-i18next";
import { RotateCcw, Trash2, TriangleAlert } from "lucide-react";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { SettingContainer } from "../../ui/SettingContainer";
import { Collapse } from "../../ui/Collapse";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { Dropdown } from "../../ui/Dropdown";
import { ConfirmationModal } from "../../ui/ConfirmationModal";
import { Slider } from "../../ui/Slider";
import { TellMeMore } from "../../ui/TellMeMore";
import { useSettings } from "../../../hooks/useSettings";
import { navigateToSettingsAnchor } from "../../../lib/anchorNavigation";
import { useNavigationStore } from "../../../stores/navigationStore";
import { ShowOverlay } from "../ShowOverlay";
import { RecordingOverlayPreview } from "./RecordingOverlayPreview";
import type {
  RecordingOverlayAnimatedBorderMode,
  RecordingOverlayBackgroundMode,
  RecordingOverlayBarStyle,
  RecordingOverlayCenterpieceMode,
  RecordingOverlayMaterialMode,
  RecordingOverlayTheme,
  RecordingOverlayUserPreset,
} from "@/bindings";
import { commands } from "@/bindings";
import {
  LEGACY_RECORDING_OVERLAY_BAR_STYLES,
  normalizeLegacyRecordingOverlayBarStyle,
  normalizeRecordingOverlayAnimatedBorderMode,
  normalizeRecordingOverlayBackgroundMode,
  normalizeRecordingOverlayBarStyle,
  normalizeRecordingOverlayCenterpieceMode,
  normalizeRecordingOverlayColor,
  normalizeRecordingOverlayMaterialMode,
  normalizeRecordingOverlayStatusIconStyle,
  type RecordingOverlayStatusIconStyle,
} from "../../../overlay/recordingOverlayAppearance";
import {
  DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG,
  getRecordingOverlayStyleConfigFromSettings,
  parseRecordingOverlayStyleConfig,
  RECORDING_OVERLAY_STYLE_PRESETS,
  RECORDING_OVERLAY_INDICATOR_FONT_FAMILIES,
  resolveRecordingOverlayPresetConfig,
  serializeRecordingOverlayStyleConfig,
  type RecordingOverlayStyleConfig,
  type RecordingOverlayStylePreset,
} from "../../../overlay/recordingOverlayStyleConfig";

type PreviewState = "recording" | "silence" | "arming" | "transcribing" | "error";

type OverlaySliderDraftKey =
  | "recording_overlay_bar_count"
  | "recording_overlay_width_px"
  | "recording_overlay_bar_width_px"
  | "recording_overlay_audio_reactive_scale_max_percent"
  | "recording_overlay_voice_sensitivity_percent"
  | "recording_overlay_animation_softness_percent"
  | "recording_overlay_depth_parallax_percent"
  | "recording_overlay_opacity_percent"
  | "recording_overlay_silence_opacity_percent"
  | "recording_overlay_decapitalize_indicator_font_size_px";

type OverlaySliderDrafts = Record<OverlaySliderDraftKey, number>;

const RESET_RECORDING_OVERLAY_STYLE_CONFIG: RecordingOverlayStyleConfig = {
  ...DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG,
};

/** Jump to Text Replacement -> Decapitalize After Manual Edit. */
const openDecapitalizeFeatureSettings = () => {
  navigateToSettingsAnchor({
    activateSection: () =>
      useNavigationStore.getState().setSection("textReplacement"),
    targetId: "decapitalize-after-edit-settings",
    readyId: "settings-section-textReplacement",
    updateHash: false,
  });
};

const CUSTOM_OVERLAY_SETTING_KEYS = new Set([
  "recording_overlay_material_mode", "recording_overlay_background_mode",
  "recording_overlay_centerpiece_mode", "recording_overlay_animated_border_mode",
  "recording_overlay_status_icon_style", "recording_overlay_audio_reactive_scale", "recording_overlay_audio_reactive_scale_max_percent",
  "recording_overlay_voice_sensitivity_percent", "recording_overlay_animation_softness_percent",
  "recording_overlay_depth_parallax_percent", "recording_overlay_opacity_percent",
  "recording_overlay_silence_fade", "recording_overlay_silence_opacity_percent",
]);

const RECORDING_OVERLAY_SETTINGS_COLLAPSED_KEY =
  "aivorelay.userInterface.recordingOverlay.collapsed";
const RECORDING_OVERLAY_PRESETS_COLLAPSED_KEY =
  "aivorelay.userInterface.recordingOverlay.presetsCollapsed";

const THEME_OPTIONS: Array<{
  value: RecordingOverlayTheme;
  label: string;
  labelKey: string;
}> = [
  { value: "classic", label: "Classic", labelKey: "classic" },
  { value: "minimal", label: "Minimal", labelKey: "minimal" },
  { value: "glass", label: "Glass", labelKey: "glass" },
];

const BACKGROUND_MODE_OPTIONS: Array<{
  value: RecordingOverlayBackgroundMode;
  label: string;
  labelKey: string;
}> = [
  { value: "none", label: "None", labelKey: "none" },
  { value: "mist", label: "Mist", labelKey: "mist" },
  { value: "petals_haze", label: "Petals Haze", labelKey: "petalsHaze" },
  {
    value: "soft_glow_field",
    label: "Soft Glow Field",
    labelKey: "softGlowField",
  },
  { value: "stardust", label: "Stardust", labelKey: "stardust" },
  { value: "silk_fog", label: "Silk Fog", labelKey: "silkFog" },
  {
    value: "firefly_veil",
    label: "Firefly Veil",
    labelKey: "fireflyVeil",
  },
  { value: "rose_sparks", label: "Rose Sparks", labelKey: "roseSparks" },
  { value: "horizon_grid", label: "Horizon Grid (3D)", labelKey: "horizonGrid" },
  { value: "starfield_warp", label: "Warp Stars (3D)", labelKey: "starfieldWarp" },
  { value: "tunnel_rings", label: "Tunnel (3D)", labelKey: "tunnelRings" },
  { value: "galaxy_spiral", label: "Spiral Galaxy (3D)", labelKey: "galaxySpiral" },
  { value: "dot_swell", label: "Dot Swell (3D)", labelKey: "dotSwell" },
];

const MATERIAL_MODE_OPTIONS: Array<{
  value: RecordingOverlayMaterialMode;
  label: string;
  labelKey: string;
}> = [
  {
    value: "liquid_glass",
    label: "Liquid Glass",
    labelKey: "liquidGlass",
  },
  { value: "pearl", label: "Pearl", labelKey: "pearl" },
  {
    value: "velvet_neon",
    label: "Velvet Neon",
    labelKey: "velvetNeon",
  },
  { value: "frost", label: "Frost", labelKey: "frost" },
  {
    value: "candy_chrome",
    label: "Candy Chrome",
    labelKey: "candyChrome",
  },
  { value: "graphite", label: "Graphite", labelKey: "graphite" },
  { value: "obsidian", label: "Obsidian", labelKey: "obsidian" },
  {
    value: "gradient_mesh",
    label: "Gradient Mesh",
    labelKey: "gradientMesh",
  },
  { value: "porcelain", label: "Porcelain", labelKey: "porcelain" },
  { value: "clay", label: "Clay (soft 3D)", labelKey: "clay" },
  { value: "keycap", label: "Keycap (3D)", labelKey: "keycap" },
];

const CENTERPIECE_MODE_OPTIONS: Array<{
  value: RecordingOverlayCenterpieceMode;
  label: string;
  labelKey: string;
}> = [
  { value: "none", label: "None", labelKey: "none" },
  { value: "halo_core", label: "Halo Core", labelKey: "haloCore" },
  {
    value: "aurora_ribbon",
    label: "Aurora Ribbon",
    labelKey: "auroraRibbon",
  },
  {
    value: "orbital_beads",
    label: "Orbital Beads",
    labelKey: "orbitalBeads",
  },
  { value: "bloom_heart", label: "Bloom Heart", labelKey: "bloomHeart" },
  {
    value: "signal_crown",
    label: "Signal Crown",
    labelKey: "signalCrown",
  },
  { value: "gyroscope", label: "Gyroscope (3D)", labelKey: "gyroscope" },
  { value: "holo_globe", label: "Holo Globe (3D)", labelKey: "holoGlobe" },
  { value: "ringed_planet", label: "Ringed Planet (3D)", labelKey: "ringedPlanet" },
  { value: "plasma_orb", label: "Plasma Orb (3D)", labelKey: "plasmaOrb" },
];

const STATUS_ICON_STYLE_OPTIONS: Array<{
  value: RecordingOverlayStatusIconStyle;
  label: string;
}> = [
  { value: "auto", label: "Auto (match material)" },
  { value: "capsule", label: "Glass Capsule" },
  { value: "bare", label: "Clean" },
  { value: "ring", label: "Ring" },
  { value: "tile", label: "Tile" },
  { value: "dot", label: "Live Dot" },
  { value: "orb", label: "Glass Orb (3D)" },
  { value: "coin", label: "Flipping Coin (3D)" },
];

const ANIMATED_BORDER_MODE_OPTIONS: Array<{
  value: RecordingOverlayAnimatedBorderMode;
  label: string;
  labelKey: string;
}> = [
  { value: "none", label: "None", labelKey: "none" },
  {
    value: "shimmer_edge",
    label: "Shimmer Edge",
    labelKey: "shimmerEdge",
  },
  {
    value: "traveling_highlight",
    label: "Traveling Highlight",
    labelKey: "travelingHighlight",
  },
  {
    value: "breathing_contour",
    label: "Breathing Contour",
    labelKey: "breathingContour",
  },
  {
    value: "spectrum_edge",
    label: "Spectrum Edge",
    labelKey: "spectrumEdge",
  },
];

const BAR_STYLE_OPTIONS: Array<{
  value: RecordingOverlayBarStyle;
  label: string;
  labelKey: string;
}> = [
  { value: "aurora", label: "Aurora", labelKey: "aurora" },
  {
    value: "bloom_bounce",
    label: "Bloom Bounce",
    labelKey: "bloomBounce",
  },
  { value: "comet", label: "Comet", labelKey: "comet" },
  {
    value: "constellation",
    label: "Constellation",
    labelKey: "constellation",
  },
  { value: "crown", label: "Crown", labelKey: "crown" },
  { value: "daisy", label: "Daisy", labelKey: "daisy" },
  { value: "ember", label: "Ember", labelKey: "ember" },
  { value: "fireflies", label: "Fireflies", labelKey: "fireflies" },
  {
    value: "garden_sway",
    label: "Garden Sway",
    labelKey: "gardenSway",
  },
  { value: "hologram", label: "Hologram", labelKey: "hologram" },
  { value: "helix", label: "Helix", labelKey: "helix" },
  { value: "lotus", label: "Lotus", labelKey: "lotus" },
  { value: "matrix", label: "Matrix Rain", labelKey: "matrix" },
  { value: "morse", label: "Morse", labelKey: "morse" },
  { value: "needles", label: "Needles", labelKey: "needles" },
  { value: "orbit", label: "Orbit", labelKey: "orbit" },
  { value: "petals", label: "Petals", labelKey: "petals" },
  {
    value: "petal_rain",
    label: "Petal Rain",
    labelKey: "petalRain",
  },
  { value: "radar", label: "Radar", labelKey: "radar" },
  {
    value: "pulse_rings",
    label: "Pulse Rings",
    labelKey: "pulseRings",
  },
  {
    value: "retro",
    label: "Equalizer Retro",
    labelKey: "retro",
  },
  { value: "shards", label: "Shards", labelKey: "shards" },
  { value: "skyline", label: "Skyline", labelKey: "skyline" },
  { value: "solid", label: "Solid", labelKey: "solid" },
  { value: "capsule", label: "Capsule", labelKey: "capsule" },
  { value: "glow", label: "Glow", labelKey: "glow" },
  { value: "prism", label: "Prism", labelKey: "prism" },
  { value: "tuner", label: "Tuner", labelKey: "tuner" },
  { value: "vinyl", label: "Vinyl", labelKey: "vinyl" },
  { value: "wave_line", label: "Wave Line", labelKey: "waveLine" },
  { value: "mirror", label: "Mirror", labelKey: "mirror" },
  { value: "dot_matrix", label: "Dot Matrix", labelKey: "dotMatrix" },
  { value: "spectrum", label: "Spectrum", labelKey: "spectrum" },
  { value: "liquid", label: "Liquid", labelKey: "liquid" },
  { value: "pillars", label: "3D Pillars", labelKey: "pillars" },
  { value: "orbs", label: "3D Marbles", labelKey: "orbs" },
  { value: "cubes", label: "3D Spinning Cubes", labelKey: "cubes" },
  { value: "ridgeline", label: "3D Ridgeline", labelKey: "ridgeline" },
  { value: "carousel", label: "3D Carousel", labelKey: "carousel" },
  { value: "twist_ribbon", label: "3D Twisted Ribbon", labelKey: "twistRibbon" },
];

const PREVIEW_STATES: Array<{
  value: PreviewState;
  label: string;
  labelKey: string;
}> = [
  { value: "recording", label: "Recording", labelKey: "recording" },
  { value: "silence", label: "Silence", labelKey: "silence" },
  { value: "arming", label: "Starting capture", labelKey: "arming" },
  {
    value: "transcribing",
    label: "Processing",
    labelKey: "transcribing",
  },
  { value: "error", label: "Error", labelKey: "error" },
];

const DECAPITALIZE_INDICATOR_MODE_OPTIONS = [
  { value: "text", label: "Default Text" },
  { value: "custom", label: "Custom Text / Emoji" },
  { value: "hidden", label: "Hidden" },
] as const;

const PRESETS_TOOLBAR_STICKY_TOP_PX = 12;

function findScrollableAncestor(element: HTMLElement | null): HTMLElement | null {
  let current = element?.parentElement ?? null;
  while (current) {
    const style = window.getComputedStyle(current);
    if (
      style.overflowY === "auto" ||
      style.overflowY === "scroll" ||
      style.overflowY === "overlay"
    ) {
      return current;
    }
    current = current.parentElement;
  }
  return null;
}

/** A titled card that groups related overlay controls. */
const OverlaySettingsSection: React.FC<{
  title: string;
  description: string;
  note?: string | null;
  children: React.ReactNode;
}> = ({ title, description, note, children }) => (
  <section className="mx-3 my-4 rounded-xl border border-white/[0.07] bg-white/[0.02]">
    <div className="px-6 pt-4 pb-1">
      <h3 className="text-xs font-bold uppercase tracking-widest text-[#ff8ebb]">
        {title}
      </h3>
      <p className="mt-1 text-xs leading-relaxed text-[#a0a0a0]">{description}</p>
      {note && <p className="mt-1 text-xs leading-relaxed text-[#ffb6cf]">{note}</p>}
    </div>
    <div className="divide-y divide-white/[0.05]">{children}</div>
  </section>
);

/** Controls that only the custom renderer uses; dimmed in classic mode. */
const CustomOverlayOnly: React.FC<{
  enabled: boolean;
  busy: boolean;
  reason: string;
  children: React.ReactNode;
}> = ({ enabled, busy, reason, children }) => (
  <fieldset
    disabled={!enabled || busy}
    title={enabled ? undefined : reason}
    className={`min-w-0 border-0 p-0 m-0 divide-y divide-white/[0.05]${enabled ? "" : " opacity-45"}`}
  >
    {children}
  </fieldset>
);

/** Compact recording-state preview used by preset cards. */
const StylePresetPreview: React.FC<{ config: RecordingOverlayStyleConfig }> = ({
  config,
}) => (
  <RecordingOverlayPreview
    customEnabled={true}
    theme={config.theme}
    accentColor={config.accentColor}
    surfaceBaseColor={config.surfaceBaseColor}
    bodyBackgroundColor={config.bodyBackgroundColor}
    materialMode={config.materialMode}
    showStatusIcon={config.showStatusIcon}
    statusIconStyle={config.statusIconStyle}
    showCancelButton={config.showCancelButton}
    cancelButtonInvisible={config.cancelButtonInvisible}
    statusIconColor={config.statusIconColor}
    cancelIconColor={config.cancelIconColor}
    minimumWidthPx={config.widthPx}
    backgroundMode={config.backgroundMode}
    centerpieceMode={config.centerpieceMode}
    animatedBorderMode={config.animatedBorderMode}
    barCount={config.barCount}
    barWidthPx={config.barWidthPx}
    barStyle={config.barStyle}
    showDragGrip={config.showDragGrip}
    state="recording"
    audioReactiveScale={config.audioReactiveScale}
    audioReactiveScaleMaxPercent={config.audioReactiveScaleMaxPercent}
    voiceSensitivityPercent={config.voiceSensitivityPercent}
    animationSoftnessPercent={config.animationSoftnessPercent}
    depthParallaxPercent={config.depthParallaxPercent}
    opacityPercent={config.opacityPercent}
    silenceFade={config.silenceFade}
    silenceOpacityPercent={config.silenceOpacityPercent}
    decapIndicatorMode="hidden"
    decapIndicatorFontFamily="Segoe UI"
    decapIndicatorFontSizePx={11}
    decapIndicatorColor="#72f29a"
    maxPreviewWidthPx={248}
  />
);

const MODERN_RECORDING_OVERLAY_PRESETS = RECORDING_OVERLAY_STYLE_PRESETS.filter(
  (preset) => preset.collection === "modern",
);
const DEPTH_RECORDING_OVERLAY_PRESETS = RECORDING_OVERLAY_STYLE_PRESETS.filter(
  (preset) => preset.collection === "depth",
);
const ORIGINAL_RECORDING_OVERLAY_PRESETS = RECORDING_OVERLAY_STYLE_PRESETS.filter(
  (preset) => preset.collection === undefined,
);

export const RecordingOverlaySettings: React.FC = () => {
  const { t } = useTranslation();
  const {
    settings,
    updateSetting: persistSetting,
    applyRecordingOverlayStyle,
    saveRecordingOverlayUserPreset,
    deleteRecordingOverlayUserPreset,
    isUpdating,
    refreshSettings,
  } = useSettings();
  const appearanceOperationRef = React.useRef(false);
  const [showAppInPreview, setShowAppInPreview] = React.useState(false);
  const [previewState, setPreviewState] = React.useState<PreviewState>("recording");
  const [showDecapIndicatorInPreview, setShowDecapIndicatorInPreview] =
    React.useState(false);
  const floatingPreviewAnchorRef = React.useRef<HTMLDivElement | null>(null);
  const floatingPreviewPanelRef = React.useRef<HTMLDivElement | null>(null);
  const floatingPreviewButtonRef = React.useRef<HTMLButtonElement | null>(null);
  const [isCollapsedPreviewOpen, setIsCollapsedPreviewOpen] =
    React.useState(false);
  const [floatingPreviewLayout, setFloatingPreviewLayout] = React.useState({
    dockedVisible: false,
    dockLeft: 0,
    dockTop: 24,
    buttonLeft: 24,
    buttonTop: 24,
    overlayLeft: 24,
    overlayTop: 24,
  });
  const [isResettingAppearance, setIsResettingAppearance] = React.useState(false);
  const [isResettingPosition, setIsResettingPosition] = React.useState(false);
  const [isApplyingPreset, setIsApplyingPreset] = React.useState(false);
  const [isApplyingStyleCode, setIsApplyingStyleCode] = React.useState(false);
  const [arePresetsExpanded, setArePresetsExpanded] = React.useState(true);
  const presetsToolbarRef = React.useRef<HTMLDivElement | null>(null);
  const presetsToolbarSentinelRef = React.useRef<HTMLDivElement | null>(null);
  const [isPresetsToolbarStuck, setIsPresetsToolbarStuck] = React.useState(false);
  const [isRecordingOverlayCollapsed, setIsRecordingOverlayCollapsed] =
    React.useState(false);
  const [styleCodeDraft, setStyleCodeDraft] = React.useState("");
  const [styleToolsStatus, setStyleToolsStatus] = React.useState<string | null>(null);
  const [userPresetName, setUserPresetName] = React.useState("");
  const [pendingDeleteUserPreset, setPendingDeleteUserPreset] =
    React.useState<RecordingOverlayUserPreset | null>(null);

  React.useEffect(() => {
    try {
      if (window.localStorage.getItem(RECORDING_OVERLAY_SETTINGS_COLLAPSED_KEY) === "true") {
        setIsRecordingOverlayCollapsed(true);
      }
      if (window.localStorage.getItem(RECORDING_OVERLAY_PRESETS_COLLAPSED_KEY) === "true") {
        setArePresetsExpanded(false);
      }
    } catch {
      // Keep the section expanded when localStorage is unavailable.
    }
  }, []);

  const togglePresetsExpanded = React.useCallback(() => {
    const expanded = !arePresetsExpanded;
    setArePresetsExpanded(expanded);
    try {
      window.localStorage.setItem(
        RECORDING_OVERLAY_PRESETS_COLLAPSED_KEY,
        expanded ? "false" : "true",
      );
    } catch {
      // UI preference only; ignoring storage errors preserves the toggle behavior.
    }
    if (!expanded) {
      // Collapsing from the floating button shrinks the list under the reader;
      // bring the toolbar back so they stay at the presets.
      window.requestAnimationFrame(() => {
        presetsToolbarRef.current?.scrollIntoView({ block: "nearest" });
      });
    }
  }, [arePresetsExpanded]);

  // While the packs are open the toolbar sticks to the top of the scroll area;
  // the sentinel above it tells us when it is stuck so it can shrink to a pill.
  React.useEffect(() => {
    const sentinel = presetsToolbarSentinelRef.current;
    if (
      !arePresetsExpanded ||
      isRecordingOverlayCollapsed ||
      !sentinel ||
      typeof IntersectionObserver === "undefined"
    ) {
      setIsPresetsToolbarStuck(false);
      return;
    }

    const observer = new IntersectionObserver(
      ([entry]) => {
        const stickyTop = entry.rootBounds?.top ?? 0;
        setIsPresetsToolbarStuck(
          !entry.isIntersecting && entry.boundingClientRect.top < stickyTop,
        );
      },
      {
        root: findScrollableAncestor(sentinel),
        // Matches the toolbar's `top-3` sticky offset.
        rootMargin: `-${PRESETS_TOOLBAR_STICKY_TOP_PX}px 0px 0px 0px`,
      },
    );
    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [arePresetsExpanded, isRecordingOverlayCollapsed]);

  const updateRecordingOverlayCollapsed = React.useCallback((collapsed: boolean) => {
    setIsRecordingOverlayCollapsed(collapsed);
    if (collapsed) {
      // The floating preview is hidden together with the section.
      setIsCollapsedPreviewOpen(false);
    }
    try {
      window.localStorage.setItem(
        RECORDING_OVERLAY_SETTINGS_COLLAPSED_KEY,
        collapsed ? "true" : "false",
      );
    } catch {
      // UI preference only; ignoring storage errors preserves the toggle behavior.
    }
  }, []);

  const overlayTheme =
    ((settings as any)?.recording_overlay_theme ?? "classic") as RecordingOverlayTheme;
  const backgroundMode = normalizeRecordingOverlayBackgroundMode(
    (settings as any)?.recording_overlay_background_mode,
  );
  const materialMode = normalizeRecordingOverlayMaterialMode(
    (settings as any)?.recording_overlay_material_mode,
  );
  const centerpieceMode = normalizeRecordingOverlayCenterpieceMode(
    (settings as any)?.recording_overlay_centerpiece_mode,
  );
  const animatedBorderMode = normalizeRecordingOverlayAnimatedBorderMode(
    (settings as any)?.recording_overlay_animated_border_mode,
  );
  const showStatusIcon = Boolean(
    (settings as any)?.recording_overlay_show_status_icon ?? true,
  );
  const statusIconStyle = normalizeRecordingOverlayStatusIconStyle(
    (settings as any)?.recording_overlay_status_icon_style,
  );
  const showCancelButton = Boolean(
    (settings as any)?.recording_overlay_show_cancel_button ?? true,
  );
  const cancelButtonInvisible =
    (settings as any)?.recording_overlay_cancel_button_invisible === true;
  const rawBarCount = Number((settings as any)?.recording_overlay_bar_count ?? 9);
  const rawBarWidthPx = Number(
    (settings as any)?.recording_overlay_bar_width_px ?? 6,
  );
  const rawOverlayWidthPx = Number(
    (settings as any)?.recording_overlay_width_px ?? 172,
  );
  const barCount = Number.isFinite(rawBarCount) ? rawBarCount : 9;
  const barWidthPx = Number.isFinite(rawBarWidthPx) ? rawBarWidthPx : 6;
  const overlayWidthPx = Number.isFinite(rawOverlayWidthPx) ? rawOverlayWidthPx : 172;
  const clampedOverlayWidthPx = Math.max(
    172,
    Math.min(420, Math.round(overlayWidthPx)),
  );
  const customOverlayEnabled = Boolean(
    (settings as any)?.recording_overlay_custom_enabled ?? false,
  );
  const barStyle = normalizeRecordingOverlayBarStyle(
    (settings as any)?.recording_overlay_bar_style,
  );
  const effectiveBarStyle = customOverlayEnabled
    ? barStyle
    : normalizeLegacyRecordingOverlayBarStyle(barStyle);
  const accentColor = normalizeRecordingOverlayColor(
    (settings as any)?.recording_overlay_accent_color,
  );
  const statusIconColor = normalizeRecordingOverlayColor(
    (settings as any)?.recording_overlay_status_icon_color,
    "#faa2ca",
  );
  const cancelIconColor = normalizeRecordingOverlayColor(
    (settings as any)?.recording_overlay_cancel_icon_color,
    "#faa2ca",
  );
  const surfaceBaseColor = normalizeRecordingOverlayColor(
    (settings as any)?.recording_overlay_surface_base_color,
    "#101216",
  );
  const bodyBackgroundColor = normalizeRecordingOverlayColor(
    (settings as any)?.recording_overlay_body_background_color,
    "#101216",
  );
  const showDragGrip = Boolean(
    (settings as any)?.recording_overlay_show_drag_grip ?? true,
  );
  const audioReactiveScale = Boolean(
    (settings as any)?.recording_overlay_audio_reactive_scale ?? false,
  );
  const audioReactiveScaleMaxPercent = Number(
    (settings as any)?.recording_overlay_audio_reactive_scale_max_percent ?? 12,
  );
  const voiceSensitivityPercent = Number(
    (settings as any)?.recording_overlay_voice_sensitivity_percent ?? 50,
  );
  const animationSoftnessPercent = Number(
    (settings as any)?.recording_overlay_animation_softness_percent ?? 55,
  );
  const depthParallaxPercent = Number(
    (settings as any)?.recording_overlay_depth_parallax_percent ?? 40,
  );
  const opacityPercent = Number(
    (settings as any)?.recording_overlay_opacity_percent ?? 100,
  );
  const silenceFade = Boolean(
    (settings as any)?.recording_overlay_silence_fade ?? false,
  );
  const silenceOpacityPercent = Number(
    (settings as any)?.recording_overlay_silence_opacity_percent ?? 58,
  );
  const decapIndicatorMode = String(
    (settings as any)?.recording_overlay_decapitalize_indicator_mode ?? "text",
  );
  const decapIndicatorCustomText = String(
    (settings as any)?.recording_overlay_decapitalize_indicator_custom_text ?? "",
  );
  const decapIndicatorFontFamily = String(
    (settings as any)?.recording_overlay_decapitalize_indicator_font_family ??
      "Segoe UI",
  );
  const decapIndicatorFontSizePx = Number(
    (settings as any)?.recording_overlay_decapitalize_indicator_font_size_px ?? 11,
  );
  const decapIndicatorColor = normalizeRecordingOverlayColor(
    (settings as any)?.recording_overlay_decapitalize_indicator_color,
    "#72f29a",
  );
  const hasManualPosition = Boolean(
    (settings as any)?.recording_overlay_has_saved_custom_position ??
      (Boolean((settings as any)?.recording_overlay_use_manual_position) ||
        Boolean(
          (settings as any)?.recording_overlay_manual_position_uses_physical_px,
        )),
  );
  const customOverlayDisabledReason = t(
    "settings.userInterface.recordingOverlay.customOverlay.disabledTooltip",
    "Enable Custom Overlay to use these controls.",
  );
  const previewStates = PREVIEW_STATES.map((option) => ({
    value: option.value,
    label: t(
      `settings.userInterface.recordingOverlay.previewStates.${option.labelKey}`,
      option.label,
    ),
  }));
  const themeOptions = THEME_OPTIONS.map((option) => ({
    value: option.value,
    label: t(
      `settings.userInterface.recordingOverlay.theme.options.${option.labelKey}`,
      option.label,
    ),
  }));
  const backgroundModeOptions = BACKGROUND_MODE_OPTIONS.map((option) => ({
    value: option.value,
    label: t(
      `settings.userInterface.recordingOverlay.backgroundMode.options.${option.labelKey}`,
      option.label,
    ),
  }));
  const materialModeOptions = MATERIAL_MODE_OPTIONS.map((option) => ({
    value: option.value,
    label: t(
      `settings.userInterface.recordingOverlay.materialMode.options.${option.labelKey}`,
      option.label,
    ),
  }));
  const centerpieceModeOptions = CENTERPIECE_MODE_OPTIONS.map((option) => ({
    value: option.value,
    label: t(
      `settings.userInterface.recordingOverlay.centerpieceMode.options.${option.labelKey}`,
      option.label,
    ),
  }));
  const statusIconStyleOptions = STATUS_ICON_STYLE_OPTIONS.map((option) => ({
    value: option.value,
    label: t(
      `settings.userInterface.recordingOverlay.statusIconStyle.options.${option.value}`,
      option.label,
    ),
  }));
  const animatedBorderModeOptions = ANIMATED_BORDER_MODE_OPTIONS.map((option) => ({
    value: option.value,
    label: t(
      `settings.userInterface.recordingOverlay.animatedBorderMode.options.${option.labelKey}`,
      option.label,
    ),
  }));
  const barStyleOptions = BAR_STYLE_OPTIONS.map((option) => ({
    value: option.value,
    label: t(
      `settings.userInterface.recordingOverlay.barStyle.options.${option.labelKey}`,
      option.label,
    ),
  }));
  const decapIndicatorModeOptions = DECAPITALIZE_INDICATOR_MODE_OPTIONS.map((option) => ({
    value: option.value,
    label: t(
      `settings.userInterface.recordingOverlay.decapIndicator.mode.options.${option.value}`,
      option.label,
    ),
  }));
  const decapIndicatorFontOptions = RECORDING_OVERLAY_INDICATOR_FONT_FAMILIES.map((fontFamily) => ({
    value: fontFamily,
    label: fontFamily,
  }));
  const currentStyleConfig = React.useMemo(
    () => getRecordingOverlayStyleConfigFromSettings(settings),
    [settings],
  );
  const appearanceBusy = isResettingAppearance || isResettingPosition ||
    isApplyingPreset || isApplyingStyleCode ||
    isUpdating("recording_overlay_appearance") ||
    isUpdating("recording_overlay_custom_enabled");
  // Single-field saves are queued in order by the store, so they neither lock
  // the section nor wait for each other.
  const updateSetting = React.useCallback<typeof persistSetting>(async (key, value) => {
    if (appearanceOperationRef.current || isUpdating("recording_overlay_appearance") ||
      isUpdating("recording_overlay_custom_enabled") ||
      (!customOverlayEnabled && CUSTOM_OVERLAY_SETTING_KEYS.has(String(key)))) return;
    setStyleToolsStatus(null);
    try {
      await persistSetting(key, value, { throwOnError: true });
    } catch (error) {
      setStyleToolsStatus(`Could not save overlay setting: ${String(error)}`);
    }
  }, [persistSetting, isUpdating, customOverlayEnabled]);
  const currentStyleCode = React.useMemo(
    () => serializeRecordingOverlayStyleConfig(currentStyleConfig),
    [currentStyleConfig],
  );
  const appliedPresetId = React.useMemo(() => {
    for (const preset of RECORDING_OVERLAY_STYLE_PRESETS) {
      const presetStyleCode = serializeRecordingOverlayStyleConfig(
        resolveRecordingOverlayPresetConfig(preset, currentStyleConfig),
      );
      if (presetStyleCode === currentStyleCode) {
        return preset.id;
      }
    }

    return null;
  }, [currentStyleCode, currentStyleConfig]);
  const userPresets = React.useMemo(
    () =>
      (settings?.recording_overlay_user_presets ?? []).map((preset) => {
        const config = getRecordingOverlayStyleConfigFromSettings(preset.appearance);
        return {
          preset,
          config,
          isApplied: serializeRecordingOverlayStyleConfig(config) === currentStyleCode,
        };
      }),
    [settings?.recording_overlay_user_presets, currentStyleCode],
  );
  const userPresetsBusy = isUpdating("recording_overlay_user_presets");
  const matchingUserPreset = React.useMemo(() => {
    const name = userPresetName.trim().toLowerCase();
    return name
      ? userPresets.find(({ preset }) => preset.name.toLowerCase() === name)?.preset ?? null
      : null;
  }, [userPresetName, userPresets]);
  const sliderDraftSource = React.useMemo<OverlaySliderDrafts>(
    () => ({
      recording_overlay_bar_count: Math.max(3, Math.min(16, Math.round(barCount))),
      recording_overlay_width_px: clampedOverlayWidthPx,
      recording_overlay_bar_width_px: Math.max(2, Math.min(12, Math.round(barWidthPx))),
      recording_overlay_audio_reactive_scale_max_percent: Math.max(
        0,
        Math.min(24, Math.round(audioReactiveScaleMaxPercent)),
      ),
      recording_overlay_voice_sensitivity_percent: Math.max(
        0,
        Math.min(100, Math.round(voiceSensitivityPercent)),
      ),
      recording_overlay_animation_softness_percent: Math.max(
        0,
        Math.min(100, Math.round(animationSoftnessPercent)),
      ),
      recording_overlay_depth_parallax_percent: Math.max(
        0,
        Math.min(100, Math.round(depthParallaxPercent)),
      ),
      recording_overlay_opacity_percent: Math.max(
        20,
        Math.min(100, Math.round(opacityPercent)),
      ),
      recording_overlay_silence_opacity_percent: Math.max(
        20,
        Math.min(100, Math.round(silenceOpacityPercent)),
      ),
      recording_overlay_decapitalize_indicator_font_size_px: Math.max(
        6,
        Math.min(48, Math.round(decapIndicatorFontSizePx)),
      ),
    }),
    [
      audioReactiveScaleMaxPercent,
      animationSoftnessPercent,
      barCount,
      barWidthPx,
      clampedOverlayWidthPx,
      decapIndicatorFontSizePx,
      depthParallaxPercent,
      opacityPercent,
      silenceOpacityPercent,
      voiceSensitivityPercent,
    ],
  );
  const [sliderDrafts, setSliderDrafts] =
    React.useState<OverlaySliderDrafts>(sliderDraftSource);

  React.useEffect(() => {
    setSliderDrafts(sliderDraftSource);
  }, [sliderDraftSource]);

  React.useEffect(() => {
    const anchorElement = floatingPreviewAnchorRef.current;
    if (!anchorElement) {
      return;
    }

    let frameId = 0;
    const scrollParent = findScrollableAncestor(anchorElement);

    const updateFloatingPreviewLayout = () => {
      frameId = 0;
      const anchorRect = anchorElement.getBoundingClientRect();
      const panelWidth = 360;
      const gap = 28;
      const edgeGap = 20;
      const buttonSize = 52;
      const navigationRight =
        document
          .querySelector<HTMLElement>(".adobe-sidebar")
          ?.getBoundingClientRect().right ?? 0;
      const safeContentLeft = navigationRight + edgeGap;
      const nextDockLeft = Math.round(anchorRect.left - panelWidth - gap);
      const panelHeight = floatingPreviewPanelRef.current?.offsetHeight ?? 0;
      const maxDockTop = Math.max(24, window.innerHeight - panelHeight - 24);
      const nextDockTop = Math.round(
        Math.min(Math.max(anchorRect.top, 24), maxDockTop),
      );
      const nextDockedVisible =
        window.innerWidth >= 1280 && nextDockLeft >= safeContentLeft;
      const gutterButtonLeft = anchorRect.left - buttonSize - 18;
      const preferredButtonLeft =
        gutterButtonLeft >= safeContentLeft
          ? gutterButtonLeft
          : anchorRect.left + 18;
      const maxButtonLeft = window.innerWidth - buttonSize - edgeGap;
      const nextButtonLeft = Math.round(
        maxButtonLeft >= safeContentLeft
          ? Math.min(
              Math.max(preferredButtonLeft, safeContentLeft),
              maxButtonLeft,
            )
          : Math.max(edgeGap, maxButtonLeft),
      );
      const nextButtonTop = Math.round(
        Math.min(
          Math.max(anchorRect.top + 14, 20),
          window.innerHeight - buttonSize - 20,
        ),
      );
      const freeGutterOverlayLeft = anchorRect.left - panelWidth - gap;
      const maxOverlayLeft = window.innerWidth - panelWidth - edgeGap;
      const nextOverlayLeft = Math.round(
        freeGutterOverlayLeft >= safeContentLeft
          ? freeGutterOverlayLeft
          : maxOverlayLeft >= safeContentLeft
            ? Math.min(
                Math.max(anchorRect.left, safeContentLeft),
                maxOverlayLeft,
              )
            : Math.max(edgeGap, maxOverlayLeft),
      );
      const overlayHeight = panelHeight || 320;
      const nextOverlayTop = Math.round(
        Math.min(
          Math.max(nextButtonTop - 12, 20),
          Math.max(20, window.innerHeight - overlayHeight - 20),
        ),
      );

      setFloatingPreviewLayout((current) => {
        if (
          current.dockedVisible === nextDockedVisible &&
          current.dockLeft === nextDockLeft &&
          current.dockTop === nextDockTop &&
          current.buttonLeft === nextButtonLeft &&
          current.buttonTop === nextButtonTop &&
          current.overlayLeft === nextOverlayLeft &&
          current.overlayTop === nextOverlayTop
        ) {
          return current;
        }

        return {
          dockedVisible: nextDockedVisible,
          dockLeft: nextDockLeft,
          dockTop: nextDockTop,
          buttonLeft: nextButtonLeft,
          buttonTop: nextButtonTop,
          overlayLeft: nextOverlayLeft,
          overlayTop: nextOverlayTop,
        };
      });
    };

    const scheduleLayoutUpdate = () => {
      if (frameId !== 0) {
        window.cancelAnimationFrame(frameId);
      }
      frameId = window.requestAnimationFrame(updateFloatingPreviewLayout);
    };

    scheduleLayoutUpdate();
    window.addEventListener("resize", scheduleLayoutUpdate);
    scrollParent?.addEventListener("scroll", scheduleLayoutUpdate, {
      passive: true,
    });

    return () => {
      if (frameId !== 0) {
        window.cancelAnimationFrame(frameId);
      }
      window.removeEventListener("resize", scheduleLayoutUpdate);
      scrollParent?.removeEventListener("scroll", scheduleLayoutUpdate);
    };
  }, [
    previewState,
    showDecapIndicatorInPreview,
    isCollapsedPreviewOpen,
    isRecordingOverlayCollapsed,
  ]);

  React.useEffect(() => {
    if (floatingPreviewLayout.dockedVisible && isCollapsedPreviewOpen) {
      setIsCollapsedPreviewOpen(false);
    }
  }, [floatingPreviewLayout.dockedVisible, isCollapsedPreviewOpen]);

  const updateSliderDraft = React.useCallback(
    (key: OverlaySliderDraftKey, value: number) => {
      setSliderDrafts((current) => ({
        ...current,
        [key]: value,
      }));
    },
    [],
  );

  const commitSliderDraft = React.useCallback(
    async (key: OverlaySliderDraftKey, value: number) => {
      const savedValue = sliderDraftSource[key];
      const restoreDraft = () =>
        setSliderDrafts((current) => ({ ...current, [key]: savedValue }));
      if (appearanceOperationRef.current || isUpdating("recording_overlay_appearance") ||
        (!customOverlayEnabled && CUSTOM_OVERLAY_SETTING_KEYS.has(key))) {
        restoreDraft();
        return;
      }
      const nextValue = Math.round(value);
      // Blur after a drag or key commit reports the value that is already saved.
      if (nextValue === savedValue) return;
      setStyleToolsStatus(null);
      try {
        await persistSetting(key, nextValue, { throwOnError: true });
      } catch (error) {
        restoreDraft();
        setStyleToolsStatus(`Could not save overlay setting: ${String(error)}`);
      }
    },
    [persistSetting, isUpdating, sliderDraftSource, customOverlayEnabled],
  );

  const applyStyleConfig = applyRecordingOverlayStyle;

  const handleResetAppearance = async () => {
    if (appearanceBusy || appearanceOperationRef.current) {
      return;
    }

    appearanceOperationRef.current = true;
    setStyleToolsStatus(null);
    setIsResettingAppearance(true);
    try {
      await applyStyleConfig(RESET_RECORDING_OVERLAY_STYLE_CONFIG);
      setSliderDrafts({
        ...sliderDraftSource,
        recording_overlay_bar_count: RESET_RECORDING_OVERLAY_STYLE_CONFIG.barCount,
        recording_overlay_width_px: 172,
        recording_overlay_bar_width_px: RESET_RECORDING_OVERLAY_STYLE_CONFIG.barWidthPx,
        recording_overlay_audio_reactive_scale_max_percent:
          RESET_RECORDING_OVERLAY_STYLE_CONFIG.audioReactiveScaleMaxPercent,
        recording_overlay_voice_sensitivity_percent:
          RESET_RECORDING_OVERLAY_STYLE_CONFIG.voiceSensitivityPercent,
        recording_overlay_animation_softness_percent:
          RESET_RECORDING_OVERLAY_STYLE_CONFIG.animationSoftnessPercent,
        recording_overlay_depth_parallax_percent:
          RESET_RECORDING_OVERLAY_STYLE_CONFIG.depthParallaxPercent,
        recording_overlay_opacity_percent:
          RESET_RECORDING_OVERLAY_STYLE_CONFIG.opacityPercent,
        recording_overlay_silence_opacity_percent:
          RESET_RECORDING_OVERLAY_STYLE_CONFIG.silenceOpacityPercent,
        recording_overlay_decapitalize_indicator_font_size_px:
          RESET_RECORDING_OVERLAY_STYLE_CONFIG.decapitalizeIndicatorFontSizePx,
      });
      setStyleToolsStatus(
        t(
          "settings.userInterface.recordingOverlay.reset.appearanceDone",
          "Overlay appearance reset to the default look.",
        ),
      );
    } catch (error) {
      console.error("Failed to reset recording overlay appearance:", error);
      setStyleToolsStatus(`Could not reset overlay appearance: ${String(error)}`);
    } finally {
      appearanceOperationRef.current = false;
      setIsResettingAppearance(false);
    }
  };

  const handleResetPosition = async () => {
    if (appearanceBusy || appearanceOperationRef.current) {
      return;
    }

    appearanceOperationRef.current = true;
    setStyleToolsStatus(null);
    setIsResettingPosition(true);
    try {
      const result = await commands.resetRecordingOverlayManualPosition();
      if (result.status === "error") {
        throw new Error(String(result.error));
      }
      await refreshSettings();
      setStyleToolsStatus("Overlay position reset.");
    } catch (error) {
      console.error("Failed to reset recording overlay position:", error);
      setStyleToolsStatus(`Could not reset overlay position: ${String(error)}`);
    } finally {
      appearanceOperationRef.current = false;
      setIsResettingPosition(false);
    }
  };

  const handleApplyPreset = async (config: RecordingOverlayStyleConfig) => {
    if (!customOverlayEnabled || appearanceBusy || appearanceOperationRef.current) {
      return;
    }

    appearanceOperationRef.current = true;
    setStyleToolsStatus(null);
    setIsApplyingPreset(true);
    try {
      await applyStyleConfig(config);
      setStyleToolsStatus(
        t(
          "settings.userInterface.recordingOverlay.presets.applied",
          "Preset applied.",
        ),
      );
    } catch (error) {
      console.error("Failed to apply recording overlay preset:", error);
      setStyleToolsStatus(
        t(
          "settings.userInterface.recordingOverlay.presets.applyError",
          "Could not apply preset.",
        ),
      );
    } finally {
      appearanceOperationRef.current = false;
      setIsApplyingPreset(false);
    }
  };

  const handleSaveUserPreset = async () => {
    const name = userPresetName.trim();
    if (!name || userPresetsBusy || appearanceBusy || appearanceOperationRef.current) {
      return;
    }

    setStyleToolsStatus(null);
    try {
      await saveRecordingOverlayUserPreset(name);
      setUserPresetName("");
      setStyleToolsStatus(
        t(
          "settings.userInterface.recordingOverlay.presets.user.saved",
          "Preset “{{name}}” saved.",
          { name },
        ),
      );
    } catch (error) {
      console.error("Failed to save recording overlay preset:", error);
      setStyleToolsStatus(
        t(
          "settings.userInterface.recordingOverlay.presets.user.saveError",
          "Could not save preset: {{error}}",
          { error: String(error) },
        ),
      );
    }
  };

  const handleDeleteUserPreset = async () => {
    const preset = pendingDeleteUserPreset;
    setPendingDeleteUserPreset(null);
    if (!preset || userPresetsBusy) {
      return;
    }

    setStyleToolsStatus(null);
    try {
      await deleteRecordingOverlayUserPreset(preset.id);
      setStyleToolsStatus(
        t(
          "settings.userInterface.recordingOverlay.presets.user.deleted",
          "Preset “{{name}}” deleted.",
          { name: preset.name },
        ),
      );
    } catch (error) {
      console.error("Failed to delete recording overlay preset:", error);
      setStyleToolsStatus(
        t(
          "settings.userInterface.recordingOverlay.presets.user.deleteError",
          "Could not delete preset: {{error}}",
          { error: String(error) },
        ),
      );
    }
  };

  const renderPresetCardHeader = (
    name: string,
    description: string | null,
    isApplied: boolean,
  ) => (
    <div className="mb-3 flex items-start justify-between gap-3">
      <div className="min-w-0">
        <div className="truncate text-sm font-semibold text-[#f2f2f2]">{name}</div>
        {description && (
          <div className="mt-1 text-xs leading-relaxed text-[#9d9d9d]">
            {description}
          </div>
        )}
      </div>
      <span
        className={`shrink-0 rounded-full px-2 py-0.5 text-[10px] font-medium uppercase tracking-[0.16em] ${
          isApplied
            ? "border border-[#ff92c1]/60 bg-[#ff5fa4]/20 text-[#ffd6e8]"
            : "border border-[#4a4a4a] bg-[#232323] text-[#ff8ebb]"
        }`}
      >
        {isApplied
          ? t(
              "settings.userInterface.recordingOverlay.presets.active",
              "Applied",
            )
          : t(
              "settings.userInterface.recordingOverlay.presets.applyCta",
              "Apply",
            )}
      </span>
    </div>
  );

  const presetCardClassName = (isApplied: boolean) =>
    `w-full rounded-xl border p-3 text-left transition-all duration-200 disabled:cursor-not-allowed disabled:opacity-45 ${
      isApplied
        ? "border-[#ff78b4] bg-[#20161c]"
        : "border-[#323232] bg-[#191919] hover:border-[#4a4a4a] hover:bg-[#202020]"
    }`;

  const presetCardStyle = (isApplied: boolean): React.CSSProperties | undefined =>
    isApplied
      ? {
          background:
            "linear-gradient(180deg, rgba(255,120,180,0.12), rgba(255,120,180,0.04))",
          boxShadow:
            "0 0 0 1px rgba(255,120,180,0.22), 0 18px 38px rgba(0,0,0,0.22)",
        }
      : undefined;

  const renderPresetGroup = (
    title: string,
    presets: RecordingOverlayStylePreset[],
  ) => (
    <div className="space-y-2">
      <div className="text-xs font-semibold uppercase tracking-[0.18em] text-[#ff8ebb]">
        {title}
      </div>
      <div className="grid gap-3 xl:grid-cols-2">
        {presets.map((preset) => {
          const presetConfig = resolveRecordingOverlayPresetConfig(
            preset,
            currentStyleConfig,
          );
          const isApplied = preset.id === appliedPresetId;
          return (
            <button
              key={preset.id}
              type="button"
              onClick={() => void handleApplyPreset(presetConfig)}
              aria-pressed={isApplied}
              disabled={!customOverlayEnabled || appearanceBusy}
              className={presetCardClassName(isApplied)}
              style={presetCardStyle(isApplied)}
            >
              {renderPresetCardHeader(preset.name, preset.description, isApplied)}
              <StylePresetPreview config={presetConfig} />
            </button>
          );
        })}
      </div>
    </div>
  );

  const handleCopyStyleCode = async () => {
    try {
      await navigator.clipboard.writeText(currentStyleCode);
      setStyleToolsStatus(
        t(
          "settings.userInterface.recordingOverlay.styleCode.copySuccess",
          "Style code copied.",
        ),
      );
    } catch (error) {
      console.error("Failed to copy recording overlay style code:", error);
      setStyleToolsStatus(
        t(
          "settings.userInterface.recordingOverlay.styleCode.copyError",
          "Could not copy style code.",
        ),
      );
    }
  };

  const handlePasteStyleCode = async () => {
    try {
      const clipboardText = await navigator.clipboard.readText();
      setStyleCodeDraft(clipboardText);
      setStyleToolsStatus(
        t(
          "settings.userInterface.recordingOverlay.styleCode.pasteSuccess",
          "Clipboard loaded into the import field.",
        ),
      );
    } catch (error) {
      console.error("Failed to read recording overlay style code from clipboard:", error);
      setStyleToolsStatus(
        t(
          "settings.userInterface.recordingOverlay.styleCode.pasteError",
          "Could not read clipboard.",
        ),
      );
    }
  };

  const handleApplyStyleCode = async () => {
    if (!customOverlayEnabled || appearanceBusy || appearanceOperationRef.current) {
      return;
    }

    appearanceOperationRef.current = true;
    setStyleToolsStatus(null);
    setIsApplyingStyleCode(true);
    try {
      const parsed = parseRecordingOverlayStyleConfig(styleCodeDraft, currentStyleConfig);
      await applyStyleConfig(parsed);
      setStyleToolsStatus(
        t(
          "settings.userInterface.recordingOverlay.styleCode.applySuccess",
          "Imported style applied.",
        ),
      );
    } catch (error) {
      console.error("Failed to import recording overlay style code:", error);
      setStyleToolsStatus(
        error instanceof Error
          ? error.message
          : t(
              "settings.userInterface.recordingOverlay.styleCode.applyError",
              "Could not import style code.",
            ),
      );
    } finally {
      appearanceOperationRef.current = false;
      setIsApplyingStyleCode(false);
    }
  };

  const floatingPreviewCard = (
    <div className="space-y-3">
      <div className="mb-3 space-y-1">
        <div className="text-[11px] font-semibold uppercase tracking-[0.22em] text-[#ff8ebb]">
          Live Preview
        </div>
        <div className="text-sm font-semibold text-[#f4f4f4]">
          Floating side preview
        </div>
        <div className="text-xs leading-relaxed text-[#a8a8a8]">
          This panel stays reachable at every width. When there is enough room,
          it docks in the empty gutter; otherwise it collapses into a floating
          button.
        </div>
      </div>
      <RecordingOverlayPreview
        customEnabled={customOverlayEnabled}
        theme={overlayTheme}
        accentColor={accentColor}
        statusIconColor={statusIconColor}
        cancelIconColor={cancelIconColor}
        surfaceBaseColor={surfaceBaseColor}
        bodyBackgroundColor={bodyBackgroundColor}
        materialMode={materialMode}
        showStatusIcon={showStatusIcon}
        statusIconStyle={statusIconStyle}
        showCancelButton={showCancelButton}
        cancelButtonInvisible={cancelButtonInvisible}
        backgroundMode={backgroundMode}
        centerpieceMode={centerpieceMode}
        animatedBorderMode={animatedBorderMode}
        barCount={sliderDrafts.recording_overlay_bar_count}
        barWidthPx={sliderDrafts.recording_overlay_bar_width_px}
        barStyle={effectiveBarStyle}
        showDragGrip={showDragGrip}
        state={previewState}
        audioReactiveScale={audioReactiveScale}
        audioReactiveScaleMaxPercent={
          sliderDrafts.recording_overlay_audio_reactive_scale_max_percent
        }
        voiceSensitivityPercent={
          sliderDrafts.recording_overlay_voice_sensitivity_percent
        }
        animationSoftnessPercent={
          sliderDrafts.recording_overlay_animation_softness_percent
        }
        depthParallaxPercent={
          sliderDrafts.recording_overlay_depth_parallax_percent
        }
        opacityPercent={sliderDrafts.recording_overlay_opacity_percent}
        silenceFade={silenceFade}
        silenceOpacityPercent={
          sliderDrafts.recording_overlay_silence_opacity_percent
        }
        decapIndicatorMode={
          showDecapIndicatorInPreview ? decapIndicatorMode : "hidden"
        }
        decapIndicatorCustomText={decapIndicatorCustomText}
        decapIndicatorFontFamily={decapIndicatorFontFamily}
        decapIndicatorFontSizePx={
          sliderDrafts.recording_overlay_decapitalize_indicator_font_size_px
        }
        decapIndicatorColor={decapIndicatorColor}
        minimumWidthPx={sliderDrafts.recording_overlay_width_px}
        showAppChip={showAppInPreview}
        maxPreviewWidthPx={360}
      />
    </div>
  );

  const floatingPreview = (
    <>
      {floatingPreviewLayout.dockedVisible && (
        <div
          className="pointer-events-none fixed z-20 hidden xl:block transition-all duration-200 translate-x-0 opacity-100"
          style={{
            left: `${floatingPreviewLayout.dockLeft}px`,
            top: `${floatingPreviewLayout.dockTop}px`,
            width: "360px",
          }}
        >
          <div
            ref={floatingPreviewPanelRef}
            className="pointer-events-auto rounded-[22px] border border-[#2b2b2b] bg-[#131313]/95 p-4 shadow-[0_22px_44px_rgba(0,0,0,0.28)] backdrop-blur-sm"
          >
            {floatingPreviewCard}
          </div>
        </div>
      )}

      {!floatingPreviewLayout.dockedVisible && (
        <>
          <button
            ref={floatingPreviewButtonRef}
            type="button"
            onClick={() => setIsCollapsedPreviewOpen((current) => !current)}
            className="fixed z-30 flex h-12 w-12 items-center justify-center rounded-full border border-[#ff4d8d]/35 bg-[#161616]/95 text-[10px] font-semibold uppercase tracking-[0.18em] text-[#ff9cbe] shadow-[0_16px_34px_rgba(0,0,0,0.36)] backdrop-blur-md transition-all duration-200 hover:scale-[1.03] hover:border-[#ff4d8d]/55 hover:text-[#ffd2e1]"
            style={{
              left: `${floatingPreviewLayout.buttonLeft}px`,
              top: `${floatingPreviewLayout.buttonTop}px`,
            }}
            aria-label={isCollapsedPreviewOpen ? "Hide preview panel" : "Show preview panel"}
            title={isCollapsedPreviewOpen ? "Hide preview panel" : "Show preview panel"}
          >
            PV
          </button>

          {isCollapsedPreviewOpen && (
            <div className="fixed inset-0 z-40 pointer-events-none">
              <div
                ref={floatingPreviewPanelRef}
                className="pointer-events-auto absolute rounded-[22px] border border-[#2b2b2b] bg-[#131313]/98 p-4 pr-16 shadow-[0_26px_60px_rgba(0,0,0,0.4)] backdrop-blur-md"
                style={{
                  left: `${floatingPreviewLayout.overlayLeft}px`,
                  top: `${floatingPreviewLayout.overlayTop}px`,
                  width: "360px",
                }}
              >
                <button
                  type="button"
                  onClick={() => setIsCollapsedPreviewOpen(false)}
                  className="absolute right-4 top-4 shrink-0 rounded-full border border-white/10 bg-white/5 px-2 py-1 text-xs font-semibold text-[#d7d7d7] transition-colors hover:bg-white/10 hover:text-white"
                >
                  Close
                </button>
                {floatingPreviewCard}
              </div>
            </div>
          )}
        </>
      )}
    </>
  );

  return (
    <>
    <SettingsGroup
      id="recording-overlay-settings"
      title={t(
        "settings.userInterface.recordingOverlay.title",
        "Recording Overlay",
      )}
      collapsible={true}
      collapsed={isRecordingOverlayCollapsed}
      collapseLabel={t(
        "settings.userInterface.recordingOverlay.collapse",
        "Collapse",
      )}
      expandLabel={t(
        "settings.userInterface.recordingOverlay.expand",
        "Expand",
      )}
      onCollapsedChange={updateRecordingOverlayCollapsed}
    >
      {styleToolsStatus && (
        <div role="status" aria-live="polite" className="px-4 py-3 text-xs text-[#ffb6cf]">
          {styleToolsStatus}
        </div>
      )}
      <fieldset disabled={appearanceBusy} aria-busy={appearanceBusy} className="min-w-0 border-0 p-0 m-0 flex flex-col divide-y divide-white/[0.05]">
        <div className="order-1">
      <SettingContainer
        title={t(
          "settings.userInterface.recordingOverlay.preview.title",
          "Recording Overlay Preview",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.preview.description",
          "Preview the main overlay style here before using it in live recording.",
        )}
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <div className="space-y-4">
          <div ref={floatingPreviewAnchorRef} className="h-0" />
          <div className="flex flex-wrap gap-2">
            {previewStates.map((option) => {
              const selected = previewState === option.value;
              return (
                <button
                  key={option.value}
                  type="button"
                  onClick={() => setPreviewState(option.value)}
                  aria-pressed={selected}
                  className={`rounded-md border px-3 py-1.5 text-xs font-medium transition-colors ${
                    selected
                      ? "border-[#ff4d8d] bg-[#ff4d8d]/15 text-[#ff8ebb]"
                      : "border-[#3a3a3a] bg-[#1d1d1d] text-[#cfcfcf] hover:border-[#555555]"
                  }`}
                >
                  {option.label}
                </button>
              );
            })}
          </div>
          <ToggleSwitch
            checked={showAppInPreview}
            onChange={setShowAppInPreview}
            label="Show application chip in preview"
            description="Shows a sample application name in this preview only."
            descriptionMode="tooltip"
            grouped={true}
          />
          <div className="rounded-lg border border-dashed border-[#3a3a3a] bg-[#181818] px-3 py-2 text-xs leading-relaxed text-[#a8a8a8] xl:hidden">
            Preview docks in the empty left gutter when there is enough room.
            On narrower windows it collapses into a floating button that opens
            the preview above everything else.
          </div>
        </div>
      </SettingContainer>
        </div>

        <div className="order-2">
      <ToggleSwitch
        id="settings-overlay-custom"
        checked={customOverlayEnabled}
        onChange={(enabled) =>
          void updateSetting("recording_overlay_custom_enabled", enabled)
        }
        isUpdating={isUpdating("recording_overlay_custom_enabled")}
        label={t(
          "settings.userInterface.recordingOverlay.customOverlay.label",
          "Custom Overlay",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.customOverlay.description",
          "Turn on the premium custom renderer with presets, decorative layers, and advanced motion controls.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      />
      <div className="px-4 pb-4 text-xs text-[#a8a8a8]">
        {customOverlayEnabled
          ? t(
              "settings.userInterface.recordingOverlay.customOverlay.enabledHelp",
              "Custom mode is active. Decorative layers, presets, and advanced motion controls are available below.",
            )
          : t(
              "settings.userInterface.recordingOverlay.customOverlay.disabledHelp",
              "Classic mode stays simpler, but still keeps the same overlay moving, drag handle, and placement behavior.",
            )}
      </div>
        </div>

        <div
          className="order-3"
          title={customOverlayEnabled ? undefined : customOverlayDisabledReason}
        >
          <fieldset disabled={!customOverlayEnabled || appearanceBusy}
            className={
              customOverlayEnabled
                ? "min-w-0 border-0 p-0 m-0"
                : "min-w-0 border-0 p-0 m-0 opacity-45"
            }
          >
      <SettingContainer
        id="settings-overlay-presets"
        title={t(
          "settings.userInterface.recordingOverlay.presets.title",
          "Hero Preset Packs",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.presets.description",
          "Apply a dramatic curated look with premium materials, centerpieces, and motion in one click.",
        )}
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <div ref={presetsToolbarSentinelRef} className="h-0" aria-hidden="true" />
        <div className="space-y-3">
          <div
            ref={presetsToolbarRef}
            className={`flex scroll-mt-3 flex-wrap items-center justify-between gap-2 rounded-lg border px-3 py-2 transition-colors duration-150 ${
              arePresetsExpanded ? "sticky top-3 z-30" : ""
            } ${
              isPresetsToolbarStuck
                ? "pointer-events-none border-transparent bg-transparent"
                : "border-[#2f2f2f] bg-[#171717]"
            }`}
          >
            <div
              className={`text-xs text-[#a7a7a7] ${
                isPresetsToolbarStuck ? "invisible" : ""
              }`}
            >
              {t(
                "settings.userInterface.recordingOverlay.presets.memoryHint",
                "Collapse preset packs to unload the preview cards and save memory.",
              )}
            </div>
            <button
              type="button"
              onClick={togglePresetsExpanded}
              className={
                isPresetsToolbarStuck
                  ? "pointer-events-auto ml-auto rounded-full border border-[#ff4d8d]/40 bg-[#161616]/95 px-3 py-1.5 text-xs font-medium text-[#ffd6e5] shadow-[0_12px_28px_rgba(0,0,0,0.42)] backdrop-blur-md transition-colors hover:border-[#ff4d8d]/60 hover:bg-[#202020]"
                  : "ml-auto rounded-md border border-[#3f3f3f] bg-[#202020] px-3 py-1.5 text-xs font-medium text-[#ededed] transition-colors hover:bg-[#2b2b2b]"
              }
            >
              {arePresetsExpanded
                ? t(
                    "settings.userInterface.recordingOverlay.presets.collapse",
                    "Collapse Presets",
                  )
                : t(
                    "settings.userInterface.recordingOverlay.presets.expand",
                    "Expand Presets",
                  )}
            </button>
          </div>
          <div className="space-y-3 rounded-lg border border-[#2f2f2f] bg-[#151515] p-3">
            <div>
              <div className="text-xs font-semibold uppercase tracking-[0.18em] text-[#ff8ebb]">
                {t(
                  "settings.userInterface.recordingOverlay.presets.user.title",
                  "My Presets",
                )}
              </div>
              <div className="mt-1 text-xs leading-relaxed text-[#9d9d9d]">
                {t(
                  "settings.userInterface.recordingOverlay.presets.user.description",
                  "Save the current look, including width, icon colors, and the decapitalize indicator, and bring it back in one click.",
                )}
              </div>
            </div>
            <form
              className="flex flex-wrap items-center gap-2"
              onSubmit={(event) => {
                event.preventDefault();
                void handleSaveUserPreset();
              }}
            >
              <input
                type="text"
                value={userPresetName}
                maxLength={40}
                onChange={(event) => setUserPresetName(event.target.value)}
                placeholder={t(
                  "settings.userInterface.recordingOverlay.presets.user.namePlaceholder",
                  "Preset name",
                )}
                aria-label={t(
                  "settings.userInterface.recordingOverlay.presets.user.nameLabel",
                  "New preset name",
                )}
                className="min-w-0 flex-1 rounded-md border border-[#3c3c3c] bg-[#111111] px-3 py-2 text-sm text-[#f5f5f5] placeholder:text-[#777777] disabled:opacity-40"
              />
              <button
                type="submit"
                disabled={!userPresetName.trim() || userPresetsBusy || appearanceBusy}
                className="rounded-md border border-[#5a2c40] bg-[#ff4d8d]/15 px-3 py-2 text-xs font-medium text-[#ffd6e5] transition-colors hover:bg-[#ff4d8d]/22 disabled:cursor-not-allowed disabled:opacity-45"
              >
                {matchingUserPreset
                  ? t(
                      "settings.userInterface.recordingOverlay.presets.user.update",
                      "Update “{{name}}”",
                      { name: matchingUserPreset.name },
                    )
                  : t(
                      "settings.userInterface.recordingOverlay.presets.user.save",
                      "Save Current Look",
                    )}
              </button>
            </form>
            {userPresets.length === 0 ? (
              <div className="text-xs text-[#8a8a8a]">
                {t(
                  "settings.userInterface.recordingOverlay.presets.user.empty",
                  "No saved presets yet.",
                )}
              </div>
            ) : arePresetsExpanded ? (
              <div className="grid gap-3 xl:grid-cols-2">
                {userPresets.map(({ preset, config, isApplied }) => (
                  <div key={preset.id} className="relative">
                    <button
                      type="button"
                      onClick={() => void handleApplyPreset(config)}
                      aria-pressed={isApplied}
                      disabled={!customOverlayEnabled || appearanceBusy}
                      className={`${presetCardClassName(isApplied)} pr-10`}
                      style={presetCardStyle(isApplied)}
                    >
                      {renderPresetCardHeader(preset.name, null, isApplied)}
                      <StylePresetPreview config={config} />
                    </button>
                    <button
                      type="button"
                      onClick={() => setPendingDeleteUserPreset(preset)}
                      disabled={userPresetsBusy}
                      aria-label={t(
                        "settings.userInterface.recordingOverlay.presets.user.deleteLabel",
                        "Delete preset {{name}}",
                        { name: preset.name },
                      )}
                      title={t(
                        "settings.userInterface.recordingOverlay.presets.user.delete",
                        "Delete preset",
                      )}
                      className="absolute right-2 top-2 rounded-md p-1.5 text-[#9d9d9d] transition-colors hover:bg-white/10 hover:text-[#ffd6e5] disabled:cursor-not-allowed disabled:opacity-45"
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  </div>
                ))}
              </div>
            ) : null}
          </div>
          <Collapse open={arePresetsExpanded} className="-mt-3">
            <div className="space-y-3 pt-3">
              {renderPresetGroup(
                t(
                  "settings.userInterface.recordingOverlay.presets.modernTitle",
                  "Modern",
                ),
                MODERN_RECORDING_OVERLAY_PRESETS,
              )}
              {renderPresetGroup(
                t(
                  "settings.userInterface.recordingOverlay.presets.depthTitle",
                  "3D Depth",
                ),
                DEPTH_RECORDING_OVERLAY_PRESETS,
              )}
              {renderPresetGroup(
                t(
                  "settings.userInterface.recordingOverlay.presets.originalTitle",
                  "Original Packs",
                ),
                ORIGINAL_RECORDING_OVERLAY_PRESETS,
              )}
            </div>
          </Collapse>
        </div>
      </SettingContainer>
          </fieldset>
        </div>

        <div className="order-4">
      <TellMeMore
        title={t(
          "settings.userInterface.recordingOverlay.help.title",
          "How To Use These Controls",
        )}
        defaultOpen={false}
      >
        <div className="space-y-2">
          <p>
            {t(
              "settings.userInterface.recordingOverlay.help.presetsFirst",
              "Start with Preset Packs if you want a fast direction, then shape the result with the controls below.",
            )}
          </p>
          <p>
            {t(
              "settings.userInterface.recordingOverlay.help.grouping",
              "The controls are grouped into cards: Shape, Surface, Visualizer, Atmosphere, and Motion. Work from top to bottom, or start from a preset and adjust.",
            )}
          </p>
          <p>
            {t(
              "settings.userInterface.recordingOverlay.help.sliders",
              "Sliders update the preview while you drag and apply to the live overlay when you let go.",
            )}
          </p>
        </div>
      </TellMeMore>
        </div>

        <div
          className="order-8"
          title={customOverlayEnabled ? undefined : customOverlayDisabledReason}
        >
          <fieldset disabled={!customOverlayEnabled || appearanceBusy}
            className={
              customOverlayEnabled
                ? "min-w-0 border-0 p-0 m-0"
                : "min-w-0 border-0 p-0 m-0 opacity-45"
            }
          >
      <SettingContainer
        id="settings-overlay-style-code"
        title={t(
          "settings.userInterface.recordingOverlay.styleCode.title",
          "Share Style",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.styleCode.description",
          "Copy your current look as a reusable code, or import one from anywhere.",
        )}
        descriptionMode="inline"
        layout="stacked"
        grouped={true}
      >
        <div className="space-y-4">
          <div className="space-y-2">
            <div className="text-xs font-semibold uppercase tracking-[0.16em] text-[#a0a0a0]">
              {t(
                "settings.userInterface.recordingOverlay.styleCode.current",
                "Current Style Code",
              )}
            </div>
            <textarea
              readOnly
              aria-label="Current overlay style code"
              value={currentStyleCode}
              className="min-h-[88px] w-full rounded-lg border border-[#353535] bg-[#141414] px-3 py-2 text-xs leading-relaxed text-[#d6d6d6] outline-none"
            />
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() => void handleCopyStyleCode()}
                className="rounded-md border border-[#3c3c3c] bg-[#202020] px-3 py-2 text-xs font-medium text-[#e5e5e5] transition-colors hover:bg-[#2a2a2a]"
              >
                {t(
                  "settings.userInterface.recordingOverlay.styleCode.copy",
                  "Copy Current Code",
                )}
              </button>
            </div>
          </div>

          <div className="space-y-2">
            <div className="text-xs font-semibold uppercase tracking-[0.16em] text-[#a0a0a0]">
              {t(
                "settings.userInterface.recordingOverlay.styleCode.import",
                "Import Style Code",
              )}
            </div>
            <textarea
              value={styleCodeDraft}
              aria-label="Import overlay style code"
              onChange={(event) => setStyleCodeDraft(event.target.value)}
              placeholder={t(
                "settings.userInterface.recordingOverlay.styleCode.importPlaceholder",
                "Paste an Aivo overlay style code or JSON here.",
              )}
              className="min-h-[96px] w-full rounded-lg border border-[#353535] bg-[#141414] px-3 py-2 text-xs leading-relaxed text-[#f0f0f0] outline-none transition-colors focus:border-[#ff4d8d]"
            />
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() => void handlePasteStyleCode()}
                className="rounded-md border border-[#3c3c3c] bg-[#202020] px-3 py-2 text-xs font-medium text-[#e5e5e5] transition-colors hover:bg-[#2a2a2a]"
              >
                {t(
                  "settings.userInterface.recordingOverlay.styleCode.loadClipboard",
                  "Load From Clipboard",
                )}
              </button>
              <button
                type="button"
                onClick={() => void handleApplyStyleCode()}
                disabled={!customOverlayEnabled || appearanceBusy || !styleCodeDraft.trim()}
                className="rounded-md border border-[#5a2c40] bg-[#ff4d8d]/15 px-3 py-2 text-xs font-medium text-[#ffd6e5] transition-colors hover:bg-[#ff4d8d]/22 disabled:cursor-not-allowed disabled:opacity-45"
              >
                {t(
                  "settings.userInterface.recordingOverlay.styleCode.apply",
                  "Apply Imported Style",
                )}
              </button>
            </div>
          </div>
        </div>
      </SettingContainer>
          </fieldset>
        </div>

        <div className="order-0">
      <SettingContainer
        id="settings-overlay-reset"
        title={t(
          "settings.userInterface.recordingOverlay.reset.title",
          "Reset Overlay",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.reset.description",
          "Reset either the overlay look or its saved manual position without touching unrelated settings.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      >
        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={() => void handleResetAppearance()}
            disabled={appearanceBusy}
            title={t(
              "settings.userInterface.recordingOverlay.reset.appearanceTooltip",
              "This resets the current overlay look. For the most default/basic variant, turn off Custom Overlay.",
            )}
            className="inline-flex items-center gap-2 rounded-md border border-[#3c3c3c] bg-[#202020] px-3 py-2 text-xs font-medium text-[#e5e5e5] transition-colors hover:bg-[#2a2a2a] disabled:cursor-not-allowed disabled:opacity-45"
          >
            <RotateCcw className="h-3.5 w-3.5" />
            <span>
              {t(
                "settings.userInterface.recordingOverlay.reset.appearance",
                "Reset Appearance",
              )}
            </span>
          </button>
          <button
            type="button"
            onClick={() => void handleResetPosition()}
            disabled={appearanceBusy || !hasManualPosition}
            className="inline-flex items-center gap-2 rounded-md border border-[#3c3c3c] bg-[#202020] px-3 py-2 text-xs font-medium text-[#e5e5e5] transition-colors hover:bg-[#2a2a2a] disabled:cursor-not-allowed disabled:opacity-45"
          >
            <RotateCcw className="h-3.5 w-3.5" />
            <span>
              {t(
                "settings.userInterface.recordingOverlay.reset.position",
                "Reset Position",
              )}
            </span>
          </button>
        </div>
      </SettingContainer>

      <ShowOverlay descriptionMode="tooltip" grouped={true} />
        </div>

        <div className="order-5">
      <OverlaySettingsSection
        title={t(
          "settings.userInterface.recordingOverlay.sections.shape.title",
          "Shape",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.sections.shape.description",
          "How wide the overlay is and what sits on its sides: the status icon on the left and the cancel button on the right.",
        )}
      >
      <Slider
        id="settings-overlay-width"
        label={t(
          "settings.userInterface.recordingOverlay.overlayWidth.title",
          "Overlay Width",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.overlayWidth.description",
          "The smallest width of the overlay. It grows on its own when the visualizer needs more room. Built-in presets keep your width.",
        )}
        descriptionMode="tooltip"
        grouped={true}
        min={172}
        max={420}
        step={1}
        value={sliderDrafts.recording_overlay_width_px}
        formatValue={(value) => `${Math.round(value)} px`}
        onChange={(value) =>
          updateSliderDraft("recording_overlay_width_px", value)
        }
        onChangeComplete={(value) =>
          void commitSliderDraft("recording_overlay_width_px", value)
        }
      />

      <ToggleSwitch
        id="settings-overlay-status-icon"
        checked={showStatusIcon}
        onChange={(enabled) =>
          void updateSetting(
            "recording_overlay_show_status_icon" as any,
            enabled as any,
          )
        }
        isUpdating={isUpdating("recording_overlay_show_status_icon")}
        label={t(
          "settings.userInterface.recordingOverlay.statusIcon.label",
          "Show Status Icon",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.statusIcon.description",
          "Show the icon on the left: the ear while recording, a processing icon while your speech is handled, and a cross on errors.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      />

      <CustomOverlayOnly
        enabled={customOverlayEnabled}
        busy={appearanceBusy}
        reason={customOverlayDisabledReason}
      >
      <SettingContainer
        id="settings-overlay-status-icon-style"
        title={t(
          "settings.userInterface.recordingOverlay.statusIconStyle.title",
          "Status Icon Style",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.statusIconStyle.description",
          "What surrounds the status icon. Auto keeps the glass capsule on the original materials and shows a clean icon on the flat modern ones. Live Dot replaces the icon with a pulsing dot.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      >
        <Dropdown
          options={statusIconStyleOptions}
          selectedValue={statusIconStyle}
          onSelect={(value) =>
            void updateSetting(
              "recording_overlay_status_icon_style" as any,
              value as any,
            )
          }
          disabled={!customOverlayEnabled || !showStatusIcon}
        />
      </SettingContainer>
      </CustomOverlayOnly>

      <SettingContainer
        id="settings-overlay-status-icon-color"
        title={t(
          "settings.userInterface.recordingOverlay.statusIconColor.title",
          "Status Icon Color",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.statusIconColor.description",
          "Color of the icon on the left. Built-in presets keep your choice.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      >
        <div className="flex items-center gap-3">
          <input
            type="color"
            value={statusIconColor}
            onChange={(event) =>
              void updateSetting(
                "recording_overlay_status_icon_color" as any,
                event.target.value as any,
              )
            }
            className="h-8 w-12 rounded border border-[#3c3c3c] bg-transparent disabled:opacity-40"
          />
          <span className="text-xs font-mono text-[#a0a0a0]">
            {statusIconColor}
          </span>
        </div>
      </SettingContainer>

      <ToggleSwitch
        id="settings-overlay-cancel-button"
        checked={showCancelButton}
        onChange={(enabled) =>
          void updateSetting(
            "recording_overlay_show_cancel_button" as any,
            enabled as any,
          )
        }
        isUpdating={isUpdating("recording_overlay_show_cancel_button")}
        label={t(
          "settings.userInterface.recordingOverlay.showCancelButton.label",
          "Show Cancel Button",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.showCancelButton.description",
          "Show the × button on the right. Clicking it cancels the recording.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      />

      <div>
      <ToggleSwitch
        id="settings-overlay-invisible-cancel"
        checked={cancelButtonInvisible}
        onChange={(enabled) =>
          void updateSetting(
            "recording_overlay_cancel_button_invisible" as any,
            enabled as any,
          )
        }
        isUpdating={isUpdating("recording_overlay_cancel_button_invisible")}
        disabled={!showCancelButton}
        label={t(
          "settings.userInterface.recordingOverlay.cancelButtonInvisible.label",
          "Invisible Cancel Button",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.cancelButtonInvisible.description",
          "Hide the × but keep its spot working: clicking the right end of the overlay still cancels the recording. The preview marks the spot with a dashed circle.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      />
      {showCancelButton && cancelButtonInvisible && (
        <div
          role="note"
          className="mx-4 mb-3 flex items-start gap-2 rounded-lg border border-[#ff4d4d]/35 bg-[#ff4d4d]/10 px-3 py-2 text-xs leading-relaxed text-[#ff8080]"
        >
          <TriangleAlert className="mt-0.5 h-3.5 w-3.5 shrink-0" aria-hidden="true" />
          <span>
            {t(
              "settings.userInterface.recordingOverlay.cancelButtonInvisible.warning",
              "Blind mode: the × is gone, so you cancel by feel. Click the empty spot at the right end of the overlay.",
            )}
          </span>
        </div>
      )}
      </div>

      <SettingContainer
        id="settings-overlay-cancel-color"
        title={t(
          "settings.userInterface.recordingOverlay.cancelIconColor.title",
          "Cancel Button Icon Color",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.cancelIconColor.description",
          "Color of the × on the cancel button. Built-in presets keep your choice.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      >
        <div className="flex items-center gap-3">
          <input
            type="color"
            value={cancelIconColor}
            onChange={(event) =>
              void updateSetting(
                "recording_overlay_cancel_icon_color" as any,
                event.target.value as any,
              )
            }
            className="h-8 w-12 rounded border border-[#3c3c3c] bg-transparent disabled:opacity-40"
          />
          <span className="text-xs font-mono text-[#a0a0a0]">
            {cancelIconColor}
          </span>
        </div>
      </SettingContainer>
      </OverlaySettingsSection>

      <OverlaySettingsSection
        title={t(
          "settings.userInterface.recordingOverlay.sections.surface.title",
          "Surface",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.sections.surface.description",
          "What the overlay is made of and how it is colored.",
        )}
      >
      <SettingContainer
        id="settings-overlay-theme"
        title={t(
          "settings.userInterface.recordingOverlay.theme.title",
          "Overlay Theme",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.theme.description",
          "The overall shape. Classic is soft and rounded, Minimal is flatter with smaller corners, and Glass adds a see-through accent tint.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      >
        <Dropdown
          options={themeOptions}
          selectedValue={overlayTheme}
          onSelect={(value) =>
            void updateSetting("recording_overlay_theme" as any, value as any)
          }
          disabled={isUpdating("recording_overlay_theme")}
        />
      </SettingContainer>

      <CustomOverlayOnly
        enabled={customOverlayEnabled}
        busy={appearanceBusy}
        reason={customOverlayDisabledReason}
      >
      <SettingContainer
        id="settings-overlay-material"
        title={t(
          "settings.userInterface.recordingOverlay.materialMode.title",
          "Material Mode",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.materialMode.description",
          "What the surface is made of. The original materials glow and shimmer; Graphite, Obsidian, Gradient Mesh, and Porcelain are flat and modern.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      >
        <Dropdown
          options={materialModeOptions}
          selectedValue={materialMode}
          onSelect={(value) =>
            void updateSetting("recording_overlay_material_mode" as any, value as any)
          }
          disabled={
            isUpdating("recording_overlay_material_mode") || !customOverlayEnabled
          }
        />
      </SettingContainer>

      <Slider
        id="settings-overlay-opacity"
        label={t(
          "settings.userInterface.recordingOverlay.opacity.title",
          "Overlay Opacity",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.opacity.description",
          "How solid the overlay is. At 100% nothing behind it shows through.",
        )}
        descriptionMode="tooltip"
        grouped={true}
        min={20}
        max={100}
        step={1}
        value={sliderDrafts.recording_overlay_opacity_percent}
        formatValue={(value) => `${Math.round(value)} %`}
        onChange={(value) =>
          updateSliderDraft("recording_overlay_opacity_percent", value)
        }
        onChangeComplete={(value) =>
          void commitSliderDraft("recording_overlay_opacity_percent", value)
        }
      />
      </CustomOverlayOnly>

      <SettingContainer
        id="settings-overlay-body-color"
        title={t(
          "settings.userInterface.recordingOverlay.bodyBackgroundColor.title",
          "Body Background Color",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.bodyBackgroundColor.description",
          "The main color of the overlay. The theme and material add light and shade on top of it. Porcelain is always light and only takes a hint of this color.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      >
        <div className="flex items-center gap-3">
          <input
            type="color"
            value={bodyBackgroundColor}
            onChange={(event) =>
              void updateSetting(
                "recording_overlay_body_background_color" as any,
                event.target.value as any,
              )
            }
            className="h-8 w-12 rounded border border-[#3c3c3c] bg-transparent disabled:opacity-40"
          />
          <span className="text-xs font-mono text-[#a0a0a0]">
            {bodyBackgroundColor}
          </span>
        </div>
      </SettingContainer>

      <SettingContainer
        id="settings-overlay-surface-tint"
        title={t(
          "settings.userInterface.recordingOverlay.surfaceBaseColor.title",
          "Surface Tint",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.surfaceBaseColor.description",
          "A color that shows through the body from underneath. It is most visible at lower opacity and is handy for warming up or cooling down the surface.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      >
        <div className="flex items-center gap-3">
          <input
            type="color"
            value={surfaceBaseColor}
            onChange={(event) =>
              void updateSetting(
                "recording_overlay_surface_base_color" as any,
                event.target.value as any,
              )
            }
            className="h-8 w-12 rounded border border-[#3c3c3c] bg-transparent disabled:opacity-40"
          />
          <span className="text-xs font-mono text-[#a0a0a0]">
            {surfaceBaseColor}
          </span>
        </div>
      </SettingContainer>

      <SettingContainer
        id="settings-overlay-accent-color"
        title={t(
          "settings.userInterface.recordingOverlay.accentColor.title",
          "Overlay Accent Color",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.accentColor.description",
          "Color of the visualizer, glows, and hover highlights.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      >
        <div className="flex items-center gap-3">
          <input
            type="color"
            value={accentColor}
            onChange={(event) =>
              void updateSetting(
                "recording_overlay_accent_color" as any,
                event.target.value as any,
              )
            }
            className="h-8 w-12 rounded border border-[#3c3c3c] bg-transparent disabled:opacity-40"
          />
          <span className="text-xs font-mono text-[#a0a0a0]">{accentColor}</span>
        </div>
      </SettingContainer>
      </OverlaySettingsSection>

      <OverlaySettingsSection
        title={t(
          "settings.userInterface.recordingOverlay.sections.visualizer.title",
          "Visualizer",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.sections.visualizer.description",
          "The part that moves while you speak.",
        )}
      >
      <SettingContainer
        id="settings-overlay-visualizer-style"
        title={t(
          "settings.userInterface.recordingOverlay.barStyle.title",
          "Visualizer Style",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.barStyle.description",
          "How your voice is drawn: bars, waves, dots, liquid, and more. Classic mode offers a shorter list.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      >
        <Dropdown
          options={
            customOverlayEnabled
              ? barStyleOptions
              : barStyleOptions.filter((option) =>
                  LEGACY_RECORDING_OVERLAY_BAR_STYLES.includes(option.value),
                )
          }
          selectedValue={effectiveBarStyle}
          onSelect={(value) =>
            void updateSetting("recording_overlay_bar_style" as any, value as any)
          }
          disabled={isUpdating("recording_overlay_bar_style")}
        />
      </SettingContainer>

      <Slider
        id="settings-overlay-visualizer-count"
        label={t(
          "settings.userInterface.recordingOverlay.barCount.title",
          "Visualizer Count",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.barCount.description",
          "How many bars, dots, or drops are drawn.",
        )}
        descriptionMode="tooltip"
        grouped={true}
        min={3}
        max={16}
        step={1}
        value={sliderDrafts.recording_overlay_bar_count}
        formatValue={(value) => String(Math.round(value))}
        onChange={(value) =>
          updateSliderDraft("recording_overlay_bar_count", value)
        }
        onChangeComplete={(value) =>
          void commitSliderDraft("recording_overlay_bar_count", value)
        }
      />

      <Slider
        id="settings-overlay-visualizer-size"
        label={t(
          "settings.userInterface.recordingOverlay.barWidth.title",
          "Visualizer Size",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.barWidth.description",
          "How thick each bar, dot, or drop is. Thicker elements also make the overlay wider.",
        )}
        descriptionMode="tooltip"
        grouped={true}
        min={2}
        max={12}
        step={1}
        value={sliderDrafts.recording_overlay_bar_width_px}
        formatValue={(value) => `${Math.round(value)} px`}
        onChange={(value) =>
          updateSliderDraft("recording_overlay_bar_width_px", value)
        }
        onChangeComplete={(value) =>
          void commitSliderDraft("recording_overlay_bar_width_px", value)
        }
      />
      </OverlaySettingsSection>

      <OverlaySettingsSection
        title={t(
          "settings.userInterface.recordingOverlay.sections.atmosphere.title",
          "Atmosphere",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.sections.atmosphere.description",
          "Optional decoration around the visualizer. Leave it off for a clean look.",
        )}
        note={customOverlayEnabled ? null : customOverlayDisabledReason}
      >
      <CustomOverlayOnly
        enabled={customOverlayEnabled}
        busy={appearanceBusy}
        reason={customOverlayDisabledReason}
      >
      <SettingContainer
        id="settings-overlay-ambient-background"
        title={t(
          "settings.userInterface.recordingOverlay.backgroundMode.title",
          "Background Mode",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.backgroundMode.description",
          "A soft animated layer behind the visualizer, such as mist or sparks.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      >
        <Dropdown
          options={backgroundModeOptions}
          selectedValue={backgroundMode}
          onSelect={(value) =>
            void updateSetting(
              "recording_overlay_background_mode" as any,
              value as any,
            )
          }
          disabled={
            isUpdating("recording_overlay_background_mode") || !customOverlayEnabled
          }
        />
      </SettingContainer>

      <SettingContainer
        title={t(
          "settings.userInterface.recordingOverlay.centerpieceMode.title",
          "Centerpiece Mode",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.centerpieceMode.description",
          "A decorative shape that glows in the middle of the overlay, behind the visualizer.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      >
        <Dropdown
          options={centerpieceModeOptions}
          selectedValue={centerpieceMode}
          onSelect={(value) =>
            void updateSetting("recording_overlay_centerpiece_mode" as any, value as any)
          }
          disabled={
            isUpdating("recording_overlay_centerpiece_mode") || !customOverlayEnabled
          }
        />
      </SettingContainer>

      <SettingContainer
        title={t(
          "settings.userInterface.recordingOverlay.animatedBorderMode.title",
          "Animated Border",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.animatedBorderMode.description",
          "Light that moves along the edge of the overlay.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      >
        <Dropdown
          options={animatedBorderModeOptions}
          selectedValue={animatedBorderMode}
          onSelect={(value) =>
            void updateSetting(
              "recording_overlay_animated_border_mode" as any,
              value as any,
            )
          }
          disabled={
            isUpdating("recording_overlay_animated_border_mode") || !customOverlayEnabled
          }
        />
      </SettingContainer>

      <Slider
        label={t(
          "settings.userInterface.recordingOverlay.depthParallax.title",
          "Depth Parallax",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.depthParallax.description",
          "How far the background and centerpiece drift to create depth. Needs an ambient background or a centerpiece.",
        )}
        descriptionMode="tooltip"
        grouped={true}
        min={0}
        max={100}
        step={1}
        value={sliderDrafts.recording_overlay_depth_parallax_percent}
        formatValue={(value) => `${Math.round(value)} %`}
        onChange={(value) =>
          updateSliderDraft("recording_overlay_depth_parallax_percent", value)
        }
        onChangeComplete={(value) =>
          void commitSliderDraft(
            "recording_overlay_depth_parallax_percent",
            value,
          )
        }
        disabled={backgroundMode === "none" && centerpieceMode === "none"}
      />
      </CustomOverlayOnly>
      </OverlaySettingsSection>

      <OverlaySettingsSection
        title={t(
          "settings.userInterface.recordingOverlay.sections.motion.title",
          "Motion",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.sections.motion.description",
          "How the overlay reacts to your voice and to pauses.",
        )}
        note={customOverlayEnabled ? null : customOverlayDisabledReason}
      >
      <CustomOverlayOnly
        enabled={customOverlayEnabled}
        busy={appearanceBusy}
        reason={customOverlayDisabledReason}
      >
      <ToggleSwitch
        checked={audioReactiveScale}
        onChange={(enabled) =>
          void updateSetting(
            "recording_overlay_audio_reactive_scale" as any,
            enabled as any,
          )
        }
        isUpdating={isUpdating("recording_overlay_audio_reactive_scale")}
        label={t(
          "settings.userInterface.recordingOverlay.audioReactiveScale.label",
          "Voice-Reactive Scale",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.audioReactiveScale.description",
          "The whole overlay grows slightly when you speak louder.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      />

      <Slider
        label={t(
          "settings.userInterface.recordingOverlay.audioReactiveScaleAmount.title",
          "Scale Strength",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.audioReactiveScaleAmount.description",
          "How much the overlay can grow when you are loudest.",
        )}
        descriptionMode="tooltip"
        grouped={true}
        min={0}
        max={24}
        step={1}
        value={sliderDrafts.recording_overlay_audio_reactive_scale_max_percent}
        formatValue={(value) => `${Math.round(value)} %`}
        onChange={(value) =>
          updateSliderDraft(
            "recording_overlay_audio_reactive_scale_max_percent",
            value,
          )
        }
        onChangeComplete={(value) =>
          void commitSliderDraft(
            "recording_overlay_audio_reactive_scale_max_percent",
            value,
          )
        }
        disabled={!audioReactiveScale}
      />

      <Slider
        label={t(
          "settings.userInterface.recordingOverlay.voiceSensitivity.title",
          "Voice Sensitivity",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.voiceSensitivity.description",
          "How quiet speech can be and still count as voice for scaling and fading. Higher values react to softer speech.",
        )}
        descriptionMode="tooltip"
        grouped={true}
        min={0}
        max={100}
        step={1}
        value={sliderDrafts.recording_overlay_voice_sensitivity_percent}
        formatValue={(value) => `${Math.round(value)} %`}
        onChange={(value) =>
          updateSliderDraft("recording_overlay_voice_sensitivity_percent", value)
        }
        onChangeComplete={(value) =>
          void commitSliderDraft(
            "recording_overlay_voice_sensitivity_percent",
            value,
          )
        }
        disabled={!audioReactiveScale && !silenceFade}
      />

      <Slider
        label={t(
          "settings.userInterface.recordingOverlay.animationSoftness.title",
          "Animation Softness",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.animationSoftness.description",
          "Low values feel snappy and lively, high values feel smooth and calm.",
        )}
        descriptionMode="tooltip"
        grouped={true}
        min={0}
        max={100}
        step={1}
        value={sliderDrafts.recording_overlay_animation_softness_percent}
        formatValue={(value) => `${Math.round(value)} %`}
        onChange={(value) =>
          updateSliderDraft("recording_overlay_animation_softness_percent", value)
        }
        onChangeComplete={(value) =>
          void commitSliderDraft(
            "recording_overlay_animation_softness_percent",
            value,
          )
        }
      />

      <ToggleSwitch
        checked={silenceFade}
        onChange={(enabled) =>
          void updateSetting(
            "recording_overlay_silence_fade" as any,
            enabled as any,
          )
        }
        isUpdating={isUpdating("recording_overlay_silence_fade")}
        label={t(
          "settings.userInterface.recordingOverlay.silenceFade.label",
          "Silence Fade",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.silenceFade.description",
          "Make the overlay more transparent during pauses. It comes back as soon as you speak.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      />

      <Slider
        label={t(
          "settings.userInterface.recordingOverlay.silenceOpacity.title",
          "Silence Opacity",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.silenceOpacity.description",
          "How visible the overlay stays during pauses. Needs Fade When Quiet.",
        )}
        descriptionMode="tooltip"
        grouped={true}
        min={20}
        max={100}
        step={1}
        value={sliderDrafts.recording_overlay_silence_opacity_percent}
        formatValue={(value) => `${Math.round(value)} %`}
        onChange={(value) =>
          updateSliderDraft("recording_overlay_silence_opacity_percent", value)
        }
        onChangeComplete={(value) =>
          void commitSliderDraft(
            "recording_overlay_silence_opacity_percent",
            value,
          )
        }
        disabled={!silenceFade}
      />
      </CustomOverlayOnly>
      </OverlaySettingsSection>

      <section
        id="recording-overlay-decapitalize-indicator"
        tabIndex={-1}
        className="mx-3 my-4 rounded-xl border border-[#ff4d8d]/20 bg-[#ff4d8d]/[0.04] outline-none focus-visible:ring-2 focus-visible:ring-[#ff4d8d]/35"
      >
        <div className="px-6 pt-4 pb-1">
          <h3 className="text-xs font-bold uppercase tracking-widest text-[#ff8ebb]">
            {t(
              "settings.userInterface.recordingOverlay.decapIndicator.title",
              "Decapitalize Indicator",
            )}
          </h3>
          <p className="mt-1 text-xs leading-relaxed text-[#a0a0a0]">
            {t(
              "settings.userInterface.recordingOverlay.decapIndicator.descriptionBeforeLink",
              "The chip shown on the recording overlay while decapitalization is armed. The feature itself is set up in",
            )}{" "}
            <button
              type="button"
              onClick={openDecapitalizeFeatureSettings}
              className="font-medium text-primary underline underline-offset-2 transition-colors hover:text-primary/80"
            >
              {t(
                "settings.userInterface.recordingOverlay.decapIndicator.featureLink",
                "Text Processing → Decapitalize After Manual Edit",
              )}
            </button>
            .
          </p>
        </div>
        <div className="divide-y divide-white/[0.05]">
        <ToggleSwitch
          checked={showDecapIndicatorInPreview}
          onChange={setShowDecapIndicatorInPreview}
          label={t(
            "settings.userInterface.recordingOverlay.decapIndicator.showInPreview.label",
            "Show Decapitalize Indicator In Preview",
          )}
          description={t(
            "settings.userInterface.recordingOverlay.decapIndicator.showInPreview.description",
            "Only affects the preview on this page. Preset cards never show the indicator.",
          )}
          descriptionMode="tooltip"
          grouped={true}
        />
        <SettingContainer
          title={t(
            "settings.userInterface.recordingOverlay.decapIndicator.mode.title",
            "Decapitalize Indicator Mode",
          )}
          description={t(
            "settings.userInterface.recordingOverlay.decapIndicator.mode.description",
            "Show the standard label, your own text or emoji, or nothing at all.",
          )}
          descriptionMode="tooltip"
          grouped={true}
        >
          <Dropdown
            options={decapIndicatorModeOptions}
            selectedValue={decapIndicatorMode}
            onSelect={(value) =>
              void updateSetting(
                "recording_overlay_decapitalize_indicator_mode" as any,
                value as any,
              )
            }
            disabled={isUpdating("recording_overlay_decapitalize_indicator_mode")}
          />
        </SettingContainer>

      <SettingContainer
        title={t(
          "settings.userInterface.recordingOverlay.decapIndicator.font.title",
          "Decapitalize Indicator Font",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.decapIndicator.font.description",
          "Font of the indicator chip.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      >
        <Dropdown
          options={decapIndicatorFontOptions}
          selectedValue={decapIndicatorFontFamily}
          onSelect={(value) =>
            void updateSetting(
              "recording_overlay_decapitalize_indicator_font_family" as any,
              value as any,
            )
          }
          disabled={
            isUpdating("recording_overlay_decapitalize_indicator_font_family") ||
            decapIndicatorMode === "hidden"
          }
        />
      </SettingContainer>

      <Slider
        label={t(
          "settings.userInterface.recordingOverlay.decapIndicator.size.title",
          "Decapitalize Indicator Size",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.decapIndicator.size.description",
          "Size of the indicator text or emoji.",
        )}
        descriptionMode="tooltip"
        grouped={true}
        min={6}
        max={48}
        step={1}
        value={sliderDrafts.recording_overlay_decapitalize_indicator_font_size_px}
        formatValue={(value) => `${Math.round(value)} px`}
        onChange={(value) =>
          updateSliderDraft(
            "recording_overlay_decapitalize_indicator_font_size_px",
            value,
          )
        }
        onChangeComplete={(value) =>
          void commitSliderDraft(
            "recording_overlay_decapitalize_indicator_font_size_px",
            value,
          )
        }
        disabled={decapIndicatorMode === "hidden"}
      />

      {decapIndicatorMode === "custom" && (
        <SettingContainer
          title={t(
            "settings.userInterface.recordingOverlay.decapIndicator.customText.title",
            "Custom Indicator Text / Emoji",
          )}
          description={t(
            "settings.userInterface.recordingOverlay.decapIndicator.customText.description",
            "Any short text, emoji, or both, up to 24 characters. The chip stays centered above the overlay.",
          )}
          descriptionMode="tooltip"
          grouped={true}
        >
          <div className="space-y-3">
            <input
              type="text"
              value={decapIndicatorCustomText}
              maxLength={24}
              onChange={(event) =>
                void updateSetting(
                  "recording_overlay_decapitalize_indicator_custom_text" as any,
                  event.target.value as any,
                )
              }
              placeholder={t(
                "settings.userInterface.recordingOverlay.decapIndicator.customText.placeholder",
                "eg. a, Aa, ✍️, lower",
              )}
              className="w-full rounded-md border border-[#3c3c3c] bg-[#111111] px-3 py-2 text-sm text-[#f5f5f5] placeholder:text-[#777777] disabled:opacity-40"
            />
            <div
              className="rounded-md border border-white/[0.08] bg-white/[0.03] px-3 py-2 text-center"
              style={{
                color: decapIndicatorColor,
                fontFamily: `${decapIndicatorFontFamily}, "Segoe UI Emoji", sans-serif`,
                fontSize: `${sliderDrafts.recording_overlay_decapitalize_indicator_font_size_px}px`,
                fontWeight: 600,
              }}
            >
              {decapIndicatorCustomText.trim() || "Decapitalization"}
            </div>
          </div>
        </SettingContainer>
      )}

      <SettingContainer
        title={t(
          "settings.userInterface.recordingOverlay.decapIndicator.color.title",
          "Decapitalize Indicator Color",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.decapIndicator.color.description",
          "Color of the indicator text or emoji.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      >
        <div className="flex items-center gap-3">
          <input
            type="color"
            value={decapIndicatorColor}
            onChange={(event) =>
              void updateSetting(
                "recording_overlay_decapitalize_indicator_color" as any,
                event.target.value as any,
              )
            }
            disabled={decapIndicatorMode === "hidden"}
            className="h-8 w-12 rounded border border-[#3c3c3c] bg-transparent disabled:opacity-40"
          />
          <span className="text-xs font-mono text-[#a0a0a0]">
            {decapIndicatorColor}
          </span>
        </div>
      </SettingContainer>
        </div>
      </section>
        </div>
      </fieldset>
    </SettingsGroup>
    {!isRecordingOverlayCollapsed && floatingPreview}
    <ConfirmationModal
      isOpen={pendingDeleteUserPreset !== null}
      onClose={() => setPendingDeleteUserPreset(null)}
      onConfirm={() => void handleDeleteUserPreset()}
      title={t(
        "settings.userInterface.recordingOverlay.presets.user.deleteConfirmTitle",
        "Delete preset?",
      )}
      message={t(
        "settings.userInterface.recordingOverlay.presets.user.deleteConfirmMessage",
        "Delete the saved preset “{{name}}”? This cannot be undone.",
        { name: pendingDeleteUserPreset?.name ?? "" },
      )}
      confirmText={t(
        "settings.userInterface.recordingOverlay.presets.user.deleteConfirm",
        "Delete",
      )}
      cancelText={t(
        "settings.userInterface.recordingOverlay.presets.user.cancel",
        "Cancel",
      )}
      variant="danger"
    />
    </>
  );
};
