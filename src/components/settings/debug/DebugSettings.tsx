import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { type } from "@tauri-apps/plugin-os";
import { AlertTriangle } from "lucide-react";
import { WordCorrectionThreshold } from "./WordCorrectionThreshold";
import { LogDirectory } from "./LogDirectory";
import { LogLevelSelector } from "./LogLevelSelector";
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

export const DebugSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating, settings } = useSettings();
  const pushToTalk = getSetting("push_to_talk");
  const isLinux = type() === "linux";
  const isWindows = type() === "windows";

  // Modal states
  const [showVoiceCommandsWarning, setShowVoiceCommandsWarning] = useState(false);

  const betaVoiceCommandsEnabled = (settings as any)?.beta_voice_commands_enabled ?? false;

  const handleVoiceCommandsToggle = (enabled: boolean) => {
    if (enabled) {
      setShowVoiceCommandsWarning(true);
    } else {
      void updateSetting("beta_voice_commands_enabled" as any, false);
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
        <WordCorrectionThreshold descriptionMode="tooltip" grouped={true} />
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
          </>
        )}
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
