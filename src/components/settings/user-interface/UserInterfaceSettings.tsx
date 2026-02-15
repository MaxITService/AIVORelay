import React from "react";
import { type } from "@tauri-apps/plugin-os";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { SettingContainer } from "../../ui/SettingContainer";
import { HandyShortcut } from "../HandyShortcut";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { Dropdown } from "../../ui/Dropdown";
import { Slider } from "../../ui/Slider";
import { Input } from "../../ui/Input";
import { useSettings } from "../../../hooks/useSettings";
import { ShowOverlay } from "../ShowOverlay";
import { ShowTrayIcon } from "../ShowTrayIcon";

const SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN = 24;
const SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX = 320;
const SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN = -10000;
const SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX = 10000;
const SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN = 320;
const SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX = 2200;
const SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN = 100;
const SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX = 1400;

function clampToRange(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, Math.round(value)));
}

export const UserInterfaceSettings: React.FC = () => {
  const { settings, updateSetting, isUpdating } = useSettings();
  const isWindows = type() === "windows";

  const voiceButtonShowAotToggle =
    (settings as any)?.voice_button_show_aot_toggle ?? false;
  const voiceButtonSingleClickClose =
    (settings as any)?.voice_button_single_click_close ?? false;
  const sonioxLivePreviewEnabled =
    (settings as any)?.soniox_live_preview_enabled ?? true;
  const sonioxLivePreviewPosition =
    ((settings as any)?.soniox_live_preview_position ?? "bottom") as string;
  const sonioxLivePreviewCursorOffsetPx = Number(
    (settings as any)?.soniox_live_preview_cursor_offset_px ?? 96,
  );
  const sonioxLivePreviewCustomXPx = Number(
    (settings as any)?.soniox_live_preview_custom_x_px ?? 240,
  );
  const sonioxLivePreviewCustomYPx = Number(
    (settings as any)?.soniox_live_preview_custom_y_px ?? 120,
  );
  const sonioxLivePreviewSize =
    ((settings as any)?.soniox_live_preview_size ?? "medium") as string;
  const sonioxLivePreviewCustomWidthPx = Number(
    (settings as any)?.soniox_live_preview_custom_width_px ?? 760,
  );
  const sonioxLivePreviewCustomHeightPx = Number(
    (settings as any)?.soniox_live_preview_custom_height_px ?? 200,
  );
  const sonioxLivePreviewTheme =
    ((settings as any)?.soniox_live_preview_theme ?? "main_dark") as string;
  const sonioxLivePreviewOpacityPercent = Number(
    (settings as any)?.soniox_live_preview_opacity_percent ?? 88,
  );
  const sonioxLivePreviewFontColor =
    ((settings as any)?.soniox_live_preview_font_color ?? "#f5f5f5") as string;
  const sonioxLivePreviewInterimFontColor =
    ((settings as any)?.soniox_live_preview_interim_font_color ?? "#f5f5f5") as string;
  const sonioxLivePreviewAccentColor =
    ((settings as any)?.soniox_live_preview_accent_color ?? "#ff4d8d") as string;
  const sonioxLivePreviewInterimOpacityPercent = Number(
    (settings as any)?.soniox_live_preview_interim_opacity_percent ?? 58,
  );

  const handleSpawnVoiceButton = async () => {
    try {
      await invoke("spawn_voice_activation_button_window");
    } catch (error) {
      console.error("Failed to spawn voice activation button window:", error);
      toast.error(String(error));
    }
  };

  const updatePxSetting = (
    key: string,
    value: number,
    min: number,
    max: number,
  ) => {
    void updateSetting(key as any, clampToRange(value, min, max) as any);
  };

  const handleOpenPreviewWindow = async () => {
    try {
      await invoke("preview_soniox_live_preview_window");
    } catch (error) {
      console.error("Failed to open Soniox preview demo window:", error);
      const message =
        error instanceof Error
          ? error.message
          : typeof error === "string"
            ? error
            : "Failed to open preview window.";
      toast.error(message);
    }
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title="User Interface">
        <ShowTrayIcon descriptionMode="tooltip" grouped={true} />
        <ShowOverlay descriptionMode="tooltip" grouped={true} />
      </SettingsGroup>

      {isWindows && (
        <SettingsGroup title="Soniox Live Preview">
          <SettingContainer
            title="Soniox Live Preview Window"
            description="Show a separate visual window with Soniox live interim/final text updates."
            descriptionMode="inline"
            grouped={true}
          >
            <ToggleSwitch
              checked={sonioxLivePreviewEnabled}
              onChange={(enabled) =>
                void updateSetting(
                  "soniox_live_preview_enabled" as any,
                  enabled as any,
                )
              }
              disabled={isUpdating("soniox_live_preview_enabled")}
            />
          </SettingContainer>
          <SettingContainer
            title="Preview Window"
            description="Open a resizable preview window with example Confirmed Text + Live Draft."
            descriptionMode="inline"
            grouped={true}
          >
            <button
              type="button"
              onClick={handleOpenPreviewWindow}
              className="px-3 py-1.5 bg-[#2b2b2b] hover:bg-[#3c3c3c] border border-[#3c3c3c] rounded-lg text-xs text-gray-200 font-medium transition-colors"
            >
              Open Preview
            </button>
          </SettingContainer>
          <SettingContainer
            title="Soniox Live Preview Position"
            description="Choose where to place the Soniox live preview window."
            descriptionMode="inline"
            grouped={true}
          >
            <Dropdown
              options={[
                { value: "bottom", label: "Bottom" },
                { value: "top", label: "Top" },
                { value: "near_cursor", label: "Near Cursor (Dynamic)" },
                { value: "custom_xy", label: "Custom X/Y (px)" },
              ]}
              selectedValue={sonioxLivePreviewPosition}
              onSelect={(value) =>
                void updateSetting(
                  "soniox_live_preview_position" as any,
                  value as any,
                )
              }
            />
          </SettingContainer>
          <SettingContainer
            title="Cursor Distance (Dynamic Mode)"
            description="Vertical distance from cursor to preview window when using Near Cursor position."
            descriptionMode="inline"
            grouped={true}
            disabled={!sonioxLivePreviewEnabled || sonioxLivePreviewPosition !== "near_cursor"}
          >
            <div className="w-full flex items-center gap-3">
              <input
                type="range"
                min={SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN}
                max={SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX}
                step={1}
                value={clampToRange(
                  sonioxLivePreviewCursorOffsetPx,
                  SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN,
                  SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX,
                )}
                onChange={(event) =>
                  updatePxSetting(
                    "soniox_live_preview_cursor_offset_px",
                    Number.parseInt(event.target.value, 10),
                    SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN,
                    SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX,
                  )
                }
                disabled={
                  !sonioxLivePreviewEnabled ||
                  sonioxLivePreviewPosition !== "near_cursor" ||
                  isUpdating("soniox_live_preview_cursor_offset_px")
                }
                className="flex-grow h-2 rounded-full appearance-none cursor-pointer focus:outline-none focus:ring-2 focus:ring-[#ff4d8d]/40 disabled:opacity-40 disabled:cursor-not-allowed"
                style={{
                  background: `linear-gradient(to right, #ff4d8d ${
                    ((clampToRange(
                      sonioxLivePreviewCursorOffsetPx,
                      SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN,
                      SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX,
                    ) -
                      SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN) /
                      (SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX -
                        SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN)) *
                    100
                  }%, #333333 ${
                    ((clampToRange(
                      sonioxLivePreviewCursorOffsetPx,
                      SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN,
                      SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX,
                    ) -
                      SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN) /
                      (SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX -
                        SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN)) *
                    100
                  }%)`,
                }}
              />
              <Input
                type="number"
                min={SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN}
                max={SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX}
                step={1}
                value={clampToRange(
                  sonioxLivePreviewCursorOffsetPx,
                  SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN,
                  SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX,
                )}
                onChange={(event) => {
                  const parsed = Number.parseInt(event.target.value, 10);
                  if (Number.isNaN(parsed)) {
                    return;
                  }
                  updatePxSetting(
                    "soniox_live_preview_cursor_offset_px",
                    parsed,
                    SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN,
                    SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX,
                  );
                }}
                className="w-24"
                disabled={
                  !sonioxLivePreviewEnabled ||
                  sonioxLivePreviewPosition !== "near_cursor" ||
                  isUpdating("soniox_live_preview_cursor_offset_px")
                }
              />
            </div>
          </SettingContainer>
          {sonioxLivePreviewPosition === "custom_xy" && (
            <>
              <SettingContainer
                title="Custom X (px)"
                description="Absolute X screen coordinate of preview window."
                descriptionMode="inline"
                grouped={true}
                disabled={!sonioxLivePreviewEnabled}
              >
                <div className="w-full flex items-center gap-3">
                  <input
                    type="range"
                    min={SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN}
                    max={SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX}
                    step={1}
                    value={clampToRange(
                      sonioxLivePreviewCustomXPx,
                      SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                      SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                    )}
                    onChange={(event) =>
                      updatePxSetting(
                        "soniox_live_preview_custom_x_px",
                        Number.parseInt(event.target.value, 10),
                        SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                        SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                      )
                    }
                    disabled={
                      !sonioxLivePreviewEnabled ||
                      isUpdating("soniox_live_preview_custom_x_px")
                    }
                    className="flex-grow h-2 rounded-full appearance-none cursor-pointer focus:outline-none focus:ring-2 focus:ring-[#ff4d8d]/40 disabled:opacity-40 disabled:cursor-not-allowed"
                    style={{
                      background: `linear-gradient(to right, #ff4d8d ${
                        ((clampToRange(
                          sonioxLivePreviewCustomXPx,
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                        ) -
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN) /
                          (SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX -
                            SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN)) *
                        100
                      }%, #333333 ${
                        ((clampToRange(
                          sonioxLivePreviewCustomXPx,
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                        ) -
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN) /
                          (SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX -
                            SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN)) *
                        100
                      }%)`,
                    }}
                  />
                  <Input
                    type="number"
                    min={SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN}
                    max={SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX}
                    step={1}
                    value={clampToRange(
                      sonioxLivePreviewCustomXPx,
                      SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                      SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                    )}
                    onChange={(event) => {
                      const parsed = Number.parseInt(event.target.value, 10);
                      if (Number.isNaN(parsed)) {
                        return;
                      }
                      updatePxSetting(
                        "soniox_live_preview_custom_x_px",
                        parsed,
                        SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                        SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                      );
                    }}
                    className="w-24"
                    disabled={
                      !sonioxLivePreviewEnabled ||
                      isUpdating("soniox_live_preview_custom_x_px")
                    }
                  />
                </div>
              </SettingContainer>
              <SettingContainer
                title="Custom Y (px)"
                description="Absolute Y screen coordinate of preview window."
                descriptionMode="inline"
                grouped={true}
                disabled={!sonioxLivePreviewEnabled}
              >
                <div className="w-full flex items-center gap-3">
                  <input
                    type="range"
                    min={SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN}
                    max={SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX}
                    step={1}
                    value={clampToRange(
                      sonioxLivePreviewCustomYPx,
                      SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                      SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                    )}
                    onChange={(event) =>
                      updatePxSetting(
                        "soniox_live_preview_custom_y_px",
                        Number.parseInt(event.target.value, 10),
                        SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                        SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                      )
                    }
                    disabled={
                      !sonioxLivePreviewEnabled ||
                      isUpdating("soniox_live_preview_custom_y_px")
                    }
                    className="flex-grow h-2 rounded-full appearance-none cursor-pointer focus:outline-none focus:ring-2 focus:ring-[#ff4d8d]/40 disabled:opacity-40 disabled:cursor-not-allowed"
                    style={{
                      background: `linear-gradient(to right, #ff4d8d ${
                        ((clampToRange(
                          sonioxLivePreviewCustomYPx,
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                        ) -
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN) /
                          (SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX -
                            SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN)) *
                        100
                      }%, #333333 ${
                        ((clampToRange(
                          sonioxLivePreviewCustomYPx,
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                        ) -
                          SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN) /
                          (SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX -
                            SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN)) *
                        100
                      }%)`,
                    }}
                  />
                  <Input
                    type="number"
                    min={SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN}
                    max={SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX}
                    step={1}
                    value={clampToRange(
                      sonioxLivePreviewCustomYPx,
                      SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                      SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                    )}
                    onChange={(event) => {
                      const parsed = Number.parseInt(event.target.value, 10);
                      if (Number.isNaN(parsed)) {
                        return;
                      }
                      updatePxSetting(
                        "soniox_live_preview_custom_y_px",
                        parsed,
                        SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN,
                        SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX,
                      );
                    }}
                    className="w-24"
                    disabled={
                      !sonioxLivePreviewEnabled ||
                      isUpdating("soniox_live_preview_custom_y_px")
                    }
                  />
                </div>
              </SettingContainer>
            </>
          )}
          <SettingContainer
            title="Soniox Live Preview Size"
            description="Set the size of the Soniox live preview window."
            descriptionMode="inline"
            grouped={true}
            disabled={!sonioxLivePreviewEnabled}
          >
            <Dropdown
              options={[
                { value: "small", label: "Small" },
                { value: "medium", label: "Medium" },
                { value: "large", label: "Large" },
                { value: "custom", label: "Custom (px)" },
              ]}
              selectedValue={sonioxLivePreviewSize}
              onSelect={(value) =>
                void updateSetting("soniox_live_preview_size" as any, value as any)
              }
              disabled={
                !sonioxLivePreviewEnabled || isUpdating("soniox_live_preview_size")
              }
            />
          </SettingContainer>
          {sonioxLivePreviewSize === "custom" && (
            <>
              <SettingContainer
                title="Custom Width (px)"
                description="Manual window width in pixels."
                descriptionMode="inline"
                grouped={true}
                disabled={!sonioxLivePreviewEnabled}
              >
                <div className="w-full flex items-center gap-3">
                  <input
                    type="range"
                    min={SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN}
                    max={SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX}
                    step={1}
                    value={clampToRange(
                      sonioxLivePreviewCustomWidthPx,
                      SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN,
                      SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX,
                    )}
                    onChange={(event) =>
                      updatePxSetting(
                        "soniox_live_preview_custom_width_px",
                        Number.parseInt(event.target.value, 10),
                        SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN,
                        SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX,
                      )
                    }
                    disabled={
                      !sonioxLivePreviewEnabled ||
                      isUpdating("soniox_live_preview_custom_width_px")
                    }
                    className="flex-grow h-2 rounded-full appearance-none cursor-pointer focus:outline-none focus:ring-2 focus:ring-[#ff4d8d]/40 disabled:opacity-40 disabled:cursor-not-allowed"
                    style={{
                      background: `linear-gradient(to right, #ff4d8d ${
                        ((clampToRange(
                          sonioxLivePreviewCustomWidthPx,
                          SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN,
                          SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX,
                        ) -
                          SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN) /
                          (SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX -
                            SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN)) *
                        100
                      }%, #333333 ${
                        ((clampToRange(
                          sonioxLivePreviewCustomWidthPx,
                          SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN,
                          SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX,
                        ) -
                          SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN) /
                          (SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX -
                            SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN)) *
                        100
                      }%)`,
                    }}
                  />
                  <Input
                    type="number"
                    min={SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN}
                    max={SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX}
                    step={1}
                    value={clampToRange(
                      sonioxLivePreviewCustomWidthPx,
                      SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN,
                      SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX,
                    )}
                    onChange={(event) => {
                      const parsed = Number.parseInt(event.target.value, 10);
                      if (Number.isNaN(parsed)) {
                        return;
                      }
                      updatePxSetting(
                        "soniox_live_preview_custom_width_px",
                        parsed,
                        SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN,
                        SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX,
                      );
                    }}
                    className="w-24"
                    disabled={
                      !sonioxLivePreviewEnabled ||
                      isUpdating("soniox_live_preview_custom_width_px")
                    }
                  />
                </div>
              </SettingContainer>
              <SettingContainer
                title="Custom Height (px)"
                description="Manual window height in pixels."
                descriptionMode="inline"
                grouped={true}
                disabled={!sonioxLivePreviewEnabled}
              >
                <div className="w-full flex items-center gap-3">
                  <input
                    type="range"
                    min={SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN}
                    max={SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX}
                    step={1}
                    value={clampToRange(
                      sonioxLivePreviewCustomHeightPx,
                      SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN,
                      SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX,
                    )}
                    onChange={(event) =>
                      updatePxSetting(
                        "soniox_live_preview_custom_height_px",
                        Number.parseInt(event.target.value, 10),
                        SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN,
                        SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX,
                      )
                    }
                    disabled={
                      !sonioxLivePreviewEnabled ||
                      isUpdating("soniox_live_preview_custom_height_px")
                    }
                    className="flex-grow h-2 rounded-full appearance-none cursor-pointer focus:outline-none focus:ring-2 focus:ring-[#ff4d8d]/40 disabled:opacity-40 disabled:cursor-not-allowed"
                    style={{
                      background: `linear-gradient(to right, #ff4d8d ${
                        ((clampToRange(
                          sonioxLivePreviewCustomHeightPx,
                          SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN,
                          SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX,
                        ) -
                          SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN) /
                          (SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX -
                            SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN)) *
                        100
                      }%, #333333 ${
                        ((clampToRange(
                          sonioxLivePreviewCustomHeightPx,
                          SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN,
                          SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX,
                        ) -
                          SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN) /
                          (SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX -
                            SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN)) *
                        100
                      }%)`,
                    }}
                  />
                  <Input
                    type="number"
                    min={SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN}
                    max={SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX}
                    step={1}
                    value={clampToRange(
                      sonioxLivePreviewCustomHeightPx,
                      SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN,
                      SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX,
                    )}
                    onChange={(event) => {
                      const parsed = Number.parseInt(event.target.value, 10);
                      if (Number.isNaN(parsed)) {
                        return;
                      }
                      updatePxSetting(
                        "soniox_live_preview_custom_height_px",
                        parsed,
                        SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN,
                        SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX,
                      );
                    }}
                    className="w-24"
                    disabled={
                      !sonioxLivePreviewEnabled ||
                      isUpdating("soniox_live_preview_custom_height_px")
                    }
                  />
                </div>
              </SettingContainer>
            </>
          )}
          <SettingContainer
            title="Soniox Live Preview Theme"
            description="Use the app-matching theme by default, or switch to alternate palettes."
            descriptionMode="inline"
            grouped={true}
            disabled={!sonioxLivePreviewEnabled}
          >
            <Dropdown
              options={[
                { value: "main_dark", label: "Main App Dark" },
                { value: "ocean", label: "Ocean Glass" },
                { value: "light", label: "Light" },
              ]}
              selectedValue={sonioxLivePreviewTheme}
              onSelect={(value) =>
                void updateSetting("soniox_live_preview_theme" as any, value as any)
              }
              disabled={
                !sonioxLivePreviewEnabled ||
                isUpdating("soniox_live_preview_theme")
              }
            />
          </SettingContainer>
          <Slider
            label="Soniox Live Preview Transparency"
            description="Controls panel transparency."
            descriptionMode="inline"
            grouped={true}
            min={35}
            max={100}
            step={1}
            value={sonioxLivePreviewOpacityPercent}
            formatValue={(value) => `${Math.round(value)}%`}
            onChange={(value) =>
              void updateSetting(
                "soniox_live_preview_opacity_percent" as any,
                Math.round(value) as any,
              )
            }
            disabled={
              !sonioxLivePreviewEnabled ||
              isUpdating("soniox_live_preview_opacity_percent")
            }
          />
          <SettingContainer
            title="Confirmed Text Color"
            description="Color of text that is already confirmed and will not change."
            descriptionMode="inline"
            grouped={true}
            disabled={!sonioxLivePreviewEnabled}
          >
            <div className="flex items-center gap-3">
              <input
                type="color"
                value={sonioxLivePreviewFontColor}
                onChange={(event) =>
                  void updateSetting(
                    "soniox_live_preview_font_color" as any,
                    event.target.value as any,
                  )
                }
                disabled={
                  !sonioxLivePreviewEnabled ||
                  isUpdating("soniox_live_preview_font_color")
                }
                className="h-8 w-12 rounded border border-[#3c3c3c] bg-transparent disabled:opacity-40"
              />
              <span className="text-xs text-[#a0a0a0] font-mono">
                {sonioxLivePreviewFontColor}
              </span>
            </div>
          </SettingContainer>
          <SettingContainer
            title="Live Draft Color"
            description="Color of text that is still being refined and may change."
            descriptionMode="inline"
            grouped={true}
            disabled={!sonioxLivePreviewEnabled}
          >
            <div className="flex items-center gap-3">
              <input
                type="color"
                value={sonioxLivePreviewInterimFontColor}
                onChange={(event) =>
                  void updateSetting(
                    "soniox_live_preview_interim_font_color" as any,
                    event.target.value as any,
                  )
                }
                disabled={
                  !sonioxLivePreviewEnabled ||
                  isUpdating("soniox_live_preview_interim_font_color")
                }
                className="h-8 w-12 rounded border border-[#3c3c3c] bg-transparent disabled:opacity-40"
              />
              <span className="text-xs text-[#a0a0a0] font-mono">
                {sonioxLivePreviewInterimFontColor}
              </span>
            </div>
          </SettingContainer>
          <SettingContainer
            title="Soniox Live Preview Accent Color"
            description="Header and accent color."
            descriptionMode="inline"
            grouped={true}
            disabled={!sonioxLivePreviewEnabled}
          >
            <div className="flex items-center gap-3">
              <input
                type="color"
                value={sonioxLivePreviewAccentColor}
                onChange={(event) =>
                  void updateSetting(
                    "soniox_live_preview_accent_color" as any,
                    event.target.value as any,
                  )
                }
                disabled={
                  !sonioxLivePreviewEnabled ||
                  isUpdating("soniox_live_preview_accent_color")
                }
                className="h-8 w-12 rounded border border-[#3c3c3c] bg-transparent disabled:opacity-40"
              />
              <span className="text-xs text-[#a0a0a0] font-mono">
                {sonioxLivePreviewAccentColor}
              </span>
            </div>
          </SettingContainer>
          <Slider
            label="Live Draft Opacity"
            description="How faded the Live Draft text appears before it becomes confirmed."
            descriptionMode="inline"
            grouped={true}
            min={20}
            max={95}
            step={1}
            value={sonioxLivePreviewInterimOpacityPercent}
            formatValue={(value) => `${Math.round(value)}%`}
            onChange={(value) =>
              void updateSetting(
                "soniox_live_preview_interim_opacity_percent" as any,
                Math.round(value) as any,
              )
            }
            disabled={
              !sonioxLivePreviewEnabled ||
              isUpdating("soniox_live_preview_interim_opacity_percent")
            }
          />
          <div className="px-6 py-3 border-t border-white/[0.05]">
            <details className="group">
              <summary className="flex items-center gap-2 text-sm text-[#9b5de5] hover:text-[#b47eff] transition-colors cursor-pointer list-none">
                <span>Positioning Help</span>
                <span className="text-xs text-[#707070] group-open:hidden">(expand)</span>
                <span className="text-xs text-[#707070] hidden group-open:inline">(collapse)</span>
              </summary>
              <div className="mt-3 p-4 bg-[#1a1a1a] rounded-lg border border-[#333333] text-sm text-[#b8b8b8] space-y-2">
                <p>
                  <strong className="text-[#f5f5f5]">Near Cursor (Dynamic)</strong> repositions the preview every time
                  a new Soniox live session starts. The window appears above your cursor.
                </p>
                <p>
                  <strong className="text-[#f5f5f5]">Custom X/Y (px)</strong> pins the window to exact screen coordinates.
                </p>
                <p>
                  Use <strong className="text-[#f5f5f5]">Cursor Distance</strong> to control how far above the cursor
                  the preview should appear.
                </p>
                <p>
                  If there is not enough space near screen edges, the app keeps the window inside the active monitor.
                </p>
              </div>
            </details>
          </div>
          <div className="px-6 py-3 border-t border-white/[0.05]">
            <details className="group">
              <summary className="flex items-center gap-2 text-sm text-[#9b5de5] hover:text-[#b47eff] transition-colors cursor-pointer list-none">
                <span>Appearance Help</span>
                <span className="text-xs text-[#707070] group-open:hidden">(expand)</span>
                <span className="text-xs text-[#707070] hidden group-open:inline">(collapse)</span>
              </summary>
              <div className="mt-3 p-4 bg-[#1a1a1a] rounded-lg border border-[#333333] text-sm text-[#b8b8b8] space-y-2">
                <p>
                  <strong className="text-[#f5f5f5]">Transparency</strong> controls panel background opacity.
                </p>
                <p>
                  <strong className="text-[#f5f5f5]">Confirmed Text Color</strong> affects stable text that will not change.
                </p>
                <p>
                  <strong className="text-[#f5f5f5]">Live Draft Color</strong> affects text that may still change.
                </p>
                <p>
                  <strong className="text-[#f5f5f5]">Live Draft Opacity</strong> controls how faded the draft text looks.
                </p>
                <p>
                  <strong className="text-[#f5f5f5]">Accent Color</strong> changes the header/accent tone.
                </p>
                <p>
                  Draft text is replaced by confirmed text as recognition stabilizes.
                </p>
              </div>
            </details>
          </div>
        </SettingsGroup>
      )}

      {isWindows && (
        <SettingsGroup title="Voice Activation Button">
          <SettingContainer
            title="Spawn Voice Activation Button"
            description="Open a floating on-screen voice activation button window."
            descriptionMode="inline"
            grouped={true}
          >
            <button
              type="button"
              onClick={handleSpawnVoiceButton}
              className="px-3 py-1.5 bg-[#2b2b2b] hover:bg-[#3c3c3c] border border-[#3c3c3c] rounded-lg text-xs text-gray-200 font-medium transition-colors"
            >
              Spawn button
            </button>
          </SettingContainer>
          <SettingContainer
            title="Show AOT Toggle in Button Window"
            description="Show the bottom always-on-top control inside the floating voice button window."
            descriptionMode="inline"
            grouped={true}
          >
            <ToggleSwitch
              checked={voiceButtonShowAotToggle}
              onChange={(enabled) =>
                void updateSetting(
                  "voice_button_show_aot_toggle" as any,
                  enabled as any,
                )
              }
              disabled={isUpdating("voice_button_show_aot_toggle")}
            />
          </SettingContainer>
          <SettingContainer
            title="Pressing x once, not twice closes the window"
            description="When enabled, one click on the x button closes the floating voice button window."
            descriptionMode="inline"
            grouped={true}
          >
            <ToggleSwitch
              checked={voiceButtonSingleClickClose}
              onChange={(enabled) =>
                void updateSetting(
                  "voice_button_single_click_close" as any,
                  enabled as any,
                )
              }
              disabled={isUpdating("voice_button_single_click_close")}
            />
          </SettingContainer>
          <HandyShortcut shortcutId="spawn_button" grouped={true} />
        </SettingsGroup>
      )}
    </div>
  );
};
