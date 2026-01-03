import React, { useEffect, useState } from "react";
import { useTranslation, Trans } from "react-i18next";
import { Globe, Info, ExternalLink, Eye, EyeOff, Copy, AlertTriangle } from "lucide-react";
import { commands } from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { HandyShortcut } from "../HandyShortcut";
import { Input } from "../../ui/Input";
import { Select } from "../../ui/Select";
import { SettingContainer } from "../../ui/SettingContainer";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { Textarea } from "../../ui/Textarea";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { ConfirmationModal } from "../../ui/ConfirmationModal";
import { ConnectorStatusIndicator } from "./ConnectorStatus";

// Preset sites for auto-open dropdown (matches extension manifest)
const AUTO_OPEN_SITES = [
  { value: "https://chatgpt.com", label: "ChatGPT" },
  { value: "https://claude.ai", label: "Claude" },
  { value: "https://www.perplexity.ai", label: "Perplexity" },
  { value: "https://gemini.google.com", label: "Gemini" },
  { value: "https://grok.com", label: "Grok" },
  { value: "https://aistudio.google.com", label: "Google AI Studio" },
];

// Default screenshot folder for Windows
const getDefaultScreenshotFolder = () => {
  // This matches the Rust default: ShareX default folder
  return "%USERPROFILE%\\Documents\\ShareX\\Screenshots";
};

export const BrowserConnectorSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting, isUpdating, refreshSettings } = useSettings();

  const [portInput, setPortInput] = useState(String(settings?.connector_port ?? 38243));
  const [portError, setPortError] = useState<string | null>(null);
  const [passwordInput, setPasswordInput] = useState(settings?.connector_password ?? "");
  const [showPassword, setShowPassword] = useState(false);
  const [showCopiedTooltip, setShowCopiedTooltip] = useState(false);

  // Warning modal states for risky features
  const [showEnableWarning, setShowEnableWarning] = useState<
    "send_to_extension" | "send_to_extension_with_selection" | "send_screenshot_to_extension" | null
  >(null);

  // Screenshot settings local state
  const [screenshotCommandInput, setScreenshotCommandInput] = useState(
    settings?.screenshot_capture_command ?? '"C:\\Program Files\\ShareX\\ShareX.exe" -RectangleRegion'
  );
  const [screenshotFolderInput, setScreenshotFolderInput] = useState(
    settings?.screenshot_folder ?? getDefaultScreenshotFolder()
  );
  const [screenshotTimeoutInput, setScreenshotTimeoutInput] = useState(
    String(settings?.screenshot_timeout_seconds ?? 5)
  );
  
  // Textarea prompt local states (to prevent focus loss on each keystroke)
  const [selectionSystemPromptInput, setSelectionSystemPromptInput] = useState(
    settings?.send_to_extension_with_selection_system_prompt ?? ""
  );
  const [selectionUserPromptInput, setSelectionUserPromptInput] = useState(
    settings?.send_to_extension_with_selection_user_prompt ?? ""
  );
  const [selectionNoVoicePromptInput, setSelectionNoVoicePromptInput] = useState(
    settings?.send_to_extension_with_selection_no_voice_system_prompt ?? ""
  );
  const [screenshotNoVoicePromptInput, setScreenshotNoVoicePromptInput] = useState(
    settings?.screenshot_no_voice_default_prompt ?? ""
  );
  
  const [captureMethod, setCaptureMethod] = useState(
    (settings as any)?.screenshot_capture_method ?? "native"
  );
  const [nativeRegionCaptureMode, setNativeRegionCaptureMode] = useState(
    (settings as any)?.native_region_capture_mode ?? "live_desktop"
  );

  useEffect(() => {
    setPortInput(String(settings?.connector_port ?? 38243));
    setPortError(null); // Clear error when port updates successfully
  }, [settings?.connector_port]);

  useEffect(() => {
    setPasswordInput(settings?.connector_password ?? "");
  }, [settings?.connector_password]);

  // Screenshot settings sync with settings
  useEffect(() => {
    setScreenshotCommandInput(
      settings?.screenshot_capture_command ?? '"C:\\Program Files\\ShareX\\ShareX.exe" -RectangleRegion'
    );
  }, [settings?.screenshot_capture_command]);

  useEffect(() => {
    setScreenshotFolderInput(settings?.screenshot_folder ?? getDefaultScreenshotFolder());
  }, [settings?.screenshot_folder]);

  useEffect(() => {
    setScreenshotTimeoutInput(String(settings?.screenshot_timeout_seconds ?? 5));
  }, [settings?.screenshot_timeout_seconds]);

  // Sync prompt textarea local states with settings
  useEffect(() => {
    setSelectionSystemPromptInput(settings?.send_to_extension_with_selection_system_prompt ?? "");
  }, [settings?.send_to_extension_with_selection_system_prompt]);

  useEffect(() => {
    setSelectionUserPromptInput(settings?.send_to_extension_with_selection_user_prompt ?? "");
  }, [settings?.send_to_extension_with_selection_user_prompt]);

  useEffect(() => {
    setSelectionNoVoicePromptInput(settings?.send_to_extension_with_selection_no_voice_system_prompt ?? "");
  }, [settings?.send_to_extension_with_selection_no_voice_system_prompt]);

  useEffect(() => {
    setScreenshotNoVoicePromptInput(settings?.screenshot_no_voice_default_prompt ?? "");
  }, [settings?.screenshot_no_voice_default_prompt]);

  useEffect(() => {
    setCaptureMethod((settings as any)?.screenshot_capture_method ?? "native");
  }, [(settings as any)?.screenshot_capture_method]);

  useEffect(() => {
    setNativeRegionCaptureMode(
      (settings as any)?.native_region_capture_mode ?? "live_desktop"
    );
  }, [(settings as any)?.native_region_capture_mode]);


  const handlePortBlur = async () => {
    const port = parseInt(portInput.trim(), 10);
    const MIN_PORT = 1024;
    
    // Validate port range
    if (isNaN(port) || port < MIN_PORT || port > 65535) {
      setPortError(t("settings.browserConnector.connection.port.errorRange", { min: MIN_PORT }));
      return;
    }
    
    if (port === settings?.connector_port) {
      setPortError(null);
      return;
    }
    
    setPortError(null);
    try {
      const result = await commands.changeConnectorPortSetting(port);
      if (result.status === "error") {
        setPortError(result.error);
        // Revert input to current working port
        setPortInput(String(settings?.connector_port ?? 38243));
      } else {
        // Refresh settings to ensure the store is in sync
        await refreshSettings();
      }
    } catch (error) {
      setPortError(String(error));
      setPortInput(String(settings?.connector_port ?? 38243));
    }
  };

  const handlePasswordBlur = () => {
    const trimmed = passwordInput.trim();
    if (trimmed !== (settings?.connector_password ?? "")) {
      void updateSetting("connector_password", trimmed);
    }
  };

  const handleCopyPassword = () => {
    void navigator.clipboard.writeText(passwordInput);
    setShowCopiedTooltip(true);
    setTimeout(() => setShowCopiedTooltip(false), 1500);
  };

  // Check if using default password
  const isDefaultPassword = passwordInput === "fklejqwhfiu342lhk3";

  const handleAutoOpenEnabledChange = (enabled: boolean) => {
    void updateSetting("connector_auto_open_enabled", enabled);
    // Auto-select first site when enabling if no site is currently selected
    if (enabled && !settings?.connector_auto_open_url) {
      void updateSetting("connector_auto_open_url", AUTO_OPEN_SITES[0].value);
    }
  };

  const handleAutoOpenSiteChange = (url: string) => {
    void updateSetting("connector_auto_open_url", url);
  };

  // Screenshot settings handlers
  const handleScreenshotCommandBlur = () => {
    const trimmed = screenshotCommandInput.trim();
    if (trimmed !== (settings?.screenshot_capture_command ?? "")) {
      void updateSetting("screenshot_capture_command", trimmed);
    }
  };

  const handleScreenshotFolderBlur = () => {
    const trimmed = screenshotFolderInput.trim();
    if (trimmed !== (settings?.screenshot_folder ?? "")) {
      void updateSetting("screenshot_folder", trimmed);
    }
  };

  const handleScreenshotTimeoutBlur = () => {
    const timeout = parseInt(screenshotTimeoutInput.trim(), 10);
    if (!isNaN(timeout) && timeout > 0 && timeout !== settings?.screenshot_timeout_seconds) {
      void updateSetting("screenshot_timeout_seconds", timeout);
    }
  };

  // Prompt textarea blur handlers
  const handleSelectionSystemPromptBlur = () => {
    if (selectionSystemPromptInput !== (settings?.send_to_extension_with_selection_system_prompt ?? "")) {
      void updateSetting("send_to_extension_with_selection_system_prompt", selectionSystemPromptInput);
    }
  };

  const handleSelectionUserPromptBlur = () => {
    if (selectionUserPromptInput !== (settings?.send_to_extension_with_selection_user_prompt ?? "")) {
      void updateSetting("send_to_extension_with_selection_user_prompt", selectionUserPromptInput);
    }
  };

  const handleSelectionNoVoicePromptBlur = () => {
    if (selectionNoVoicePromptInput !== (settings?.send_to_extension_with_selection_no_voice_system_prompt ?? "")) {
      void updateSetting("send_to_extension_with_selection_no_voice_system_prompt", selectionNoVoicePromptInput);
    }
  };

  const handleScreenshotNoVoicePromptBlur = () => {
    if (screenshotNoVoicePromptInput !== (settings?.screenshot_no_voice_default_prompt ?? "")) {
      void updateSetting("screenshot_no_voice_default_prompt", screenshotNoVoicePromptInput);
    }
  };

  const handleCaptureMethodChange = (method: string) => {
    void updateSetting("screenshot_capture_method" as any, method);
    setCaptureMethod(method);
  };

  const handleNativeRegionCaptureModeChange = (mode: string) => {
    void updateSetting("native_region_capture_mode" as any, mode);
    setNativeRegionCaptureMode(mode);
  };

  const nativeRegionCaptureModeOptions = [
    {
      value: "live_desktop",
      label: t("settings.browserConnector.screenshot.nativeMode.liveDesktop"),
    },
    {
      value: "screenshot_background",
      label: t(
        "settings.browserConnector.screenshot.nativeMode.screenshotBackground"
      ),
    },
  ];


  // Server always binds to 127.0.0.1 and serves /messages
  const endpointUrl = `http://127.0.0.1:${portInput}/messages`;

  return (
    <div className="max-w-3xl w-full mx-auto space-y-8 pb-12">
      {/* Help Banner */}
      <div className="rounded-lg border border-purple-500/30 bg-purple-500/10 p-4">
        <div className="flex items-start gap-3">
          <Info className="w-5 h-5 text-purple-400 mt-0.5 flex-shrink-0" />
          <div className="space-y-2 text-sm text-text/80">
            <p className="font-medium text-text">
              {t("settings.browserConnector.help.title")}
            </p>
            <p>
              <Trans
                i18nKey="settings.browserConnector.help.description"
                components={{
                  link: (
                    <a
                      href="https://github.com/MaxITService/AivoRelay-relay"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-purple-400 hover:underline inline-flex items-center gap-1"
                    >
                      AivoRelay Connector
                      <ExternalLink className="w-3 h-3" />
                    </a>
                  ),
                }}
              />
            </p>
            <ul className="list-disc list-inside space-y-1 ml-1">
              <li>{t("settings.browserConnector.help.feature1")}</li>
              <li>{t("settings.browserConnector.help.feature2")}</li>
              <li>{t("settings.browserConnector.help.feature3")}</li>
            </ul>
            <div className="mt-4 p-3 rounded border border-yellow-500/30 bg-yellow-500/5 text-yellow-200/90 italic">
              <div className="flex gap-2">
                <AlertTriangle className="w-4 h-4 text-yellow-400 mt-0.5 flex-shrink-0" />
                <p>{t("settings.browserConnector.help.feature4")}</p>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Extension Status */}
      <SettingsGroup title={t("settings.browserConnector.status.sectionTitle")}>
        <ConnectorStatusIndicator grouped={true} descriptionMode="tooltip" />
      </SettingsGroup>

      {/* Feature 1: Send Transcription Directly to Extension */}
      <SettingsGroup 
        title={t("settings.general.shortcut.bindings.send_to_extension.name")}
        description={t("settings.general.shortcut.bindings.send_to_extension.userStory")}
      >
        <SettingContainer
          title={t("settings.general.shortcut.bindings.send_to_extension.enable.label")}
          description={t("settings.general.shortcut.bindings.send_to_extension.enable.description")}
          descriptionMode="tooltip"
          grouped={true}
        >
          <ToggleSwitch
            checked={settings?.send_to_extension_enabled ?? false}
            onChange={(enabled) => {
              if (enabled) {
                setShowEnableWarning("send_to_extension");
              } else {
                void updateSetting("send_to_extension_enabled", false);
              }
            }}
            disabled={isUpdating("send_to_extension_enabled")}
          />
        </SettingContainer>
        <div 
          className={`overflow-hidden transition-all duration-300 ease-out ${
            settings?.send_to_extension_enabled 
              ? "max-h-[500px] opacity-100" 
              : "max-h-0 opacity-0"
          }`}
        >
          <div className="border-t border-white/[0.05]">
            <HandyShortcut shortcutId="send_to_extension" grouped={true} />
            <SettingContainer
              title={t("settings.general.shortcut.bindings.send_to_extension.pushToTalk.label")}
              description={t("settings.general.shortcut.bindings.send_to_extension.pushToTalk.description")}
              descriptionMode="tooltip"
              grouped={true}
            >
              <ToggleSwitch
                checked={settings?.send_to_extension_push_to_talk ?? true}
                onChange={(enabled) => void updateSetting("send_to_extension_push_to_talk", enabled)}
                disabled={isUpdating("send_to_extension_push_to_talk")}
              />
            </SettingContainer>
          </div>
        </div>
      </SettingsGroup>

      {/* Feature 2: Send Transcription + Selection to Extension */}
      <SettingsGroup 
        title={t("settings.general.shortcut.bindings.send_to_extension_with_selection.name")}
        description={t("settings.general.shortcut.bindings.send_to_extension_with_selection.userStory")}
      >
        <SettingContainer
          title={t("settings.general.shortcut.bindings.send_to_extension_with_selection.enable.label")}
          description={t("settings.general.shortcut.bindings.send_to_extension_with_selection.enable.description")}
          descriptionMode="tooltip"
          grouped={true}
        >

          <ToggleSwitch
            checked={settings?.send_to_extension_with_selection_enabled ?? false}
            onChange={(enabled) => {
              if (enabled) {
                setShowEnableWarning("send_to_extension_with_selection");
              } else {
                void updateSetting("send_to_extension_with_selection_enabled", false);
              }
            }}
            disabled={isUpdating("send_to_extension_with_selection_enabled")}
          />
        </SettingContainer>
        <div 
          className={`overflow-hidden transition-all duration-300 ease-out ${
            settings?.send_to_extension_with_selection_enabled 
              ? "max-h-[2000px] opacity-100" 
              : "max-h-0 opacity-0"
          }`}
        >
          <div className="border-t border-white/[0.05]">
            <HandyShortcut shortcutId="send_to_extension_with_selection" grouped={true} />
            <SettingContainer
              title={t("settings.general.shortcut.bindings.send_to_extension_with_selection.pushToTalk.label")}
              description={t("settings.general.shortcut.bindings.send_to_extension_with_selection.pushToTalk.description")}
              descriptionMode="tooltip"
              grouped={true}
            >
              <ToggleSwitch
                checked={settings?.send_to_extension_with_selection_push_to_talk ?? true}
                onChange={(enabled) => void updateSetting("send_to_extension_with_selection_push_to_talk", enabled)}
                disabled={isUpdating("send_to_extension_with_selection_push_to_talk")}
              />
            </SettingContainer>
            
            {/* Prompt Templates - now inside feature block */}
            <div className="border-t border-white/[0.08] mt-2 pt-2">
              <div className="px-6 py-2 text-xs font-bold text-[#ff4d8d] uppercase tracking-widest">
                {t("settings.browserConnector.prompts.title")}
              </div>
              <SettingContainer
                title={t("settings.browserConnector.prompts.systemPrompt.title")}
                description={t("settings.browserConnector.prompts.systemPrompt.description")}
                descriptionMode="inline"
                grouped={true}
                layout="stacked"
              >
                <Textarea
                  value={selectionSystemPromptInput}
                  onChange={(event) => setSelectionSystemPromptInput(event.target.value)}
                  onBlur={handleSelectionSystemPromptBlur}
                  disabled={isUpdating("send_to_extension_with_selection_system_prompt")}
                  className="w-full"
                  rows={4}
                />
              </SettingContainer>
              <SettingContainer
                title={t("settings.browserConnector.prompts.userPrompt.title")}
                description={t("settings.browserConnector.prompts.userPrompt.description")}
                descriptionMode="inline"
                grouped={true}
                layout="stacked"
              >
                <Textarea
                  value={selectionUserPromptInput}
                  onChange={(event) => setSelectionUserPromptInput(event.target.value)}
                  onBlur={handleSelectionUserPromptBlur}
                  disabled={isUpdating("send_to_extension_with_selection_user_prompt")}
                  className="w-full"
                  rows={3}
                />
                <div className="text-xs text-text/50 mt-1">
                  {t("settings.aiReplace.withSelection.variables")}
                </div>
              </SettingContainer>
              <SettingContainer
                title={t("settings.browserConnector.prompts.quickTap.title")}
                description={t("settings.browserConnector.prompts.quickTap.description")}
                descriptionMode="tooltip"
                grouped={true}
              >
                <ToggleSwitch
                  checked={settings?.send_to_extension_with_selection_allow_no_voice ?? true}
                  onChange={(enabled) => void updateSetting("send_to_extension_with_selection_allow_no_voice", enabled)}
                  disabled={isUpdating("send_to_extension_with_selection_allow_no_voice")}
                />
              </SettingContainer>
              <div className={!settings?.send_to_extension_with_selection_allow_no_voice ? "opacity-50" : ""}>
                <SettingContainer
                  title={t("settings.browserConnector.prompts.quickTap.threshold.title")}
                  description={t("settings.browserConnector.prompts.quickTap.threshold.description")}
                  descriptionMode="tooltip"
                  grouped={true}
                >
                  <div className="flex items-center gap-2">
                    <Input
                      type="number"
                      value={settings?.send_to_extension_with_selection_quick_tap_threshold_ms ?? 500}
                      onChange={(event) => {
                        const val = parseInt(event.target.value, 10);
                        if (!isNaN(val) && val > 0) {
                          void updateSetting("send_to_extension_with_selection_quick_tap_threshold_ms", val);
                        }
                      }}
                      disabled={!settings?.send_to_extension_with_selection_allow_no_voice || isUpdating("send_to_extension_with_selection_quick_tap_threshold_ms")}
                      min={100}
                      max={2000}
                      step={50}
                      className="w-24"
                    />
                    <span className="text-sm text-text/60">
                      {t("settings.browserConnector.prompts.quickTap.threshold.suffix")}
                    </span>
                  </div>
                </SettingContainer>
                <SettingContainer
                  title={t("settings.browserConnector.prompts.quickTap.systemPrompt.title")}
                  description={t("settings.browserConnector.prompts.quickTap.systemPrompt.description")}
                  descriptionMode="inline"
                  grouped={true}
                  layout="stacked"
                >
                  <Textarea
                    value={selectionNoVoicePromptInput}
                    onChange={(event) => setSelectionNoVoicePromptInput(event.target.value)}
                    onBlur={handleSelectionNoVoicePromptBlur}
                    disabled={!settings?.send_to_extension_with_selection_allow_no_voice || isUpdating("send_to_extension_with_selection_no_voice_system_prompt")}
                    className="w-full"
                    rows={2}
                  />
                </SettingContainer>
              </div>
            </div>
          </div>
        </div>
      </SettingsGroup>


      {/* Feature 3: Send Transcription + Screenshot to Extension */}
      <SettingsGroup 
        title={t("settings.general.shortcut.bindings.send_screenshot_to_extension.name")}
        description={t("settings.general.shortcut.bindings.send_screenshot_to_extension.userStory")}
      >

        <SettingContainer
          title={t("settings.general.shortcut.bindings.send_screenshot_to_extension.enable.label")}
          description={t("settings.general.shortcut.bindings.send_screenshot_to_extension.enable.description")}
          descriptionMode="tooltip"
          grouped={true}
        >
          <ToggleSwitch
            checked={settings?.send_screenshot_to_extension_enabled ?? false}
            onChange={(enabled) => {
              if (enabled) {
                setShowEnableWarning("send_screenshot_to_extension");
              } else {
                void updateSetting("send_screenshot_to_extension_enabled", false);
              }
            }}
            disabled={isUpdating("send_screenshot_to_extension_enabled")}
          />
        </SettingContainer>
        <div 
          className={`overflow-hidden transition-all duration-300 ease-out ${
            settings?.send_screenshot_to_extension_enabled 
              ? "max-h-[2500px] opacity-100" 
              : "max-h-0 opacity-0"
          }`}
        >
          <div className="border-t border-white/[0.05]">
            <HandyShortcut shortcutId="send_screenshot_to_extension" grouped={true} />
            <SettingContainer
              title={t("settings.general.shortcut.bindings.send_screenshot_to_extension.pushToTalk.label")}
              description={t("settings.general.shortcut.bindings.send_screenshot_to_extension.pushToTalk.description")}
              descriptionMode="tooltip"
              grouped={true}
            >
              <ToggleSwitch
                checked={settings?.send_screenshot_to_extension_push_to_talk ?? true}
                onChange={(enabled) => void updateSetting("send_screenshot_to_extension_push_to_talk", enabled)}
                disabled={isUpdating("send_screenshot_to_extension_push_to_talk")}
              />
            </SettingContainer>
            
            {/* Screenshot Settings - now inside feature block */}
            <div className="border-t border-white/[0.08] mt-2 pt-2">
              <div className="px-6 py-2 text-xs font-bold text-[#ff4d8d] uppercase tracking-widest">
                {t("settings.browserConnector.screenshot.title")}
              </div>
              <div className="mx-6 mb-2 p-3 rounded border border-red-500/30 bg-red-500/10 text-red-200/90 text-sm italic">
                <div className="flex gap-2">
                  <AlertTriangle className="w-4 h-4 text-red-400 mt-0.5 flex-shrink-0" />
                  <p>{t("settings.browserConnector.screenshot.warning")}</p>
                </div>
              </div>
              <SettingContainer
                title={t("settings.browserConnector.screenshot.method.title")}
                description={t("settings.browserConnector.screenshot.method.description")}
                grouped={true}
              >
                <div className="flex flex-col gap-3">
                  <label className="flex items-center gap-3 cursor-pointer group">
                    <div className="relative flex items-center justify-center">
                      <input
                        type="radio"
                        name="capture_method"
                        value="external_program"
                        checked={captureMethod === "external_program"}
                        onChange={(e) => handleCaptureMethodChange(e.target.value)}
                        className="peer appearance-none w-4 h-4 rounded-full border border-gray-400 checked:border-purple-500 checked:bg-purple-500 transition-colors"
                      />
                      <div className="absolute w-2 h-2 rounded-full bg-white opacity-0 peer-checked:opacity-100 pointer-events-none transition-opacity" />
                    </div>
                    <span className="text-sm text-text/90 group-hover:text-text transition-colors">
                      {t("settings.browserConnector.screenshot.method.external")}
                    </span>
                  </label>
                  <label className="flex items-center gap-3 cursor-pointer group">
                    <div className="relative flex items-center justify-center">
                      <input
                        type="radio"
                        name="capture_method"
                        value="native"
                        checked={captureMethod === "native"}
                        onChange={(e) => handleCaptureMethodChange(e.target.value)}
                        className="peer appearance-none w-4 h-4 rounded-full border border-gray-400 checked:border-purple-500 checked:bg-purple-500 transition-colors"
                      />
                      <div className="absolute w-2 h-2 rounded-full bg-white opacity-0 peer-checked:opacity-100 pointer-events-none transition-opacity" />
                    </div>
                    <span className="text-sm text-text/90 group-hover:text-text transition-colors">
                      {t("settings.browserConnector.screenshot.method.native")}
                    </span>
                  </label>
                </div>
              </SettingContainer>

              {captureMethod === "native" && (
                <SettingContainer
                  title={t("settings.browserConnector.screenshot.nativeMode.title")}
                  description={t(
                    "settings.browserConnector.screenshot.nativeMode.description"
                  )}
                  grouped={true}
                >
                  <Select
                    value={nativeRegionCaptureMode}
                    options={nativeRegionCaptureModeOptions}
                    onChange={(value) =>
                      value && handleNativeRegionCaptureModeChange(value)
                    }
                    disabled={isUpdating("native_region_capture_mode")}
                    placeholder={t(
                      "settings.browserConnector.screenshot.nativeMode.placeholder"
                    )}
                    isClearable={false}
                    className="w-64"
                  />
                </SettingContainer>
              )}

              {captureMethod === "external_program" && (
                <>
              <SettingContainer
                title={t("settings.browserConnector.screenshot.command.title")}
                description={t("settings.browserConnector.screenshot.command.description")}
                descriptionMode="inline"
                grouped={true}
                layout="stacked"
              >
                <Input
                  type="text"
                  value={screenshotCommandInput}
                  onChange={(event) => setScreenshotCommandInput(event.target.value)}
                  onBlur={handleScreenshotCommandBlur}
                  placeholder='"C:\Program Files\ShareX\ShareX.exe" -RectangleRegion'
                  className="w-full font-mono text-sm"
                />
              </SettingContainer>
              <SettingContainer
                title={t("settings.browserConnector.screenshot.folder.title")}
                description={t("settings.browserConnector.screenshot.folder.description")}
                descriptionMode="inline"
                grouped={true}
                layout="stacked"
              >
                <Input
                  type="text"
                  value={screenshotFolderInput}
                  onChange={(event) => setScreenshotFolderInput(event.target.value)}
                  onBlur={handleScreenshotFolderBlur}
                  placeholder="%USERPROFILE%\Documents\ShareX\Screenshots"
                  className="w-full font-mono text-sm"
                />
              </SettingContainer>
              <SettingContainer
                title={t("settings.browserConnector.screenshot.includeSubfolders.title")}
                description={t("settings.browserConnector.screenshot.includeSubfolders.description")}
                descriptionMode="tooltip"
                grouped={true}
              >
                <ToggleSwitch
                  checked={settings?.screenshot_include_subfolders ?? false}
                  onChange={(enabled) => void updateSetting("screenshot_include_subfolders", enabled)}
                  disabled={isUpdating("screenshot_include_subfolders")}
                />
              </SettingContainer>
              <SettingContainer
                title={t("settings.browserConnector.screenshot.requireRecent.title")}
                description={t("settings.browserConnector.screenshot.requireRecent.description")}
                descriptionMode="tooltip"
                grouped={true}
              >
                <ToggleSwitch
                  checked={settings?.screenshot_require_recent ?? true}
                  onChange={(enabled) => void updateSetting("screenshot_require_recent", enabled)}
                  disabled={isUpdating("screenshot_require_recent")}
                />
              </SettingContainer>
              <div className={!settings?.screenshot_require_recent ? "opacity-50" : ""}>
                <SettingContainer
                  title={t("settings.browserConnector.screenshot.timeout.title")}
                  description={t("settings.browserConnector.screenshot.timeout.description")}
                  descriptionMode="tooltip"
                  grouped={true}
                >
                  <div className="flex items-center gap-2">
                    <Input
                      type="number"
                      value={screenshotTimeoutInput}
                      onChange={(event) => setScreenshotTimeoutInput(event.target.value)}
                      onBlur={handleScreenshotTimeoutBlur}
                      placeholder="5"
                      min={1}
                      max={60}
                      className="w-20"
                      disabled={!settings?.screenshot_require_recent}
                    />
                    <span className="text-sm text-text/60">
                      {t("settings.browserConnector.screenshot.timeout.unit")}
                    </span>
                  </div>
                </SettingContainer>
              </div>
                </>
              )}

              <SettingContainer
                title={t("settings.browserConnector.screenshot.quickTap.title")}
                description={t("settings.browserConnector.screenshot.quickTap.description")}
                descriptionMode="tooltip"
                grouped={true}
              >
                <ToggleSwitch
                  checked={settings?.screenshot_allow_no_voice ?? true}
                  onChange={(enabled) => void updateSetting("screenshot_allow_no_voice", enabled)}
                  disabled={isUpdating("screenshot_allow_no_voice")}
                />
              </SettingContainer>
              <div className={!settings?.screenshot_allow_no_voice ? "opacity-50" : ""}>
                <SettingContainer
                  title={t("settings.browserConnector.screenshot.quickTap.threshold.title")}
                  description={t("settings.browserConnector.screenshot.quickTap.threshold.description")}
                  descriptionMode="tooltip"
                  grouped={true}
                >
                  <div className="flex items-center gap-2">
                    <Input
                      type="number"
                      value={settings?.screenshot_quick_tap_threshold_ms ?? 500}
                      onChange={(event) => {
                        const val = parseInt(event.target.value, 10);
                        if (!isNaN(val) && val > 0) {
                          void updateSetting("screenshot_quick_tap_threshold_ms", val);
                        }
                      }}
                      disabled={!settings?.screenshot_allow_no_voice || isUpdating("screenshot_quick_tap_threshold_ms")}
                      min={100}
                      max={2000}
                      step={50}
                      className="w-24"
                    />
                    <span className="text-sm text-text/60">
                      {t("settings.browserConnector.screenshot.quickTap.threshold.suffix")}
                    </span>
                  </div>
                </SettingContainer>
                <SettingContainer
                  title={t("settings.browserConnector.screenshot.quickTap.defaultPrompt.title")}
                  description={t("settings.browserConnector.screenshot.quickTap.defaultPrompt.description")}
                  descriptionMode="inline"
                  grouped={true}
                  layout="stacked"
                >
                  <Textarea
                    value={screenshotNoVoicePromptInput}
                    onChange={(event) => setScreenshotNoVoicePromptInput(event.target.value)}
                    onBlur={handleScreenshotNoVoicePromptBlur}
                    disabled={!settings?.screenshot_allow_no_voice || isUpdating("screenshot_no_voice_default_prompt")}
                    placeholder={t("settings.browserConnector.screenshot.quickTap.defaultPrompt.placeholder")}
                    className="w-full"
                    rows={2}
                  />
                </SettingContainer>
              </div>
            </div>
          </div>
        </div>
      </SettingsGroup>


      {/* Auto-Open Tab Settings */}
      <SettingsGroup 
        title={t("settings.browserConnector.autoOpen.title")}
        description={t("settings.browserConnector.autoOpen.description")}
      >
        <SettingContainer
          title={t("settings.browserConnector.autoOpen.enabled.label")}
          description={t("settings.browserConnector.autoOpen.enabled.description")}
          descriptionMode="tooltip"
          grouped={true}
        >
          <ToggleSwitch
            checked={settings?.connector_auto_open_enabled ?? false}
            onChange={handleAutoOpenEnabledChange}
            disabled={isUpdating("connector_auto_open_enabled")}
          />
        </SettingContainer>
        <div className={!settings?.connector_auto_open_enabled ? "opacity-50" : ""}>
          <SettingContainer
            title={t("settings.browserConnector.autoOpen.site.title")}
            description={t("settings.browserConnector.autoOpen.site.description")}
            descriptionMode="tooltip"
            grouped={true}
          >
            <Select
              value={settings?.connector_auto_open_url ?? null}
              options={AUTO_OPEN_SITES}
              onChange={(value) => handleAutoOpenSiteChange(value ?? "")}
              disabled={!settings?.connector_auto_open_enabled || isUpdating("connector_auto_open_url")}
              placeholder={t("settings.browserConnector.autoOpen.site.placeholder")}
              isClearable={false}
              className="w-48"
            />
          </SettingContainer>
        </div>
      </SettingsGroup>

      <SettingsGroup title={t("settings.browserConnector.connection.title")}>
        <SettingContainer
          title={t("settings.browserConnector.connection.port.title")}
          description={t("settings.browserConnector.connection.port.description")}
          descriptionMode="tooltip"
          grouped={true}
        >
          <div className="flex flex-col gap-1">
            <Input
              type="number"
              value={portInput}
              onChange={(event) => {
                setPortInput(event.target.value);
                setPortError(null);
              }}
              onBlur={handlePortBlur}
              placeholder="38243"
              min={1024}
              max={65535}
              className={`w-28 ${portError ? "border-red-500" : ""}`}
            />
            {portError && (
              <div className="text-sm text-red-400 flex items-center gap-1">
                <AlertTriangle className="w-3 h-3" />
                {portError}
              </div>
            )}
          </div>
        </SettingContainer>

        <SettingContainer
          title={t("settings.browserConnector.connection.password.title")}
          description={t("settings.browserConnector.connection.password.description")}
          descriptionMode="tooltip"
          grouped={true}
          layout="stacked"
        >
          <div className="flex items-center gap-2">
            <Input
              type={showPassword ? "text" : "password"}
              value={passwordInput}
              onChange={(event) => setPasswordInput(event.target.value)}
              onBlur={handlePasswordBlur}
              placeholder="Enter connection password..."
              className="flex-1 font-mono"
            />
            <button
              type="button"
              onClick={() => setShowPassword(!showPassword)}
              className="p-2 rounded hover:bg-mid-gray/20 text-text/60 hover:text-text"
              title={showPassword ? "Hide password" : "Show password"}
            >
              {showPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
            </button>
            <div className="relative">
              <button
                type="button"
                onClick={handleCopyPassword}
                className="p-2 rounded hover:bg-mid-gray/20 text-text/60 hover:text-text"
                title="Copy password"
              >
                <Copy className="w-4 h-4" />
              </button>
              {showCopiedTooltip && (
                <div className="absolute -top-8 left-1/2 -translate-x-1/2 px-2 py-1 bg-green-600 text-white text-xs rounded whitespace-nowrap">
                  {t("common.copied")}
                </div>
              )}
            </div>
          </div>
          {isDefaultPassword && (
            <div className="mt-2 rounded-lg border border-yellow-500/30 bg-yellow-500/10 p-3">
              <div className="flex items-start gap-2">
                <AlertTriangle className="w-4 h-4 text-yellow-400 mt-0.5 flex-shrink-0" />
                <div className="text-sm text-yellow-200">
                  <p className="font-medium">{t("settings.browserConnector.connection.password.defaultWarning.title")}</p>
                  <p className="text-yellow-200/80 mt-1">
                    {t("settings.browserConnector.connection.password.defaultWarning.description")}
                  </p>
                </div>
              </div>
            </div>
          )}
        </SettingContainer>

        <SettingContainer
          title={t("settings.browserConnector.connection.endpoint.title")}
          description={t("settings.browserConnector.connection.endpoint.description")}
          descriptionMode="tooltip"
          grouped={true}
        >
          <div className="flex items-center gap-2 px-2 py-1 rounded bg-mid-gray/10 border border-mid-gray/30">
            <Globe className="w-4 h-4 text-mid-gray" />
            <code className="text-sm font-mono">{endpointUrl}</code>
          </div>
        </SettingContainer>
      </SettingsGroup>

      {/* Warning modal for enabling risky features */}
      <ConfirmationModal
        isOpen={showEnableWarning !== null}
        onClose={() => setShowEnableWarning(null)}
        onConfirm={() => {
          if (showEnableWarning === "send_to_extension") {
            void updateSetting("send_to_extension_enabled", true);
          } else if (showEnableWarning === "send_to_extension_with_selection") {
            void updateSetting("send_to_extension_with_selection_enabled", true);
          } else if (showEnableWarning === "send_screenshot_to_extension") {
            void updateSetting("send_screenshot_to_extension_enabled", true);
          }
        }}
        title={showEnableWarning ? t(`settings.general.shortcut.bindings.${showEnableWarning}.enable.warning.title`) : ""}
        message={showEnableWarning ? t(`settings.general.shortcut.bindings.${showEnableWarning}.enable.warning.message`) : ""}
        confirmText={showEnableWarning ? t(`settings.general.shortcut.bindings.${showEnableWarning}.enable.warning.confirm`) : ""}
        variant="warning"
      />
    </div>
  );
};
