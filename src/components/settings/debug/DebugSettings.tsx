import React, { useState } from "react";
import { Trans, useTranslation } from "react-i18next";
import { type } from "@tauri-apps/plugin-os";
import { AlertTriangle } from "lucide-react";
import { LogDirectory } from "./LogDirectory";
import { SettingsDirectory } from "./SettingsDirectory";
import { LogLevelSelector } from "./LogLevelSelector";
import { DevConsoleLogLevelSelector } from "./DevConsoleLogLevelSelector";
import { ShortcutEngineSelector } from "./ShortcutEngineSelector";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { SettingContainer } from "../../ui/SettingContainer";
import { AlwaysOnMicrophone } from "../AlwaysOnMicrophone";
import { SoundPicker } from "../SoundPicker";
import { LazyStreamClose } from "./LazyStreamClose";
import { RecordingBuffer } from "./RecordingBuffer";
import { ClamshellMicrophoneSelector } from "../ClamshellMicrophoneSelector";
import { HandyShortcut } from "../HandyShortcut";
import { UpdateChecksToggle } from "../UpdateChecksToggle";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { ConfirmationModal } from "../../ui/ConfirmationModal";
import { useSettings } from "../../../hooks/useSettings";
import { OPEN_FIRST_START_WIZARD_EVENT } from "../../../constants/appEvents";
import { SessionToastHistory } from "./SessionToastHistory";

export const DebugSettings: React.FC = () => {
  const { t } = useTranslation();
  const {
    getSetting,
    updateSetting,
    isUpdating,
    settings,
    refreshSettings,
    updateRemoteSttUnsafeLogSecrets,
  } = useSettings();
  const pushToTalk = getSetting("push_to_talk");
  const isLinux = type() === "linux";
  const isWindows = type() === "windows";

  // Modal states
  const [showVoiceCommandsWarning, setShowVoiceCommandsWarning] =
    useState(false);
  const [showSecretLoggingWarning, setShowSecretLoggingWarning] =
    useState(false);

  const betaVoiceCommandsEnabled =
    (settings as any)?.beta_voice_commands_enabled ?? false;
  const unsafeLogSecrets = settings?.remote_stt?.unsafe_log_secrets ?? false;

  const handleVoiceCommandsToggle = (enabled: boolean) => {
    if (enabled) {
      setShowVoiceCommandsWarning(true);
    } else {
      void (async () => {
        await updateSetting("beta_voice_commands_enabled" as any, false);
        await refreshSettings();
      })();
    }
  };

  const handleOpenFirstStartWizard = () => {
    window.dispatchEvent(new Event(OPEN_FIRST_START_WIZARD_EVENT));
  };

  const handleSecretLoggingToggle = (enabled: boolean) => {
    if (enabled) {
      setShowSecretLoggingWarning(true);
    } else {
      void updateRemoteSttUnsafeLogSecrets(false);
    }
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SessionToastHistory />

      <SettingsGroup title={t("settings.debug.title")}>
        <SettingsDirectory grouped={true} />
        <LogDirectory grouped={true} />
        <LogLevelSelector grouped={true} />
        {import.meta.env.DEV && <DevConsoleLogLevelSelector grouped={true} />}
        <ToggleSwitch
          checked={unsafeLogSecrets}
          onChange={handleSecretLoggingToggle}
          isUpdating={isUpdating("remote_stt_unsafe_log_secrets")}
          label={t("settings.debug.unsafeSecretLogging.title")}
          description={t("settings.debug.unsafeSecretLogging.description")}
          descriptionMode="inline"
          grouped={true}
        />
        {unsafeLogSecrets && (
          <div className="mx-4 mb-3 p-3 bg-red-500/10 border border-red-500/40 rounded-lg">
            <div className="flex items-start gap-2">
              <AlertTriangle className="w-4 h-4 text-red-400 mt-0.5 flex-shrink-0" />
              <p className="text-xs font-semibold text-red-200/90">
                {t("settings.debug.unsafeSecretLogging.activeWarning")}
              </p>
            </div>
          </div>
        )}
        <UpdateChecksToggle descriptionMode="tooltip" grouped={true} />
        <SoundPicker
          label={t("settings.debug.soundTheme.label")}
          description={t("settings.debug.soundTheme.description")}
        />
        <AlwaysOnMicrophone descriptionMode="tooltip" grouped={true} />
        <ClamshellMicrophoneSelector descriptionMode="tooltip" grouped={true} />
        <RecordingBuffer descriptionMode="tooltip" grouped={true} />
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
      <SettingsGroup title={t("settings.debug.experimentalFeatures.title")}>
        <div className="px-4 py-3 mb-2 bg-yellow-500/10 border border-yellow-500/30 rounded-lg">
          <div className="flex items-start gap-2">
            <AlertTriangle className="w-4 h-4 text-yellow-400 mt-0.5 flex-shrink-0" />
            <p className="text-sm text-yellow-200/90">
              {t("settings.debug.experimentalFeatures.warning")}
            </p>
          </div>
        </div>

        {/* Voice Commands Toggle - Windows only */}
        {isWindows && (
          <>
            <LazyStreamClose descriptionMode="tooltip" grouped={true} />
            <SettingContainer
              title={t("settings.debug.voiceCommands.title")}
              description={t("settings.debug.voiceCommands.description")}
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
                    <p className="font-semibold mb-1">
                      {t("settings.debug.voiceCommands.warningTitle")}
                    </p>
                    <p>
                      <Trans
                        i18nKey="settings.debug.voiceCommands.warningMessage"
                        components={{ strong: <strong /> }}
                      />
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

      {/* Confirmation modals for dangerous debug features */}
      <ConfirmationModal
        isOpen={showSecretLoggingWarning}
        onClose={() => setShowSecretLoggingWarning(false)}
        onConfirm={() => {
          setShowSecretLoggingWarning(false);
          void updateRemoteSttUnsafeLogSecrets(true);
        }}
        title={t("settings.debug.unsafeSecretLogging.confirmationTitle")}
        message={t("settings.debug.unsafeSecretLogging.confirmationMessage")}
        confirmText={t("settings.debug.unsafeSecretLogging.confirm")}
        cancelText={t("settings.debug.unsafeSecretLogging.cancel")}
        variant="danger"
      />

      <ConfirmationModal
        isOpen={showVoiceCommandsWarning}
        onClose={() => setShowVoiceCommandsWarning(false)}
        onConfirm={() => {
          void updateSetting("beta_voice_commands_enabled" as any, true);
        }}
        title={t("settings.debug.voiceCommands.confirmationTitle")}
        message={t("settings.debug.voiceCommands.confirmationMessage")}
        confirmText={t("settings.debug.voiceCommands.confirm")}
        cancelText={t("settings.debug.voiceCommands.cancel")}
        variant="danger"
      />
    </div>
  );
};
