import React from "react";
import { useTranslation } from "react-i18next";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { SettingContainer } from "../../ui/SettingContainer";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { Dropdown } from "../../ui/Dropdown";
import { Slider } from "../../ui/Slider";
import { useSettings } from "../../../hooks/useSettings";
import { ShowOverlay } from "../ShowOverlay";
import { RecordingOverlayPreview } from "./RecordingOverlayPreview";
import type {
  RecordingOverlayBarStyle,
  RecordingOverlayTheme,
} from "@/bindings";
import {
  normalizeRecordingOverlayBarStyle,
  normalizeRecordingOverlayColor,
} from "../../../overlay/recordingOverlayAppearance";

type PreviewState = "recording" | "transcribing" | "error";

const THEME_OPTIONS: Array<{ value: RecordingOverlayTheme; label: string }> = [
  { value: "classic", label: "Classic" },
  { value: "minimal", label: "Minimal" },
  { value: "glass", label: "Glass" },
];

const BAR_STYLE_OPTIONS: Array<{ value: RecordingOverlayBarStyle; label: string }> = [
  { value: "solid", label: "Solid" },
  { value: "capsule", label: "Capsule" },
  { value: "glow", label: "Glow" },
  { value: "prism", label: "Prism" },
];

const PREVIEW_STATES: Array<{ value: PreviewState; label: string }> = [
  { value: "recording", label: "Recording" },
  { value: "transcribing", label: "Processing" },
  { value: "error", label: "Error" },
];

export const RecordingOverlaySettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting, isUpdating } = useSettings();
  const [previewState, setPreviewState] = React.useState<PreviewState>("recording");

  const overlayTheme =
    ((settings as any)?.recording_overlay_theme ?? "classic") as RecordingOverlayTheme;
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
            showStatusIcon={showStatusIcon}
            barCount={barCount}
            barWidthPx={barWidthPx}
            barStyle={barStyle}
            showDragGrip={showDragGrip}
            state={previewState}
          />
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
          "Bar Style",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.barStyle.description",
          "Choose the visual style of the recording meter bars.",
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
          "Bar Count",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.barCount.description",
          "Choose how many audio bars are shown while recording.",
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
          "Bar Width",
        )}
        description={t(
          "settings.userInterface.recordingOverlay.barWidth.description",
          "Adjust the thickness of the recording bars.",
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
          "Pick the accent color used for recording bars and hover accents.",
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
    </SettingsGroup>
  );
};
