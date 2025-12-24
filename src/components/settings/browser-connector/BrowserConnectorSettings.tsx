import React, { useEffect, useState } from "react";
import { useTranslation, Trans } from "react-i18next";
import { Globe, Info, ExternalLink } from "lucide-react";
import { useSettings } from "../../../hooks/useSettings";
import { HandyShortcut } from "../HandyShortcut";
import { Input } from "../../ui/Input";
import { Select } from "../../ui/Select";
import { SettingContainer } from "../../ui/SettingContainer";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { Textarea } from "../../ui/Textarea";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { ConnectorStatusIndicator } from "./ConnectorStatus";

// Preset sites for auto-open dropdown
const AUTO_OPEN_SITES = [
  { value: "https://chatgpt.com", label: "ChatGPT" },
  { value: "https://claude.ai", label: "Claude" },
];

export const BrowserConnectorSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, getSetting, updateSetting, isUpdating } = useSettings();

  const [hostInput, setHostInput] = useState(settings?.connector_host ?? "127.0.0.1");
  const [portInput, setPortInput] = useState(String(settings?.connector_port ?? 63155));
  const [pathInput, setPathInput] = useState(settings?.connector_path ?? "/messages");

  // Connector prompt settings
  const sendSystemPrompt = getSetting("connector_send_system_prompt") ?? "";
  const sendSelectionSystemPrompt = getSetting("connector_send_selection_system_prompt") ?? "";
  const sendSelectionUserPrompt = getSetting("connector_send_selection_user_prompt") ?? "";

  useEffect(() => {
    setHostInput(settings?.connector_host ?? "127.0.0.1");
  }, [settings?.connector_host]);

  useEffect(() => {
    setPortInput(String(settings?.connector_port ?? 63155));
  }, [settings?.connector_port]);

  useEffect(() => {
    setPathInput(settings?.connector_path ?? "/messages");
  }, [settings?.connector_path]);

  const handleHostBlur = () => {
    const trimmed = hostInput.trim();
    if (trimmed !== (settings?.connector_host ?? "")) {
      void updateSetting("connector_host", trimmed);
    }
  };

  const handlePortBlur = () => {
    const port = parseInt(portInput.trim(), 10);
    if (!isNaN(port) && port > 0 && port <= 65535 && port !== settings?.connector_port) {
      void updateSetting("connector_port", port);
    }
  };

  const handlePathBlur = () => {
    const trimmed = pathInput.trim();
    if (trimmed !== (settings?.connector_path ?? "")) {
      void updateSetting("connector_path", trimmed);
    }
  };

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

  const handleSendSystemPromptChange = (event: React.ChangeEvent<HTMLTextAreaElement>) => {
    void updateSetting("connector_send_system_prompt", event.target.value);
  };

  const handleSendSelectionSystemPromptChange = (event: React.ChangeEvent<HTMLTextAreaElement>) => {
    void updateSetting("connector_send_selection_system_prompt", event.target.value);
  };

  const handleSendSelectionUserPromptChange = (event: React.ChangeEvent<HTMLTextAreaElement>) => {
    void updateSetting("connector_send_selection_user_prompt", event.target.value);
  };

  const endpointUrl = `http://${hostInput}:${portInput}${pathInput}`;

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      {/* Help Banner */}
      <div className="rounded-lg border border-blue-500/30 bg-blue-500/10 p-4">
        <div className="flex items-start gap-3">
          <Info className="w-5 h-5 text-blue-400 mt-0.5 flex-shrink-0" />
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
                      href="https://github.com/nicekid1/Handy-Connector"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-blue-400 hover:underline inline-flex items-center gap-1"
                    >
                      Handy Connector
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
          </div>
        </div>
      </div>

      {/* Extension Status */}
      <SettingsGroup title={t("settings.browserConnector.status.sectionTitle")}>
        <ConnectorStatusIndicator grouped={true} descriptionMode="tooltip" />
      </SettingsGroup>

      <SettingsGroup title={t("settings.browserConnector.shortcuts.title")}>
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
      </SettingsGroup>

      {/* Send to Extension Prompts */}
      <SettingsGroup title={t("settings.browserConnector.sendPrompts.title")}>
        <div className="text-sm text-text/60 mb-2 px-1">
          {t("settings.browserConnector.sendPrompts.description")}
        </div>
        <SettingContainer
          title={t("settings.browserConnector.sendPrompts.systemPrompt.title")}
          description={t("settings.browserConnector.sendPrompts.systemPrompt.description")}
          descriptionMode="inline"
          grouped={true}
          layout="stacked"
        >
          <Textarea
            value={sendSystemPrompt}
            onChange={handleSendSystemPromptChange}
            disabled={isUpdating("connector_send_system_prompt")}
            placeholder={t("settings.browserConnector.sendPrompts.systemPrompt.placeholder")}
            className="w-full"
            rows={3}
          />
        </SettingContainer>
      </SettingsGroup>

      {/* Send + Selection Prompts */}
      <SettingsGroup title={t("settings.browserConnector.sendSelectionPrompts.title")}>
        <div className="text-sm text-text/60 mb-2 px-1">
          {t("settings.browserConnector.sendSelectionPrompts.description")}
        </div>
        <SettingContainer
          title={t("settings.browserConnector.sendSelectionPrompts.systemPrompt.title")}
          description={t("settings.browserConnector.sendSelectionPrompts.systemPrompt.description")}
          descriptionMode="inline"
          grouped={true}
          layout="stacked"
        >
          <Textarea
            value={sendSelectionSystemPrompt}
            onChange={handleSendSelectionSystemPromptChange}
            disabled={isUpdating("connector_send_selection_system_prompt")}
            className="w-full"
            rows={4}
          />
        </SettingContainer>
        <SettingContainer
          title={t("settings.browserConnector.sendSelectionPrompts.userPrompt.title")}
          description={t("settings.browserConnector.sendSelectionPrompts.userPrompt.description")}
          descriptionMode="inline"
          grouped={true}
          layout="stacked"
        >
          <Textarea
            value={sendSelectionUserPrompt}
            onChange={handleSendSelectionUserPromptChange}
            disabled={isUpdating("connector_send_selection_user_prompt")}
            className="w-full"
            rows={3}
          />
          <div className="text-xs text-text/50 mt-1">
            {t("settings.browserConnector.sendSelectionPrompts.variables")}
          </div>
        </SettingContainer>
      </SettingsGroup>

      {/* Auto-Open Tab Settings */}
      <SettingsGroup title={t("settings.browserConnector.autoOpen.title")}>
        <div className="text-sm text-text/60 mb-2 px-1">
          {t("settings.browserConnector.autoOpen.description")}
        </div>
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
          title={t("settings.browserConnector.connection.host.title")}
          description={t("settings.browserConnector.connection.host.description")}
          descriptionMode="tooltip"
          grouped={true}
        >
          <Input
            type="text"
            value={hostInput}
            onChange={(event) => setHostInput(event.target.value)}
            onBlur={handleHostBlur}
            placeholder="127.0.0.1"
            className="w-40"
          />
        </SettingContainer>

        <SettingContainer
          title={t("settings.browserConnector.connection.port.title")}
          description={t("settings.browserConnector.connection.port.description")}
          descriptionMode="tooltip"
          grouped={true}
        >
          <Input
            type="number"
            value={portInput}
            onChange={(event) => setPortInput(event.target.value)}
            onBlur={handlePortBlur}
            placeholder="63155"
            min={1}
            max={65535}
            className="w-28"
          />
        </SettingContainer>

        <SettingContainer
          title={t("settings.browserConnector.connection.path.title")}
          description={t("settings.browserConnector.connection.path.description")}
          descriptionMode="tooltip"
          grouped={true}
        >
          <Input
            type="text"
            value={pathInput}
            onChange={(event) => setPathInput(event.target.value)}
            onBlur={handlePathBlur}
            placeholder="/messages"
            className="w-40"
          />
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
    </div>
  );
};
