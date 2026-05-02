import React from "react";
import { useTranslation } from "react-i18next";
import { RotateCcw } from "lucide-react";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { SettingContainer } from "../../ui/SettingContainer";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { Dropdown } from "../../ui/Dropdown";
import { Slider } from "../../ui/Slider";
import { TellMeMore } from "../../ui/TellMeMore";
import { useSettings } from "../../../hooks/useSettings";
import { ShowOverlay } from "../ShowOverlay";
import { RecordingOverlayPreview } from "./RecordingOverlayPreview";
import type {
  RecordingOverlayAnimatedBorderMode,
  RecordingOverlayBackgroundMode,
  RecordingOverlayBarStyle,
  RecordingOverlayCenterpieceMode,
  RecordingOverlayMaterialMode,
  RecordingOverlayTheme,
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
} from "../../../overlay/recordingOverlayAppearance";
import {
  DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG,
  getRecordingOverlayStyleConfigFromSettings,
  parseRecordingOverlayStyleConfig,
  RECORDING_OVERLAY_STYLE_PRESETS,
  RECORDING_OVERLAY_STYLE_SETTING_ENTRIES,
  serializeRecordingOverlayStyleConfig,
  type RecordingOverlayStyleConfig,
} from "../../../overlay/recordingOverlayStyleConfig";

type PreviewState = "recording" | "transcribing" | "error";

type OverlaySliderDraftKey =
  | "recording_overlay_bar_count"
  | "recording_overlay_width_px"
  | "recording_overlay_bar_width_px"
  | "recording_overlay_audio_reactive_scale_max_percent"
  | "recording_overlay_voice_sensitivity_percent"
  | "recording_overlay_animation_softness_percent"
  | "recording_overlay_depth_parallax_percent"
  | "recording_overlay_opacity_percent"
  | "recording_overlay_silence_opacity_percent";

type OverlaySliderDrafts = Record<OverlaySliderDraftKey, number>;

const RESET_RECORDING_OVERLAY_STYLE_CONFIG: RecordingOverlayStyleConfig = {
  ...DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG,
};

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
];

const PREVIEW_STATES: Array<{
  value: PreviewState;
  label: string;
  labelKey: string;
}> = [
  { value: "recording", label: "Recording", labelKey: "recording" },
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

const WINDOWS_FONT_FAMILY_OPTIONS = [
  "Segoe UI",
  "Segoe UI Emoji",
  "Bahnschrift",
  "Arial",
  "Verdana",
  "Tahoma",
  "Trebuchet MS",
  "Georgia",
  "Times New Roman",
  "Consolas",
  "Cascadia Mono",
] as const;

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

export const RecordingOverlaySettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting, isUpdating, refreshSettings } = useSettings();
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
  const [styleCodeDraft, setStyleCodeDraft] = React.useState("");
  const [styleToolsStatus, setStyleToolsStatus] = React.useState<string | null>(null);

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
  const showCancelButton = Boolean(
    (settings as any)?.recording_overlay_show_cancel_button ?? true,
  );
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
    (settings as any)?.recording_overlay_decapitalize_indicator_font_size_px ?? 16,
  );
  const decapIndicatorColor = normalizeRecordingOverlayColor(
    (settings as any)?.recording_overlay_decapitalize_indicator_color,
    "#72f29a",
  );
  const hasManualPosition = Boolean(
    (settings as any)?.recording_overlay_use_manual_position ?? false,
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
    label: option.label,
  }));
  const decapIndicatorFontOptions = WINDOWS_FONT_FAMILY_OPTIONS.map((fontFamily) => ({
    value: fontFamily,
    label: fontFamily,
  }));
  const currentStyleConfig = React.useMemo(
    () => getRecordingOverlayStyleConfigFromSettings(settings),
    [settings],
  );
  const currentStyleCode = React.useMemo(
    () => serializeRecordingOverlayStyleConfig(currentStyleConfig),
    [currentStyleConfig],
  );
  const appliedPresetId = React.useMemo(() => {
    for (const preset of RECORDING_OVERLAY_STYLE_PRESETS) {
      const presetStyleCode = serializeRecordingOverlayStyleConfig({
        ...DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG,
        ...preset.config,
      });
      if (presetStyleCode === currentStyleCode) {
        return preset.id;
      }
    }

    return null;
  }, [currentStyleCode]);
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
    }),
    [
      audioReactiveScaleMaxPercent,
      animationSoftnessPercent,
      barCount,
      barWidthPx,
      clampedOverlayWidthPx,
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
      const minLeft = 170;
      const buttonSize = 52;
      const nextDockLeft = Math.round(anchorRect.left - panelWidth - gap);
      const panelHeight = floatingPreviewPanelRef.current?.offsetHeight ?? 0;
      const maxDockTop = Math.max(24, window.innerHeight - panelHeight - 24);
      const nextDockTop = Math.round(
        Math.min(Math.max(anchorRect.top, 24), maxDockTop),
      );
      const nextDockedVisible =
        window.innerWidth >= 1280 && nextDockLeft >= minLeft;
      const nextButtonLeft = Math.round(
        Math.min(
          Math.max(anchorRect.left - buttonSize - 18, 20),
          window.innerWidth - buttonSize - 20,
        ),
      );
      const nextButtonTop = Math.round(
        Math.min(
          Math.max(anchorRect.top + 14, 20),
          window.innerHeight - buttonSize - 20,
        ),
      );
      const preferredOverlayLeft = nextButtonLeft + buttonSize + 14;
      const fallbackOverlayLeft = nextButtonLeft - panelWidth - 14;
      const nextOverlayLeft = Math.round(
        preferredOverlayLeft + panelWidth <= window.innerWidth - 20
          ? preferredOverlayLeft
          : fallbackOverlayLeft >= 20
            ? fallbackOverlayLeft
            : Math.max(20, Math.min(
                (window.innerWidth - panelWidth) / 2,
                window.innerWidth - panelWidth - 20,
              )),
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
  }, [previewState, showDecapIndicatorInPreview, isCollapsedPreviewOpen]);

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
      await updateSetting(key as any, Math.round(value) as any);
    },
    [updateSetting],
  );

  const applyStyleConfig = React.useCallback(
    async (config: RecordingOverlayStyleConfig) => {
      for (const [key, value] of RECORDING_OVERLAY_STYLE_SETTING_ENTRIES(config)) {
        await updateSetting(key as any, value as any);
      }
    },
    [updateSetting],
  );

  const handleResetAppearance = async () => {
    if (isResettingAppearance) {
      return;
    }

    setIsResettingAppearance(true);
    try {
      await applyStyleConfig(RESET_RECORDING_OVERLAY_STYLE_CONFIG);
      await updateSetting("recording_overlay_width_px" as any, 172 as any);
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
      });
      setStyleToolsStatus(
        t(
          "settings.userInterface.recordingOverlay.reset.appearanceDone",
          "Overlay appearance reset to the default look.",
        ),
      );
    } catch (error) {
      console.error("Failed to reset recording overlay appearance:", error);
    } finally {
      setIsResettingAppearance(false);
    }
  };

  const handleResetPosition = async () => {
    if (isResettingPosition) {
      return;
    }

    setIsResettingPosition(true);
    try {
      const result = await commands.resetRecordingOverlayManualPosition();
      if (result.status === "error") {
        throw new Error(String(result.error));
      }
      await refreshSettings();
    } catch (error) {
      console.error("Failed to reset recording overlay position:", error);
    } finally {
      setIsResettingPosition(false);
    }
  };

  const handleApplyPreset = async (config: RecordingOverlayStyleConfig) => {
    if (isApplyingPreset) {
      return;
    }

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
      setIsApplyingPreset(false);
    }
  };

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
    if (isApplyingStyleCode) {
      return;
    }

    setStyleToolsStatus(null);
    setIsApplyingStyleCode(true);
    try {
      const parsed = parseRecordingOverlayStyleConfig(styleCodeDraft);
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
        showCancelButton={showCancelButton}
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
          showDecapIndicatorInPreview
            ? decapIndicatorMode === "hidden"
              ? "text"
              : decapIndicatorMode
            : "hidden"
        }
        decapIndicatorCustomText={decapIndicatorCustomText}
        decapIndicatorFontFamily={decapIndicatorFontFamily}
        decapIndicatorFontSizePx={decapIndicatorFontSizePx}
        decapIndicatorColor={decapIndicatorColor}
        minimumWidthPx={sliderDrafts.recording_overlay_width_px}
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
      title={t(
        "settings.userInterface.recordingOverlay.title",
        "Recording Overlay",
      )}
    >
      <div className="flex flex-col divide-y divide-white/[0.05]">
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
          <div className="mt-3">
            <ToggleSwitch
              checked={showDecapIndicatorInPreview}
              onChange={setShowDecapIndicatorInPreview}
              label="Show Decapitalize Indicator In Preview"
              description="Only affects this settings-page preview. Preset cards never show the decapitalize indicator."
              descriptionMode="tooltip"
              grouped={true}
            />
          </div>
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
        checked={customOverlayEnabled}
        onChange={(enabled) =>
          void (async () => {
            if (!enabled) {
              const legacyBarStyle = normalizeLegacyRecordingOverlayBarStyle(barStyle);
              if (legacyBarStyle !== barStyle) {
                await updateSetting(
                  "recording_overlay_bar_style" as any,
                  legacyBarStyle as any,
                );
              }
            }
            await updateSetting(
              "recording_overlay_custom_enabled" as any,
              enabled as any,
            );
          })()
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
          <div
            className={
              customOverlayEnabled
                ? undefined
                : "pointer-events-none select-none opacity-45"
            }
          >
      <SettingContainer
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
        <div className="space-y-3">
          <div className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-[#2f2f2f] bg-[#171717] px-3 py-2">
            <div className="text-xs text-[#a7a7a7]">
              {t(
                "settings.userInterface.recordingOverlay.presets.memoryHint",
                "Collapse preset packs to unload the preview cards and save memory.",
              )}
            </div>
            <button
              type="button"
              onClick={() => setArePresetsExpanded((current) => !current)}
              className="rounded-md border border-[#3f3f3f] bg-[#202020] px-3 py-1.5 text-xs font-medium text-[#ededed] transition-colors hover:bg-[#2b2b2b]"
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
          {arePresetsExpanded && (
            <div className="grid gap-3 xl:grid-cols-2">
              {RECORDING_OVERLAY_STYLE_PRESETS.map((preset) => {
                const presetConfig = {
                  ...DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG,
                  ...preset.config,
                };
                const isApplied = preset.id === appliedPresetId;
                return (
                  <button
                    key={preset.id}
                    type="button"
                    onClick={() => void handleApplyPreset(presetConfig)}
                    aria-pressed={isApplied}
                    disabled={isApplyingPreset}
                    className={`rounded-xl border p-3 text-left transition-all duration-200 disabled:cursor-not-allowed disabled:opacity-45 ${
                      isApplied
                        ? "border-[#ff78b4] bg-[#20161c]"
                        : "border-[#323232] bg-[#191919] hover:border-[#4a4a4a] hover:bg-[#202020]"
                    }`}
                    style={
                      isApplied
                        ? {
                            background:
                              "linear-gradient(180deg, rgba(255,120,180,0.12), rgba(255,120,180,0.04))",
                            boxShadow:
                              "0 0 0 1px rgba(255,120,180,0.22), 0 18px 38px rgba(0,0,0,0.22)",
                          }
                        : undefined
                    }
                  >
                    <div className="mb-3 flex items-start justify-between gap-3">
                      <div>
                        <div className="text-sm font-semibold text-[#f2f2f2]">
                          {preset.name}
                        </div>
                        <div className="mt-1 text-xs leading-relaxed text-[#9d9d9d]">
                          {preset.description}
                        </div>
                      </div>
                      <span
                        className={`rounded-full px-2 py-0.5 text-[10px] font-medium uppercase tracking-[0.16em] ${
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
                    <RecordingOverlayPreview
                      customEnabled={true}
                      theme={presetConfig.theme}
                      accentColor={presetConfig.accentColor}
                      statusIconColor="#faa2ca"
                      cancelIconColor="#faa2ca"
                      surfaceBaseColor={presetConfig.surfaceBaseColor}
                      bodyBackgroundColor={presetConfig.bodyBackgroundColor}
                      materialMode={presetConfig.materialMode}
                      showStatusIcon={presetConfig.showStatusIcon}
                      showCancelButton={true}
                      backgroundMode={presetConfig.backgroundMode}
                      centerpieceMode={presetConfig.centerpieceMode}
                      animatedBorderMode={presetConfig.animatedBorderMode}
                      barCount={presetConfig.barCount}
                      barWidthPx={presetConfig.barWidthPx}
                      barStyle={presetConfig.barStyle}
                      showDragGrip={presetConfig.showDragGrip}
                      state="recording"
                      audioReactiveScale={presetConfig.audioReactiveScale}
                      audioReactiveScaleMaxPercent={
                        presetConfig.audioReactiveScaleMaxPercent
                      }
                      voiceSensitivityPercent={
                        presetConfig.voiceSensitivityPercent
                      }
                      animationSoftnessPercent={
                        presetConfig.animationSoftnessPercent
                      }
                      depthParallaxPercent={presetConfig.depthParallaxPercent}
                      opacityPercent={presetConfig.opacityPercent}
                      silenceFade={presetConfig.silenceFade}
                      silenceOpacityPercent={presetConfig.silenceOpacityPercent}
                      decapIndicatorMode="hidden"
                      decapIndicatorFontFamily="Segoe UI"
                      decapIndicatorFontSizePx={16}
                      decapIndicatorColor="#72f29a"
                      maxPreviewWidthPx={248}
                    />
                  </button>
                );
              })}
            </div>
          )}
          {styleToolsStatus && (
            <div className="text-xs text-[#ffb6cf]">{styleToolsStatus}</div>
          )}
        </div>
      </SettingContainer>
          </div>
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
              "Think in this order: Layout sets the silhouette, Style sets the core look, Atmosphere adds decorative layers, and Motion controls how the overlay behaves while you speak.",
            )}
          </p>
          <p>
            {t(
              "settings.userInterface.recordingOverlay.help.sliders",
              "Slider drags now update the page preview immediately and commit to the live overlay when you release them.",
            )}
          </p>
        </div>
      </TellMeMore>
        </div>

        <div
          className="order-8"
          title={customOverlayEnabled ? undefined : customOverlayDisabledReason}
        >
          <div
            className={
              customOverlayEnabled
                ? undefined
                : "pointer-events-none select-none opacity-45"
            }
          >
      <SettingContainer
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
                disabled={isApplyingStyleCode || !styleCodeDraft.trim()}
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
          </div>
        </div>

        <div className="order-9">
      <SettingContainer
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
            disabled={isResettingAppearance}
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
            disabled={isResettingPosition || !hasManualPosition}
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
      <SettingContainer
        title={t(
          "settings.userInterface.recordingOverlay.theme.title",
          "Overlay Theme",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.theme.description",
          "Change the surface style of the recording overlay.",
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

      <div
        title={customOverlayEnabled ? undefined : customOverlayDisabledReason}
        className={
          customOverlayEnabled
            ? undefined
            : "pointer-events-none select-none opacity-45"
        }
      >
      <SettingContainer
        title={t(
          "settings.userInterface.recordingOverlay.materialMode.title",
          "Material Mode",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.materialMode.description",
          "Choose the premium material treatment for the overlay surface.",
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

      <SettingContainer
        title={t(
          "settings.userInterface.recordingOverlay.backgroundMode.title",
          "Background Mode",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.backgroundMode.description",
          "Add a decorative ambient background behind the visualizer.",
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
          "Add a living focal motif at the heart of the overlay.",
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
          "Give the overlay edge a subtle premium motion treatment.",
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
      </div>

      <ToggleSwitch
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
          "Show the microphone, processing, and error icon on the left side of the overlay.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      />

      <ToggleSwitch
        checked={showCancelButton}
        onChange={(enabled) =>
          void updateSetting(
            "recording_overlay_show_cancel_button" as any,
            enabled as any,
          )
        }
        isUpdating={isUpdating("recording_overlay_show_cancel_button")}
        label="Show Cancel Button"
        description="Show the X/cancel action on the right side of the recording overlay."
        descriptionMode="tooltip"
        grouped={true}
      />

      <SettingContainer
        title="Status Icon Color"
        description="Color of the left-side status icon."
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
            disabled={isUpdating("recording_overlay_status_icon_color")}
            className="h-8 w-12 rounded border border-[#3c3c3c] bg-transparent disabled:opacity-40"
          />
          <span className="text-xs font-mono text-[#a0a0a0]">
            {statusIconColor}
          </span>
        </div>
      </SettingContainer>

      <SettingContainer
        title="Cancel Button Icon Color"
        description="Color of the right-side cancel/X icon."
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
            disabled={isUpdating("recording_overlay_cancel_icon_color")}
            className="h-8 w-12 rounded border border-[#3c3c3c] bg-transparent disabled:opacity-40"
          />
          <span className="text-xs font-mono text-[#a0a0a0]">
            {cancelIconColor}
          </span>
        </div>
      </SettingContainer>

      <SettingContainer
        title="Decapitalize Indicator Mode"
        description="Show the standard label, a custom emoji/text badge, or hide the decapitalize indicator completely."
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
        title="Decapitalize Indicator Font"
        description="Choose a Windows font family for the decapitalize indicator badge."
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
        label="Decapitalize Indicator Size"
        description="Adjust the size of the decapitalize indicator text or emoji."
        descriptionMode="tooltip"
        grouped={true}
        min={10}
        max={32}
        step={1}
        value={Math.max(10, Math.min(32, Math.round(decapIndicatorFontSizePx)))}
        formatValue={(value) => `${Math.round(value)} px`}
        onChange={(value) =>
          void updateSetting(
            "recording_overlay_decapitalize_indicator_font_size_px" as any,
            Math.round(value) as any,
          )
        }
        disabled={
          isUpdating("recording_overlay_decapitalize_indicator_font_size_px") ||
          decapIndicatorMode === "hidden"
        }
      />

      {decapIndicatorMode === "custom" && (
        <SettingContainer
          title="Custom Indicator Text / Emoji"
          description="Enter any short text, emoji, or both. The badge stays centered above the overlay."
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
              disabled={isUpdating("recording_overlay_decapitalize_indicator_custom_text")}
              placeholder="eg. a, Aa, ✍️, lower"
              className="w-full rounded-md border border-[#3c3c3c] bg-[#111111] px-3 py-2 text-sm text-[#f5f5f5] placeholder:text-[#777777] disabled:opacity-40"
            />
            <div
              className="rounded-md border border-white/[0.08] bg-white/[0.03] px-3 py-2 text-center"
              style={{
                color: decapIndicatorColor,
                fontFamily: `${decapIndicatorFontFamily}, "Segoe UI Emoji", sans-serif`,
                fontSize: `${Math.max(10, Math.min(32, Math.round(decapIndicatorFontSizePx)))}px`,
                fontWeight: 600,
              }}
            >
              {decapIndicatorCustomText.trim() || "Decapitalization"}
            </div>
          </div>
        </SettingContainer>
      )}

      <SettingContainer
        title="Decapitalize Indicator Color"
        description="Color of the decapitalize badge text or emoji."
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
            disabled={
              isUpdating("recording_overlay_decapitalize_indicator_color") ||
              decapIndicatorMode === "hidden"
            }
            className="h-8 w-12 rounded border border-[#3c3c3c] bg-transparent disabled:opacity-40"
          />
          <span className="text-xs font-mono text-[#a0a0a0]">
            {decapIndicatorColor}
          </span>
        </div>
      </SettingContainer>

      <SettingContainer
        title={t(
          "settings.userInterface.recordingOverlay.barStyle.title",
          "Visualizer Style",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.barStyle.description",
          "Choose how recording activity is visualized.",
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
        label={t(
          "settings.userInterface.recordingOverlay.barCount.title",
          "Visualizer Count",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.barCount.description",
          "Choose how many animated elements are shown while recording.",
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
        disabled={isUpdating("recording_overlay_bar_count")}
      />

      <Slider
        label={t(
          "settings.userInterface.recordingOverlay.overlayWidth.title",
          "Overlay Width",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.overlayWidth.description",
          "Choose the base width of the recording overlay. It can still expand if the current visualizer needs more room.",
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
        disabled={isUpdating("recording_overlay_width_px")}
      />

      <Slider
        label={t(
          "settings.userInterface.recordingOverlay.barWidth.title",
          "Visualizer Size",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.barWidth.description",
          "Adjust the size or thickness of the recording visualizer elements.",
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
        disabled={isUpdating("recording_overlay_bar_width_px")}
      />

      <SettingContainer
        title={t(
          "settings.userInterface.recordingOverlay.bodyBackgroundColor.title",
          "Body Background Color",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.bodyBackgroundColor.description",
          "Adjust the true background color of the overlay body beneath the material and glow layers.",
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
            disabled={isUpdating("recording_overlay_body_background_color")}
            className="h-8 w-12 rounded border border-[#3c3c3c] bg-transparent disabled:opacity-40"
          />
          <span className="text-xs font-mono text-[#a0a0a0]">
            {bodyBackgroundColor}
          </span>
        </div>
      </SettingContainer>

      <SettingContainer
        title={t(
          "settings.userInterface.recordingOverlay.surfaceBaseColor.title",
          "Surface Tint",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.surfaceBaseColor.description",
          "Adjust the tint and glow layered over the body background. Use Body Background Color for the actual base fill.",
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
            disabled={isUpdating("recording_overlay_surface_base_color")}
            className="h-8 w-12 rounded border border-[#3c3c3c] bg-transparent disabled:opacity-40"
          />
          <span className="text-xs font-mono text-[#a0a0a0]">
            {surfaceBaseColor}
          </span>
        </div>
      </SettingContainer>

      <SettingContainer
        title={t(
          "settings.userInterface.recordingOverlay.accentColor.title",
          "Overlay Accent Color",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.accentColor.description",
          "Pick the accent color used for the recording visualizer and hover accents.",
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
            disabled={isUpdating("recording_overlay_accent_color")}
            className="h-8 w-12 rounded border border-[#3c3c3c] bg-transparent disabled:opacity-40"
          />
          <span className="text-xs font-mono text-[#a0a0a0]">{accentColor}</span>
        </div>
      </SettingContainer>

        </div>

        <div
          className="order-6"
          title={customOverlayEnabled ? undefined : customOverlayDisabledReason}
        >
          <div
            className={
              customOverlayEnabled
                ? undefined
                : "pointer-events-none select-none opacity-45"
            }
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
          "Make the overlay subtly expand with stronger voice input.",
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
          "Choose the maximum growth amount when you speak loudly.",
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
        disabled={
          isUpdating("recording_overlay_audio_reactive_scale_max_percent") ||
          !audioReactiveScale
        }
      />

      <Slider
        label={t(
          "settings.userInterface.recordingOverlay.voiceSensitivity.title",
          "Voice Sensitivity",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.voiceSensitivity.description",
          "Choose how easily the overlay reacts to quieter speech. Higher values wake up sooner; lower values wait for stronger voice input.",
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
        disabled={
          isUpdating("recording_overlay_voice_sensitivity_percent") ||
          (!audioReactiveScale && !silenceFade)
        }
      />

      <Slider
        label={t(
          "settings.userInterface.recordingOverlay.animationSoftness.title",
          "Animation Softness",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.animationSoftness.description",
          "Make the overlay calmer and smoother, or snappier and more alive.",
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
        disabled={isUpdating("recording_overlay_animation_softness_percent")}
      />

      <Slider
        label={t(
          "settings.userInterface.recordingOverlay.depthParallax.title",
          "Depth Parallax",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.depthParallax.description",
          "Let different visual layers drift at slightly different depths.",
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
        disabled={isUpdating("recording_overlay_depth_parallax_percent")}
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
          "Make the overlay quieter during pauses and wake instantly when speech returns.",
        )}
        descriptionMode="tooltip"
        grouped={true}
      />

      <Slider
        label={t(
          "settings.userInterface.recordingOverlay.opacity.title",
          "Overlay Opacity",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.opacity.description",
          "Set the base transparency of the recording overlay.",
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
        disabled={isUpdating("recording_overlay_opacity_percent")}
      />

      <Slider
        label={t(
          "settings.userInterface.recordingOverlay.silenceOpacity.title",
          "Silence Opacity",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.silenceOpacity.description",
          "Set how visible the overlay stays while you are quiet.",
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
        disabled={
          isUpdating("recording_overlay_silence_opacity_percent") ||
          !silenceFade
        }
      />
          </div>
        </div>
      </div>
    </SettingsGroup>
    {floatingPreview}
    </>
  );
};
