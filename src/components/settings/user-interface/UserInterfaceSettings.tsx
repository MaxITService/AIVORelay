import React from "react";
import { type } from "@tauri-apps/plugin-os";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { SettingContainer } from "../../ui/SettingContainer";
import { HandyShortcut } from "../HandyShortcut";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
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
