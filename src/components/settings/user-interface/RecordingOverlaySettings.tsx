import React from "react";
import { useTranslation } from "react-i18next";
import { RotateCcw } from "lucide-react";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { SettingContainer } from "../../ui/SettingContainer";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { Dropdown } from "../../ui/Dropdown";
import { Slider } from "../../ui/Slider";
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

const THEME_OPTIONS: Array<{ value: RecordingOverlayTheme; label: string }> = [
  { value: "classic", label: "Classic" },
  { value: "minimal", label: "Minimal" },
  { value: "glass", label: "Glass" },
];

const BACKGROUND_MODE_OPTIONS: Array<{
  value: RecordingOverlayBackgroundMode;
  label: string;
}> = [
  { value: "none", label: "None" },
  { value: "mist", label: "Mist" },
  { value: "petals_haze", label: "Petals Haze" },
  { value: "soft_glow_field", label: "Soft Glow Field" },
  { value: "stardust", label: "Stardust" },
  { value: "silk_fog", label: "Silk Fog" },
  { value: "firefly_veil", label: "Firefly Veil" },
  { value: "rose_sparks", label: "Rose Sparks" },
];

const MATERIAL_MODE_OPTIONS: Array<{
  value: RecordingOverlayMaterialMode;
  label: string;
}> = [
  { value: "liquid_glass", label: "Liquid Glass" },
  { value: "pearl", label: "Pearl" },
  { value: "velvet_neon", label: "Velvet Neon" },
  { value: "frost", label: "Frost" },
  { value: "candy_chrome", label: "Candy Chrome" },
];

const CENTERPIECE_MODE_OPTIONS: Array<{
  value: RecordingOverlayCenterpieceMode;
  label: string;
}> = [
  { value: "none", label: "None" },
  { value: "halo_core", label: "Halo Core" },
  { value: "aurora_ribbon", label: "Aurora Ribbon" },
  { value: "orbital_beads", label: "Orbital Beads" },
  { value: "bloom_heart", label: "Bloom Heart" },
  { value: "signal_crown", label: "Signal Crown" },
];

const ANIMATED_BORDER_MODE_OPTIONS: Array<{
  value: RecordingOverlayAnimatedBorderMode;
  label: string;
}> = [
  { value: "none", label: "None" },
  { value: "shimmer_edge", label: "Shimmer Edge" },
  { value: "traveling_highlight", label: "Traveling Highlight" },
  { value: "breathing_contour", label: "Breathing Contour" },
];

const BAR_STYLE_OPTIONS: Array<{ value: RecordingOverlayBarStyle; label: string }> = [
  { value: "aurora", label: "Aurora" },
  { value: "bloom_bounce", label: "Bloom Bounce" },
  { value: "comet", label: "Comet" },
  { value: "constellation", label: "Constellation" },
  { value: "crown", label: "Crown" },
  { value: "daisy", label: "Daisy" },
  { value: "ember", label: "Ember" },
  { value: "fireflies", label: "Fireflies" },
  { value: "garden_sway", label: "Garden Sway" },
  { value: "hologram", label: "Hologram" },
  { value: "helix", label: "Helix" },
  { value: "lotus", label: "Lotus" },
  { value: "matrix", label: "Matrix Rain" },
  { value: "morse", label: "Morse" },
  { value: "needles", label: "Needles" },
  { value: "orbit", label: "Orbit" },
  { value: "petals", label: "Petals" },
  { value: "petal_rain", label: "Petal Rain" },
  { value: "radar", label: "Radar" },
  { value: "pulse_rings", label: "Pulse Rings" },
  { value: "retro", label: "Equalizer Retro" },
  { value: "shards", label: "Shards" },
  { value: "skyline", label: "Skyline" },
  { value: "solid", label: "Solid" },
  { value: "capsule", label: "Capsule" },
  { value: "glow", label: "Glow" },
  { value: "prism", label: "Prism" },
  { value: "tuner", label: "Tuner" },
  { value: "vinyl", label: "Vinyl" },
];

const PREVIEW_STATES: Array<{ value: PreviewState; label: string }> = [
  { value: "recording", label: "Recording" },
  { value: "transcribing", label: "Processing" },
  { value: "error", label: "Error" },
];

export const RecordingOverlaySettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting, isUpdating, resetSetting, refreshSettings } =
    useSettings();
  const [previewState, setPreviewState] = React.useState<PreviewState>("recording");
  const [isResettingAppearance, setIsResettingAppearance] = React.useState(false);
  const [isResettingPosition, setIsResettingPosition] = React.useState(false);
  const [isApplyingPreset, setIsApplyingPreset] = React.useState(false);
  const [isApplyingStyleCode, setIsApplyingStyleCode] = React.useState(false);
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
  const rawBarCount = Number((settings as any)?.recording_overlay_bar_count ?? 9);
  const rawBarWidthPx = Number(
    (settings as any)?.recording_overlay_bar_width_px ?? 6,
  );
  const barCount = Number.isFinite(rawBarCount) ? rawBarCount : 9;
  const barWidthPx = Number.isFinite(rawBarWidthPx) ? rawBarWidthPx : 6;
  const barStyle = normalizeRecordingOverlayBarStyle(
    (settings as any)?.recording_overlay_bar_style,
  );
  const accentColor = normalizeRecordingOverlayColor(
    (settings as any)?.recording_overlay_accent_color,
  );
  const showDragGrip = Boolean(
    (settings as any)?.recording_overlay_show_drag_grip ?? false,
  );
  const audioReactiveScale = Boolean(
    (settings as any)?.recording_overlay_audio_reactive_scale ?? false,
  );
  const audioReactiveScaleMaxPercent = Number(
    (settings as any)?.recording_overlay_audio_reactive_scale_max_percent ?? 12,
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
  const hasManualPosition = Boolean(
    (settings as any)?.recording_overlay_use_manual_position ?? false,
  );
  const currentStyleConfig = React.useMemo(
    () => getRecordingOverlayStyleConfigFromSettings(settings),
    [settings],
  );
  const currentStyleCode = React.useMemo(
    () => serializeRecordingOverlayStyleConfig(currentStyleConfig),
    [currentStyleConfig],
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
      for (const key of [
        "recording_overlay_theme",
        "recording_overlay_background_mode",
        "recording_overlay_material_mode",
        "recording_overlay_centerpiece_mode",
        "recording_overlay_animated_border_mode",
        "recording_overlay_show_status_icon",
        "recording_overlay_bar_count",
        "recording_overlay_bar_width_px",
        "recording_overlay_bar_style",
        "recording_overlay_accent_color",
        "recording_overlay_show_drag_grip",
        "recording_overlay_audio_reactive_scale",
        "recording_overlay_audio_reactive_scale_max_percent",
        "recording_overlay_animation_softness_percent",
        "recording_overlay_depth_parallax_percent",
        "recording_overlay_opacity_percent",
        "recording_overlay_silence_fade",
        "recording_overlay_silence_opacity_percent",
      ] as const) {
        await resetSetting(key as any);
      }
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

  return (
    <SettingsGroup
      title={t(
        "settings.userInterface.recordingOverlay.title",
        "Recording Overlay",
      )}
    >
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
          <div className="flex flex-wrap gap-2">
            {PREVIEW_STATES.map((option) => {
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
          <RecordingOverlayPreview
            theme={overlayTheme}
            accentColor={accentColor}
            materialMode={materialMode}
            showStatusIcon={showStatusIcon}
            backgroundMode={backgroundMode}
            centerpieceMode={centerpieceMode}
            animatedBorderMode={animatedBorderMode}
            barCount={barCount}
            barWidthPx={barWidthPx}
            barStyle={barStyle}
            showDragGrip={showDragGrip}
            state={previewState}
            audioReactiveScale={audioReactiveScale}
            audioReactiveScaleMaxPercent={audioReactiveScaleMaxPercent}
            animationSoftnessPercent={animationSoftnessPercent}
            depthParallaxPercent={depthParallaxPercent}
            opacityPercent={opacityPercent}
            silenceFade={silenceFade}
            silenceOpacityPercent={silenceOpacityPercent}
          />
        </div>
      </SettingContainer>

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
          <div className="grid gap-3 xl:grid-cols-2">
            {RECORDING_OVERLAY_STYLE_PRESETS.map((preset) => {
              const presetConfig = {
                ...DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG,
                ...preset.config,
              };
              return (
              <button
                key={preset.id}
                type="button"
                onClick={() => void handleApplyPreset(presetConfig)}
                disabled={isApplyingPreset}
                className="rounded-xl border border-[#323232] bg-[#191919] p-3 text-left transition-colors hover:border-[#4a4a4a] hover:bg-[#202020] disabled:cursor-not-allowed disabled:opacity-45"
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
                  <span className="rounded-full border border-[#4a4a4a] bg-[#232323] px-2 py-0.5 text-[10px] font-medium uppercase tracking-[0.16em] text-[#ff8ebb]">
                    Apply
                  </span>
                </div>
                <RecordingOverlayPreview
                  theme={presetConfig.theme}
                  accentColor={presetConfig.accentColor}
                  materialMode={presetConfig.materialMode}
                  showStatusIcon={presetConfig.showStatusIcon}
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
                  animationSoftnessPercent={
                    presetConfig.animationSoftnessPercent
                  }
                  depthParallaxPercent={presetConfig.depthParallaxPercent}
                  opacityPercent={presetConfig.opacityPercent}
                  silenceFade={presetConfig.silenceFade}
                  silenceOpacityPercent={presetConfig.silenceOpacityPercent}
                />
              </button>
              );
            })}
          </div>
          {styleToolsStatus && (
            <div className="text-xs text-[#ffb6cf]">{styleToolsStatus}</div>
          )}
        </div>
      </SettingContainer>

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
          options={THEME_OPTIONS}
          selectedValue={overlayTheme}
          onSelect={(value) =>
            void updateSetting("recording_overlay_theme" as any, value as any)
          }
          disabled={isUpdating("recording_overlay_theme")}
        />
      </SettingContainer>

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
          options={MATERIAL_MODE_OPTIONS}
          selectedValue={materialMode}
          onSelect={(value) =>
            void updateSetting("recording_overlay_material_mode" as any, value as any)
          }
          disabled={isUpdating("recording_overlay_material_mode")}
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
          options={BACKGROUND_MODE_OPTIONS}
          selectedValue={backgroundMode}
          onSelect={(value) =>
            void updateSetting(
              "recording_overlay_background_mode" as any,
              value as any,
            )
          }
          disabled={isUpdating("recording_overlay_background_mode")}
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
          options={CENTERPIECE_MODE_OPTIONS}
          selectedValue={centerpieceMode}
          onSelect={(value) =>
            void updateSetting("recording_overlay_centerpiece_mode" as any, value as any)
          }
          disabled={isUpdating("recording_overlay_centerpiece_mode")}
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
          options={ANIMATED_BORDER_MODE_OPTIONS}
          selectedValue={animatedBorderMode}
          onSelect={(value) =>
            void updateSetting(
              "recording_overlay_animated_border_mode" as any,
              value as any,
            )
          }
          disabled={isUpdating("recording_overlay_animated_border_mode")}
        />
      </SettingContainer>

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
          options={BAR_STYLE_OPTIONS}
          selectedValue={barStyle}
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
        value={Math.max(3, Math.min(16, Math.round(barCount)))}
        formatValue={(value) => String(Math.round(value))}
        onChange={(value) =>
          void updateSetting(
            "recording_overlay_bar_count" as any,
            Math.round(value) as any,
          )
        }
        disabled={isUpdating("recording_overlay_bar_count")}
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
        value={Math.max(2, Math.min(12, Math.round(barWidthPx)))}
        formatValue={(value) => `${Math.round(value)} px`}
        onChange={(value) =>
          void updateSetting(
            "recording_overlay_bar_width_px" as any,
            Math.round(value) as any,
          )
        }
        disabled={isUpdating("recording_overlay_bar_width_px")}
      />

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
        value={Math.max(0, Math.min(24, Math.round(audioReactiveScaleMaxPercent)))}
        formatValue={(value) => `${Math.round(value)} %`}
        onChange={(value) =>
          void updateSetting(
            "recording_overlay_audio_reactive_scale_max_percent" as any,
            Math.round(value) as any,
          )
        }
        disabled={
          isUpdating("recording_overlay_audio_reactive_scale_max_percent") ||
          !audioReactiveScale
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
        value={Math.max(0, Math.min(100, Math.round(animationSoftnessPercent)))}
        formatValue={(value) => `${Math.round(value)} %`}
        onChange={(value) =>
          void updateSetting(
            "recording_overlay_animation_softness_percent" as any,
            Math.round(value) as any,
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
        value={Math.max(0, Math.min(100, Math.round(depthParallaxPercent)))}
        formatValue={(value) => `${Math.round(value)} %`}
        onChange={(value) =>
          void updateSetting(
            "recording_overlay_depth_parallax_percent" as any,
            Math.round(value) as any,
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
        value={Math.max(20, Math.min(100, Math.round(opacityPercent)))}
        formatValue={(value) => `${Math.round(value)} %`}
        onChange={(value) =>
          void updateSetting(
            "recording_overlay_opacity_percent" as any,
            Math.round(value) as any,
          )
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
        value={Math.max(20, Math.min(100, Math.round(silenceOpacityPercent)))}
        formatValue={(value) => `${Math.round(value)} %`}
        onChange={(value) =>
          void updateSetting(
            "recording_overlay_silence_opacity_percent" as any,
            Math.round(value) as any,
          )
        }
        disabled={
          isUpdating("recording_overlay_silence_opacity_percent") ||
          !silenceFade
        }
      />
    </SettingsGroup>
  );
};
