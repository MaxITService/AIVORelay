import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { type } from "@tauri-apps/plugin-os";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle } from "lucide-react";
import { toast } from "sonner";
import { LogDirectory } from "./LogDirectory";
import { LogLevelSelector } from "./LogLevelSelector";
import { ShortcutEngineSelector } from "./ShortcutEngineSelector";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { SettingContainer } from "../../ui/SettingContainer";
import { HistoryLimit } from "../HistoryLimit";
import { AlwaysOnMicrophone } from "../AlwaysOnMicrophone";
import { SoundPicker } from "../SoundPicker";
import { MuteWhileRecording } from "../MuteWhileRecording";
import { AppendTrailingSpace } from "../AppendTrailingSpace";
import { RecordingRetentionPeriodSelector } from "../RecordingRetentionPeriod";
import { ClamshellMicrophoneSelector } from "../ClamshellMicrophoneSelector";
import { HandyShortcut } from "../HandyShortcut";
import { UpdateChecksToggle } from "../UpdateChecksToggle";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { ConfirmationModal } from "../../ui/ConfirmationModal";
import { useSettings } from "../../../hooks/useSettings";
import { OPEN_FIRST_START_WIZARD_EVENT } from "../../../constants/appEvents";

export const DebugSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating, settings } = useSettings();
  const pushToTalk = getSetting("push_to_talk");
  const isLinux = type() === "linux";
  const isWindows = type() === "windows";

  // Modal states
  const [showVoiceCommandsWarning, setShowVoiceCommandsWarning] = useState(false);

  const betaVoiceCommandsEnabled = (settings as any)?.beta_voice_commands_enabled ?? false;
  const voiceButtonShowAotToggle =
    (settings as any)?.voice_button_show_aot_toggle ?? false;

  const handleVoiceCommandsToggle = (enabled: boolean) => {
    if (enabled) {
      setShowVoiceCommandsWarning(true);
    } else {
      void updateSetting("beta_voice_commands_enabled" as any, false);
    }
  };

  const handleOpenFirstStartWizard = () => {
    window.dispatchEvent(new Event(OPEN_FIRST_START_WIZARD_EVENT));
  };

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
      <SettingsGroup title={t("settings.debug.title")}>
        <LogDirectory grouped={true} />
        <LogLevelSelector grouped={true} />
        <UpdateChecksToggle descriptionMode="tooltip" grouped={true} />
        <SoundPicker
          label={t("settings.debug.soundTheme.label")}
          description={t("settings.debug.soundTheme.description")}
        />
        <HistoryLimit descriptionMode="tooltip" grouped={true} />
        <RecordingRetentionPeriodSelector
          descriptionMode="tooltip"
          grouped={true}
        />
        <AlwaysOnMicrophone descriptionMode="tooltip" grouped={true} />
        <ClamshellMicrophoneSelector descriptionMode="tooltip" grouped={true} />
        <MuteWhileRecording descriptionMode="tooltip" grouped={true} />
        <AppendTrailingSpace descriptionMode="tooltip" grouped={true} />
        {/* Cancel shortcut is disabled on Linux due to instability with dynamic shortcut registration */}
        {!isLinux && (
          <HandyShortcut
            shortcutId="cancel"
            grouped={true}
            disabled={pushToTalk}
          />
        )}
      </SettingsGroup>

      {/* Beta Features Section */}
      <SettingsGroup title="Experimental Features">
        <div className="px-4 py-3 mb-2 bg-yellow-500/10 border border-yellow-500/30 rounded-lg">
          <div className="flex items-start gap-2">
            <AlertTriangle className="w-4 h-4 text-yellow-400 mt-0.5 flex-shrink-0" />
            <p className="text-sm text-yellow-200/90">
              These features are experimental and may change or be removed in future versions.
            </p>
          </div>
        </div>



        {/* Voice Commands Toggle - Windows only */}
        {isWindows && (
          <>
            <SettingContainer
              title="Voice Commands"
              description="Execute scripts and commands using voice triggers"
              descriptionMode="inline"
              grouped={true}
            >
              <ToggleSwitch
                checked={betaVoiceCommandsEnabled}
                onChange={handleVoiceCommandsToggle}
                disabled={isUpdating("beta_voice_commands_enabled")}
              />
            </SettingContainer>
            {betaVoiceCommandsEnabled && (
              <div className="mx-4 mb-3 p-3 bg-red-500/10 border border-red-500/30 rounded-lg">
                <div className="flex items-start gap-2">
                  <AlertTriangle className="w-4 h-4 text-red-400 mt-0.5 flex-shrink-0" />
                  <div className="text-xs text-red-200/80">
                    <p className="font-semibold mb-1">⚠️ Advanced Users Only</p>
                    <p>
                      Voice Commands can execute <strong>any script or command</strong> on your computer.
                      Go to <strong>Voice Commands</strong> in the sidebar to configure.
                    </p>
                  </div>
                </div>
              </div>
            )}

            {/* Shortcut Engine Selector - Windows only */}
            <ShortcutEngineSelector />
          </>
        )}
      </SettingsGroup>

      <SettingsGroup title={t("settings.debug.tools.title")}>
        {isWindows && (
          <>
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
            <HandyShortcut shortcutId="spawn_button" grouped={true} />
          </>
        )}
        <SettingContainer
          title={t("settings.debug.firstStartWizard.title")}
          description={t("settings.debug.firstStartWizard.description")}
          descriptionMode="inline"
          grouped={true}
        >
          <button
            type="button"
            onClick={handleOpenFirstStartWizard}
            className="px-3 py-1.5 bg-[#2b2b2b] hover:bg-[#3c3c3c] border border-[#3c3c3c] rounded-lg text-xs text-gray-200 font-medium transition-colors"
          >
            {t("settings.debug.firstStartWizard.button")}
          </button>
        </SettingContainer>
      </SettingsGroup>

      {/* Confirmation Modal for Voice Commands */}
      <ConfirmationModal
        isOpen={showVoiceCommandsWarning}
        onClose={() => setShowVoiceCommandsWarning(false)}
        onConfirm={() => {
          void updateSetting("beta_voice_commands_enabled" as any, true);
        }}
        title="☢️ ENABLE AT YOUR OWN RISK ☢️"
        message="⚠️ EXTREME DANGER: Voice Commands is an experimental feature that executes arbitrary PowerShell scripts based on voice input. 💀 Malicious or incorrect triggers could PERMANENTLY WIPE YOUR DATA, RENDER YOUR SYSTEM COMPLETELY UNUSABLE, or CREATE BACKDOORS for hackers to silently control your PC and cause infinite harm. ☢️ This feature is intended for EXPERT DEVELOPERS ONLY. Do not enable this unless you are a PowerShell professional and fully comprehend the potentially catastrophic risks to your system and security. ☣️"
        confirmText="I AGREE, I TAKE THE RISK"
        cancelText="Cancel"
        variant="danger"
      />


    </div>
  );
};
