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
import { TellMeMore } from "../../ui/TellMeMore";
import { useSettings } from "../../../hooks/useSettings";
import { ShowOverlay } from "../ShowOverlay";
import { ShowTrayIcon } from "../ShowTrayIcon";
import type { OSType } from "../../../lib/utils/keyboard";
import {
  formatPreviewHotkeyForDisplay,
  normalizePreviewHotkeyString,
} from "../../../lib/utils/previewHotkeys";
import { HotkeyCapture } from "../../ui/HotkeyCapture";

const SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MIN = 24;
const SONIOX_LIVE_PREVIEW_CURSOR_OFFSET_MAX = 320;
const SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MIN = -10000;
const SONIOX_LIVE_PREVIEW_CUSTOM_COORD_MAX = 10000;
const SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MIN = 320;
const SONIOX_LIVE_PREVIEW_CUSTOM_WIDTH_MAX = 2200;
const SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MIN = 100;
const SONIOX_LIVE_PREVIEW_CUSTOM_HEIGHT_MAX = 1400;

type PreviewActionButtonConfig = {
  id: "close" | "clear" | "flush" | "process" | "insert";
  title: string;
  description: string;
  hotkeyKey: string;
  showVisualKey?: string;
};

const PREVIEW_ACTION_BUTTON_CONFIGS: PreviewActionButtonConfig[] = [
  {
    id: "close",
    title: "Close (X)",
    description: "Hotkey for the X button. = Full cancel, discard all text, do not insert. This button is always visible.",
    hotkeyKey: "soniox_live_preview_close_hotkey",
  },
  {
    id: "clear",
    title: "Clear all",
    description: "Clears preview text without ending the preview workflow.",
    hotkeyKey: "soniox_live_preview_clear_hotkey",
    showVisualKey: "soniox_live_preview_show_clear_button",
  },
  {
    id: "flush",
    title: "Flush",
    description: "Insert current preview text and keep the workflow running.",
    hotkeyKey: "soniox_live_preview_flush_hotkey",
    showVisualKey: "soniox_live_preview_show_flush_button",
  },
  {
    id: "process",
    title: "Processing via LLM",
    description: "Run manual LLM processing on finalized preview text.",
    hotkeyKey: "soniox_live_preview_process_hotkey",
    showVisualKey: "soniox_live_preview_show_process_button",
  },
  {
    id: "insert",
    title: "Insert",
    description: "Insert text and finish the preview workflow.",
    hotkeyKey: "soniox_live_preview_insert_hotkey",
    showVisualKey: "soniox_live_preview_show_insert_button",
  },
];

function clampToRange(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, Math.round(value)));
}

export const UserInterfaceSettings: React.FC = () => {
  const { settings, updateSetting, isUpdating } = useSettings();
  const osKind = type();
  const isWindows = osKind === "windows";
  const hotkeyOsType: OSType =
    osKind === "windows" || osKind === "macos" || osKind === "linux"
      ? osKind
      : "unknown";
  const [capturingPreviewHotkeyKey, setCapturingPreviewHotkeyKey] = React.useState<
    string | null
  >(null);
  const [previewActionsExpanded, setPreviewActionsExpanded] = React.useState(false);

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

  const previewActionsSummary = React.useMemo(() => {
    const parts: string[] = [];
    for (const config of PREVIEW_ACTION_BUTTON_CONFIGS) {
      const raw = String((settings as any)?.[config.hotkeyKey] ?? "");
      const normalized = normalizePreviewHotkeyString(raw);
      if (normalized) {
        const display = formatPreviewHotkeyForDisplay(normalized, hotkeyOsType);
        parts.push(`${config.title}: ${display}`);
      }
    }
    return parts.length > 0 ? parts.join(";  ") : "None set";
  }, [settings, hotkeyOsType]);

  const clearPreviewHotkey = (settingKey: string) => {
    void updateSetting(settingKey as any, "" as any);
    if (capturingPreviewHotkeyKey === settingKey) {
      setCapturingPreviewHotkeyKey(null);
    }
  };

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
      console.error("Failed to open live preview demo window:", error);
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
        <SettingsGroup title="Live Preview">
          <div className="px-6 py-4">
            <TellMeMore title="Tell me more: Live Preview">
              <div className="space-y-3">
                <p>
                  <strong>Live Preview is your safe staging area before final insertion.</strong>
                  Instead of pasting text immediately into the target app, you can keep text in the preview window,
                  decide when to insert, and optionally run manual LLM cleanup.
                </p>

                <p>
                  <strong>Basic workflow:</strong>
                </p>
                <ol className="list-decimal list-inside space-y-1 ml-1 opacity-90">
                  <li>Enable <strong>Live Preview Window</strong> in this section.</li>
                  <li>Use <strong>Open Preview</strong> to verify position, size, and appearance.</li>
                  <li>Start dictation using your normal transcription shortcut.</li>
                  <li>Watch confirmed and draft text accumulate in the preview window.</li>
                  <li>Use <strong>Insert</strong> when you are ready to send text to the target app.</li>
                </ol>

                <div className="p-3 bg-[#1a1a1a] border border-[#333333] rounded-md space-y-2">
                  <p>
                    <strong>Global vs profile toggles (important):</strong>
                  </p>
                  <p>
                    <strong>Live Preview Window</strong> in this section is a global visibility toggle for regular preview usage and demo preview.
                  </p>
                  <p>
                    <strong>Output to Preview</strong> in Transcription Profiles is a workflow toggle. If Output to Preview is enabled for the active profile,
                    the preview workflow window is shown even when this global Live Preview Window toggle is OFF.
                  </p>
                  <p>
                    Profile Output to Preview value has priority over the global Output to Preview fallback.
                    The global Output to Preview value is used only when no specific profile value is being applied.
                  </p>
                </div>

                <div className="p-3 bg-[#1a1a1a] border border-[#333333] rounded-md space-y-2">
                  <p>
                    <strong>What each button does:</strong>
                  </p>
                  <p>
                    <strong>X:</strong> closes the preview action window. In output-to-preview workflow mode it also ends the workflow session.
                  </p>
                  <p>
                    <strong>Clear all:</strong> clears current preview text without inserting it.
                  </p>
                  <p>
                    <strong>Flush:</strong> available in non-realtime workflow. Inserts current preview text, clears it, and lets you continue.
                  </p>
                  <p>
                    <strong>Processing via LLM:</strong> manually rewrites finalized preview text using your LLM setup, then puts the result back into preview.
                  </p>
                  <p>
                    <strong>Insert:</strong> finalizes current text and pastes it using your selected paste method.
                  </p>
                </div>

                <div className="p-3 bg-[#1a1a1a] border border-[#333333] rounded-md space-y-2">
                  <p>
                    <strong>Realtime vs non-realtime behavior:</strong>
                  </p>
                  <p>
                    <strong>Realtime:</strong> words appear progressively while you speak. Flush is usually hidden.
                  </p>
                  <p>
                    <strong>Non-realtime:</strong> preview may stay empty during recording and fill after finalize/stop. Flush helps commit chunks without ending the session.
                  </p>
                </div>

                <p>
                  <strong>Hotkeys and visual buttons:</strong> you can assign hotkeys to every preview action. Hotkeys are empty by default.
                  If you hide a visual button, its hotkey still works if configured.
                </p>

                <p>
                  <strong>Practical recommendation:</strong> keep <strong>Insert</strong> visible and bind it to a comfortable shortcut.
                  This gives you fast final commit while still preserving manual review control.
                </p>
              </div>
            </TellMeMore>
          </div>
          <SettingContainer
            title="Live Preview Window"
            description="Warning: this changes how app inserts text! Preview window appears, then, text is first displayed here, only at the end of recording it is inserted into target application. Global toggle for preview window visibility across the app. Output to Preview workflow can still force its own preview window."
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
            title="Preview of the Preview Window"
            description="I heard you like previews? This appication has a preview for the preview window, so you can pen a resizable preview window that will help you adjust the looks."
            descriptionMode="inline"
            grouped={true}
          >
            <button
              type="button"
              onClick={handleOpenPreviewWindow}
              className="px-3 py-1.5 bg-[#2b2b2b] hover:bg-[#3c3c3c] border border-[#3c3c3c] rounded-lg text-xs text-gray-200 font-medium transition-colors"
            >
              Open Preview To See How it Looks
            </button>
          </SettingContainer>
          <div
            className={`px-6 py-4 ${!sonioxLivePreviewEnabled ? "opacity-50 pointer-events-none" : ""}`}
          >
            <button
              type="button"
              onClick={() => setPreviewActionsExpanded((prev) => !prev)}
              className="w-full flex items-center justify-between gap-2 text-left"
            >
              <div className="min-w-0">
                <h3 className="text-sm font-medium text-[#f2f2f2]">
                  Preview Action Buttons
                </h3>
                {previewActionsExpanded ? (
                  <p className="text-xs text-[#a0a0a0] mt-0.5">
                    Configure hotkeys and button visibility for Preview actions.
                    Hotkeys are empty by default.
                  </p>
                ) : (
                  <p className="text-xs text-[#707070] mt-0.5 truncate">
                    {previewActionsSummary}
                  </p>
                )}
              </div>
              <svg
                className={`w-4 h-4 text-[#707070] shrink-0 transition-transform duration-200 ${previewActionsExpanded ? "rotate-180" : ""}`}
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M19 9l-7 7-7-7"
                />
              </svg>
            </button>
            {previewActionsExpanded && (
            <div className="mt-3 w-full space-y-3">
              {PREVIEW_ACTION_BUTTON_CONFIGS.map((config) => {
                const hotkeyRaw = String((settings as any)?.[config.hotkeyKey] ?? "");
                const normalizedHotkey = normalizePreviewHotkeyString(hotkeyRaw);
                const showVisual = config.showVisualKey
                  ? Boolean((settings as any)?.[config.showVisualKey] ?? true)
                  : true;

                return (
                  <div
                    key={config.id}
                    className="rounded-md border border-[#303030] bg-[#151515]/70 px-3 py-2"
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="min-w-0">
                        <div className="text-sm font-semibold text-[#f2f2f2]">
                          {config.title}
                        </div>
                        <div className="text-xs text-[#a0a0a0] mt-0.5">
                          {config.description}
                        </div>
                      </div>
                      {config.showVisualKey && (
                        <div className="flex items-center gap-2 shrink-0">
                          <span className="text-[11px] text-[#a0a0a0]">
                            Show visual button
                          </span>
                          <ToggleSwitch
                            checked={showVisual}
                            onChange={(enabled) =>
                              void updateSetting(
                                config.showVisualKey as any,
                                enabled as any,
                              )
                            }
                            disabled={
                              !sonioxLivePreviewEnabled ||
                              isUpdating(config.showVisualKey as string)
                            }
                          />
                        </div>
                      )}
                    </div>
                    <div className="mt-2">
                      <HotkeyCapture
                        value={normalizedHotkey}
                        isCapturing={capturingPreviewHotkeyKey === config.hotkeyKey}
                        onStartCapture={() => setCapturingPreviewHotkeyKey(config.hotkeyKey)}
                        onCaptured={(hotkey) => {
                          void updateSetting(config.hotkeyKey as any, hotkey as any);
                          setCapturingPreviewHotkeyKey(null);
                        }}
                        onCancel={() => setCapturingPreviewHotkeyKey(null)}
                        onClear={() => clearPreviewHotkey(config.hotkeyKey)}
                        disabled={!sonioxLivePreviewEnabled || isUpdating(config.hotkeyKey)}
                        osType={hotkeyOsType}
                      />
                    </div>
                  </div>
                );
              })}
            </div>
            )}
          </div>
          <SettingContainer
            title="Live Preview Position"
            description="Choose where to place the live preview window."
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
            title="Live Preview Size"
            description="Set the size of the live preview window."
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
            title="Live Preview Theme"
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
            label="Live Preview Transparency"
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
            title="Live Preview Accent Color"
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
                  a new live preview session starts. The window appears above your cursor.
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
