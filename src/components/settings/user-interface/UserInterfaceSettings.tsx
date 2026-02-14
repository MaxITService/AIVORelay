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
import { useSettings } from "../../../hooks/useSettings";
import { ShowOverlay } from "../ShowOverlay";
import { ShowTrayIcon } from "../ShowTrayIcon";

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
  const sonioxLivePreviewSize =
    ((settings as any)?.soniox_live_preview_size ?? "medium") as string;
  const sonioxLivePreviewTheme =
    ((settings as any)?.soniox_live_preview_theme ?? "main_dark") as string;
  const sonioxLivePreviewOpacityPercent = Number(
    (settings as any)?.soniox_live_preview_opacity_percent ?? 88,
  );
  const sonioxLivePreviewFontColor =
    ((settings as any)?.soniox_live_preview_font_color ?? "#f5f5f5") as string;
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

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title="User Interface">
        <ShowTrayIcon descriptionMode="tooltip" grouped={true} />
        <ShowOverlay descriptionMode="tooltip" grouped={true} />
        {isWindows && (
          <>
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
              title="Soniox Live Preview Position"
              description="Choose where to place the Soniox live preview window."
              descriptionMode="inline"
              grouped={true}
              disabled={!sonioxLivePreviewEnabled}
            >
              <Dropdown
                options={[
                  { value: "bottom", label: "Bottom" },
                  { value: "top", label: "Top" },
                ]}
                selectedValue={sonioxLivePreviewPosition}
                onSelect={(value) =>
                  void updateSetting(
                    "soniox_live_preview_position" as any,
                    value as any,
                  )
                }
                disabled={
                  !sonioxLivePreviewEnabled ||
                  isUpdating("soniox_live_preview_position")
                }
              />
            </SettingContainer>
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
              title="Soniox Live Preview Font Color"
              description="Final text color in preview."
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
              label="Soniox Live Interim Text Opacity"
              description="Opacity of non-final (interim) text."
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
          </>
        )}
      </SettingsGroup>

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
