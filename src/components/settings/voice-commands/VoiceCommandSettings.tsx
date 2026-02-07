import { useState, useEffect, useRef } from "react";
import { useSettings } from "@/hooks/useSettings";
import { useTranslation } from "react-i18next";
import { RefreshCcw } from "lucide-react";
import { VoiceCommand, commands, ExecutionPolicy } from "@/bindings";
import { HandyShortcut } from "../HandyShortcut";
import { listen } from "@tauri-apps/api/event";
import type { VoiceCommandResultPayload } from "@/command-confirm/CommandConfirmOverlay";
import { ExtendedThinkingSection } from "../ExtendedThinkingSection";
import { ProviderSelect } from "../PostProcessingSettingsApi/ProviderSelect";
import { ApiKeyField } from "../PostProcessingSettingsApi/ApiKeyField";
import { ModelSelect } from "../PostProcessingSettingsApi/ModelSelect";
import { ResetButton } from "../../ui/ResetButton";
import { TellMeMore } from "../../ui/TellMeMore";
import { useVoiceCommandProviderState } from "./useVoiceCommandProviderState";

const DEFAULT_VOICE_COMMAND_SYSTEM_PROMPT = `You are a Windows command generator. The user will describe what they want to do, and you must generate a SINGLE PowerShell one-liner command that accomplishes it.

Rules:
1. Return ONLY the command, nothing else - no explanations, no markdown, no code blocks
2. The command must be a valid PowerShell one-liner that can run directly
3. Use Start-Process for launching applications
4. Use common Windows paths and commands
5. If the request is unclear or dangerous (like deleting system files), return: UNSAFE_REQUEST
6. Keep commands simple and safe

Example inputs and outputs:
- "open notepad" → Start-Process notepad
- "open chrome" → Start-Process chrome
- "lock the computer" → rundll32.exe user32.dll,LockWorkStation
- "open word and excel" → Start-Process winword; Start-Process excel
- "show my documents folder" → Start-Process explorer -ArgumentList "$env:USERPROFILE\\Documents"`;

const MAX_LOG_ENTRIES = 100;

// Execution policy options for dropdown
const EXECUTION_POLICY_OPTIONS = [
  { value: "default", label: "Default (system policy)" },
  { value: "bypass", label: "Bypass (recommended)" },
  { value: "unrestricted", label: "Unrestricted" },
  { value: "remote_signed", label: "RemoteSigned" },
];

interface LogEntry extends VoiceCommandResultPayload {
  id: string;
}

interface VoiceCommandCardProps {
  command: VoiceCommand;
  onUpdate: (updated: VoiceCommand) => void;
  onDelete: () => void;
}

function VoiceCommandCard({
  command,
  onUpdate,
  onDelete,
}: VoiceCommandCardProps) {
  const { t } = useTranslation();
  const [isEditing, setIsEditing] = useState(false);
  const [isExecutionOpen, setIsExecutionOpen] = useState(false);
  const [editName, setEditName] = useState(command.name);
  const [editPhrase, setEditPhrase] = useState(command.trigger_phrase);
  const [editScript, setEditScript] = useState(command.script);
  const [editThreshold, setEditThreshold] = useState(
    command.similarity_threshold ?? 0.75,
  );
  // Execution options
  const [editSilent, setEditSilent] = useState(command.silent ?? true);
  const [editNoProfile, setEditNoProfile] = useState(
    command.no_profile ?? false,
  );
  const [editUsePwsh, setEditUsePwsh] = useState(command.use_pwsh ?? false);
  const [editExecutionPolicy, setEditExecutionPolicy] =
    useState<ExecutionPolicy | null>(command.execution_policy ?? null);
  const [editWorkingDirectory, setEditWorkingDirectory] = useState(
    command.working_directory ?? "",
  );

  const handleSave = () => {
    onUpdate({
      ...command,
      name: editName,
      trigger_phrase: editPhrase,
      script: editScript,
      similarity_threshold: editThreshold,
      silent: editSilent,
      no_profile: editNoProfile,
      use_pwsh: editUsePwsh,
      execution_policy: editExecutionPolicy,
      working_directory: editWorkingDirectory || null,
    });
    setIsEditing(false);
  };

  const handleCancel = () => {
    setEditName(command.name);
    setEditPhrase(command.trigger_phrase);
    setEditScript(command.script);
    setEditThreshold(command.similarity_threshold ?? 0.75);
    setEditSilent(command.silent ?? true);
    setEditNoProfile(command.no_profile ?? false);
    setEditUsePwsh(command.use_pwsh ?? false);
    setEditExecutionPolicy(command.execution_policy ?? null);
    setEditWorkingDirectory(command.working_directory ?? "");
    setIsEditing(false);
  };

  if (isEditing) {
    return (
      <div className="voice-command-card editing">
        <div className="voice-command-field">
          <label>{t("voiceCommands.card.name", "Name")}</label>
          <input
            type="text"
            value={editName}
            onChange={(e) => setEditName(e.target.value)}
            placeholder="Lock Computer"
          />
        </div>
        <div className="voice-command-field">
          <label>
            {t("voiceCommands.card.triggerPhrase", "Trigger Phrase")}
          </label>
          <input
            type="text"
            value={editPhrase}
            onChange={(e) => setEditPhrase(e.target.value)}
            placeholder="lock computer"
          />
        </div>
        <div className="voice-command-field">
          <label>{t("voiceCommands.card.script", "Script/Command")}</label>
          <input
            type="text"
            value={editScript}
            onChange={(e) => setEditScript(e.target.value)}
            placeholder="rundll32.exe user32.dll,LockWorkStation"
            className="mono"
          />
        </div>
        <div className="voice-command-field">
          <label>
            {t("voiceCommands.card.matchThreshold", "Match Threshold")}:{" "}
            {Math.round(editThreshold * 100)}%
          </label>
          <input
            type="range"
            min="0.5"
            max="1"
            step="0.05"
            value={editThreshold}
            onChange={(e) => setEditThreshold(parseFloat(e.target.value))}
          />
        </div>

        {/* Execution Options Section */}
        <div className="voice-command-execution-section">
          <button
            type="button"
            className="execution-toggle"
            onClick={() => setIsExecutionOpen(!isExecutionOpen)}
          >
            <span>
              {isExecutionOpen ? "▾" : "▸"}{" "}
              {t("voiceCommands.card.executionOptions", "Execution options")}
            </span>
          </button>

          {isExecutionOpen && (
            <div className="execution-options-content">
              <div className="execution-option-row">
                <span>
                  {t("voiceCommands.silentExecution", "Silent execution")}
                </span>
                <label className="toggle-switch small">
                  <input
                    type="checkbox"
                    checked={editSilent}
                    onChange={(e) => setEditSilent(e.target.checked)}
                  />
                  <span className="slider"></span>
                </label>
              </div>
              <div className="execution-option-row">
                <span>
                  {t("voiceCommands.skipProfile", "Skip profile loading")}
                </span>
                <label className="toggle-switch small">
                  <input
                    type="checkbox"
                    checked={editNoProfile}
                    onChange={(e) => setEditNoProfile(e.target.checked)}
                  />
                  <span className="slider"></span>
                </label>
              </div>
              <div className="execution-option-row">
                <span>
                  {t("voiceCommands.usePwsh", "Use PowerShell 7 (pwsh)")}
                </span>
                <label className="toggle-switch small">
                  <input
                    type="checkbox"
                    checked={editUsePwsh}
                    onChange={(e) => setEditUsePwsh(e.target.checked)}
                  />
                  <span className="slider"></span>
                </label>
              </div>
              <div className="execution-option-row">
                <span>
                  {t("voiceCommands.executionPolicy", "Execution Policy")}
                </span>
                <select
                  className="execution-policy-select-small"
                  value={editExecutionPolicy ?? "inherit"}
                  onChange={(e) =>
                    setEditExecutionPolicy(
                      e.target.value === "inherit" ? null : e.target.value as ExecutionPolicy,
                    )
                  }
                >
                  <option value="inherit">
                    {t(
                      "voiceCommands.inheritFromDefaults",
                      "Inherit from defaults",
                    )}
                  </option>
                  {EXECUTION_POLICY_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>
              <div className="execution-option-row column">
                <span>
                  {t("voiceCommands.workingDirectory", "Working Directory")}
                </span>
                <input
                  type="text"
                  value={editWorkingDirectory}
                  onChange={(e) => setEditWorkingDirectory(e.target.value)}
                  placeholder={t(
                    "voiceCommands.workingDirectoryPlaceholder",
                    "Optional, for this command",
                  )}
                  className="working-directory-input"
                />
              </div>
            </div>
          )}
        </div>

        <div className="voice-command-actions">
          <button className="btn-cancel" onClick={handleCancel}>
            {t("common.cancel", "Cancel")}
          </button>
          <button className="btn-save" onClick={handleSave}>
            {t("common.save", "Save")}
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className={`voice-command-card ${!command.enabled ? "disabled" : ""}`}>
      <div className="voice-command-header">
        <span className="voice-command-name">{command.name}</span>
        <div className="voice-command-controls">
          <label className="toggle-switch small">
            <input
              type="checkbox"
              checked={command.enabled}
              onChange={(e) =>
                onUpdate({ ...command, enabled: e.target.checked })
              }
            />
            <span className="slider"></span>
          </label>
          <button className="btn-edit" onClick={() => setIsEditing(true)}>
            ✏️
          </button>
          <button className="btn-delete" onClick={onDelete}>
            🗑️
          </button>
        </div>
      </div>
      <div className="voice-command-phrase">"{command.trigger_phrase}"</div>
      <div className="voice-command-script">{command.script}</div>
    </div>
  );
}

export default function VoiceCommandSettings() {
  const { t } = useTranslation();
  const { settings, updateSetting } = useSettings();
  const voiceCommandProviderState = useVoiceCommandProviderState();
  const [executionLog, setExecutionLog] = useState<LogEntry[]>([]);
  const logEndRef = useRef<HTMLDivElement>(null);
  const [mockInput, setMockInput] = useState("");
  const [mockStatus, setMockStatus] = useState<{
    type: "success" | "error" | "loading";
    message: string;
  } | null>(null);
  const [isLlmSettingsOpen, setIsLlmSettingsOpen] = useState(false);
  const [isFuzzyMatchingOpen, setIsFuzzyMatchingOpen] = useState(false);

  if (!settings) return null;

  // Get voice command defaults with fallbacks
  const defaults = settings.voice_command_defaults ?? {
    silent: true,
    no_profile: false,
    use_pwsh: false,
    execution_policy: "bypass",
  };

  // Listen for execution results
  useEffect(() => {
    const unlisten = listen<VoiceCommandResultPayload>(
      "voice-command-result",
      (event) => {
        const entry: LogEntry = {
          ...event.payload,
          id: `log_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
        };
        setExecutionLog((prev) => {
          const updated = [...prev, entry];
          // Keep only last MAX_LOG_ENTRIES
          return updated.slice(-MAX_LOG_ENTRIES);
        });
      },
    );

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Auto-scroll to bottom when new entries are added
  useEffect(() => {
    logEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [executionLog]);

  const handleAddCommand = () => {
    // New commands inherit execution options from global defaults
    const newCommand: VoiceCommand = {
      id: `vc_${Date.now()}`,
      name: "New Command",
      trigger_phrase: "",
      script: "",
      similarity_threshold: settings.voice_command_default_threshold || 0.75,
      enabled: true,
      // Inherit execution options from defaults
      silent: defaults.silent,
      no_profile: defaults.no_profile,
      use_pwsh: defaults.use_pwsh,
      execution_policy: null, // null = inherit from defaults at execution time
      working_directory: null,
    };
    updateSetting("voice_commands", [
      ...(settings.voice_commands || []),
      newCommand,
    ]);
  };

  const handleUpdateCommand = (index: number, updated: VoiceCommand) => {
    const commands = [...(settings.voice_commands || [])];
    commands[index] = updated;
    updateSetting("voice_commands", commands);
  };

  const handleDeleteCommand = (index: number) => {
    const commands = [...(settings.voice_commands || [])];
    commands.splice(index, 1);
    updateSetting("voice_commands", commands);
  };

  const handleClearLog = () => {
    setExecutionLog([]);
  };

  const handleCopyLog = () => {
    const logText = executionLog
      .map((entry) => {
        const time = new Date(entry.timestamp).toLocaleTimeString();
        const status = entry.isError
          ? "ERROR"
          : entry.wasOpenedInWindow
            ? "OPENED"
            : "OK";
        return `[${time}] [${status}] ${entry.command}\n${entry.output || "(no output)"}`;
      })
      .join("\n\n");
    navigator.clipboard.writeText(logText);
  };

  const formatTime = (timestamp: number) => {
    return new Date(timestamp).toLocaleTimeString();
  };

  const handleMockTest = async () => {
    if (!mockInput.trim()) {
      setMockStatus({ type: "error", message: "Please enter mock text" });
      return;
    }

    setMockStatus({ type: "loading", message: "Processing..." });

    try {
      const result = await commands.testVoiceCommandMock(mockInput.trim());
      if (result.status === "ok") {
        setMockStatus({ type: "success", message: result.data });
        // Clear after showing result
        setTimeout(() => setMockStatus(null), 3000);
      } else {
        setMockStatus({
          type: "error",
          message: result.error || "Test failed",
        });
      }
    } catch (err) {
      setMockStatus({ type: "error", message: String(err) });
    }
  };

  const modelDescription = voiceCommandProviderState.isAppleProvider
    ? t("settings.postProcessing.api.model.descriptionApple")
    : voiceCommandProviderState.isCustomProvider
      ? t("settings.postProcessing.api.model.descriptionCustom")
      : t("settings.postProcessing.api.model.descriptionDefault");

  const modelPlaceholder = voiceCommandProviderState.isAppleProvider
    ? t("settings.postProcessing.api.model.placeholderApple")
    : voiceCommandProviderState.modelOptions.length > 0
      ? t("settings.postProcessing.api.model.placeholderWithOptions")
      : t("settings.postProcessing.api.model.placeholderNoOptions");

  return (
    <div className="voice-command-settings">
      <div className="setting-section-header">
        <h3>{t("voiceCommands.title", "Voice Command Center")}</h3>
        <p className="setting-description">
          {t(
            "voiceCommands.description",
            "Define trigger phrases that execute scripts. If no match is found, an LLM can suggest a PowerShell command.",
          )}
        </p>
      </div>

      <div className="setting-row">
        <div className="setting-label">
          <span>{t("voiceCommands.enabled", "Enable Voice Commands")}</span>
        </div>
        <label className="toggle-switch">
          <input
            type="checkbox"
            checked={settings.voice_command_enabled || false}
            onChange={(e) =>
              updateSetting("voice_command_enabled", e.target.checked)
            }
          />
          <span className="slider"></span>
        </label>
      </div>

      {settings.voice_command_enabled && (
        <>
          <div className="shortcut-row">
            <HandyShortcut
              shortcutId="voice_command"
              descriptionMode="tooltip"
              grouped={false}
            />
          </div>

          <div className="setting-row">
            <div className="setting-label">
              <span>{t("voiceCommands.llmFallback", "LLM Fallback")}</span>
              <span className="setting-sublabel">
                {t(
                  "voiceCommands.llmFallbackDesc",
                  "Use AI to generate commands when no predefined match is found",
                )}
              </span>
            </div>
            <label className="toggle-switch">
              <input
                type="checkbox"
                checked={settings.voice_command_llm_fallback ?? true}
                onChange={(e) =>
                  updateSetting("voice_command_llm_fallback", e.target.checked)
                }
              />
              <span className="slider"></span>
            </label>
          </div>

          {(settings.voice_command_llm_fallback ?? true) && (
            <div className="setting-row system-prompt-row">
              <div className="setting-label">
                <span>
                  {t("voiceCommands.systemPrompt", "LLM System Prompt")}
                </span>
                <span className="setting-sublabel">
                  {t(
                    "voiceCommands.systemPromptDesc",
                    "Instructions for the AI when generating PowerShell commands",
                  )}
                </span>
              </div>
              <div className="system-prompt-container">
                <textarea
                  className="system-prompt-textarea"
                  value={settings.voice_command_system_prompt || ""}
                  onChange={(e) =>
                    updateSetting("voice_command_system_prompt", e.target.value)
                  }
                  placeholder="You are a Windows command generator..."
                  rows={8}
                />
                <button
                  className="btn-reset-prompt"
                  onClick={() =>
                    updateSetting(
                      "voice_command_system_prompt",
                      DEFAULT_VOICE_COMMAND_SYSTEM_PROMPT,
                    )
                  }
                  title={t("voiceCommands.resetPrompt", "Reset to default")}
                >
                  ↺
                </button>
              </div>
            </div>
          )}

          {(settings.voice_command_llm_fallback ?? true) && (
            <div className="llm-api-section">
              <button
                type="button"
                className="llm-api-toggle"
                onClick={() => setIsLlmSettingsOpen((prev) => !prev)}
                aria-expanded={isLlmSettingsOpen}
              >
                <div className="llm-api-toggle-text">
                  <span className="llm-api-title">
                    {t("voiceCommands.llmApi.title", "LLM API Settings")}
                  </span>
                  <span className="llm-api-sublabel">
                    {t(
                      "voiceCommands.llmApi.description",
                      "Configure the provider, API key, and model used for voice command generation.",
                    )}
                  </span>
                </div>
                <span className="llm-api-toggle-icon">
                  {isLlmSettingsOpen ? "-" : "+"}
                </span>
              </button>

              {isLlmSettingsOpen && (
                <div className="llm-api-content">
                  <div className="setting-row llm-api-row llm-api-row-provider">
                    <div className="setting-label">
                      <span>
                        {t("settings.postProcessing.api.provider.title")}
                      </span>
                      <span className="setting-sublabel">
                        {t("settings.postProcessing.api.provider.description")}
                      </span>
                    </div>
                    <div className="llm-api-control">
                      <ProviderSelect
                        options={voiceCommandProviderState.providerOptions}
                        value={voiceCommandProviderState.selectedProviderId}
                        onChange={
                          voiceCommandProviderState.handleProviderSelect
                        }
                      />
                    </div>
                  </div>

                  {voiceCommandProviderState.useSameAsPostProcess ? (
                    <div className="llm-api-note">
                      {t(
                        "voiceCommands.llmApi.sameAsPostProcessing",
                        "Using the same LLM settings as Transcription Post-Processing.",
                      )}
                    </div>
                  ) : (
                    <>
                      {voiceCommandProviderState.isAppleProvider ? (
                        <div className="llm-api-apple-note">
                          <div className="llm-api-apple-title">
                            {t(
                              "settings.postProcessing.api.appleIntelligence.title",
                            )}
                          </div>
                          <div>
                            {t(
                              "settings.postProcessing.api.appleIntelligence.description",
                            )}
                          </div>
                          <div className="llm-api-apple-requirements">
                            {t(
                              "settings.postProcessing.api.appleIntelligence.requirements",
                            )}
                          </div>
                        </div>
                      ) : (
                        <div className="setting-row llm-api-row">
                          <div className="setting-label">
                            <span>
                              {t("settings.postProcessing.api.apiKey.title")}
                            </span>
                            <span className="setting-sublabel">
                              {t(
                                "settings.postProcessing.api.apiKey.description",
                              )}
                            </span>
                          </div>
                          <div className="llm-api-control">
                            <ApiKeyField
                              value={voiceCommandProviderState.apiKey}
                              onBlur={
                                voiceCommandProviderState.handleApiKeyChange
                              }
                              placeholder={t(
                                "settings.postProcessing.api.apiKey.placeholder",
                              )}
                              disabled={
                                voiceCommandProviderState.isApiKeyUpdating
                              }
                              secureStorage={
                                voiceCommandProviderState.selectedProvider?.id
                                  ? {
                                      feature: "voice_command",
                                      providerId:
                                        voiceCommandProviderState.selectedProvider
                                          .id,
                                    }
                                  : undefined
                              }
                            />
                          </div>
                        </div>
                      )}

                      <div className="setting-row llm-api-row">
                        <div className="setting-label">
                          <span>
                            {t("settings.postProcessing.api.model.title")}
                          </span>
                          <span className="setting-sublabel">
                            {modelDescription}
                          </span>
                        </div>
                        <div className="llm-api-model-row">
                          <ModelSelect
                            value={voiceCommandProviderState.model}
                            options={voiceCommandProviderState.modelOptions}
                            disabled={voiceCommandProviderState.isModelUpdating}
                            isLoading={
                              voiceCommandProviderState.isFetchingModels
                            }
                            placeholder={modelPlaceholder}
                            onSelect={
                              voiceCommandProviderState.handleModelSelect
                            }
                            onCreate={
                              voiceCommandProviderState.handleModelCreate
                            }
                            onBlur={() => {}}
                            className="llm-api-model-select"
                          />
                          <ResetButton
                            onClick={
                              voiceCommandProviderState.handleRefreshModels
                            }
                            disabled={
                              voiceCommandProviderState.isFetchingModels ||
                              voiceCommandProviderState.isAppleProvider
                            }
                            ariaLabel={t(
                              "settings.postProcessing.api.model.refreshModels",
                            )}
                            className="llm-api-refresh"
                          >
                            <RefreshCcw
                              className={`h-4 w-4 ${
                                voiceCommandProviderState.isFetchingModels
                                  ? "animate-spin"
                                  : ""
                              }`}
                            />
                          </ResetButton>
                        </div>
                      </div>
                    </>
                  )}

                  <div className="llm-api-extended">
                    <ExtendedThinkingSection
                      settingPrefix="voice_command"
                      grouped={false}
                    />
                  </div>
                </div>
              )}
            </div>
          )}

          <div className="voice-commands-list">
            <div className="list-header">
              <h4>
                {t("voiceCommands.predefinedCommands", "Predefined Commands")}
              </h4>
              <button className="btn-add" onClick={handleAddCommand}>
                + {t("voiceCommands.addCommand", "Add Command")}
              </button>
            </div>

            {(settings.voice_commands || []).length === 0 ? (
              <div className="empty-state">
                <p>
                  {t(
                    "voiceCommands.noCommands",
                    "No commands defined yet. Add one to get started!",
                  )}
                </p>
                <p className="hint">
                  {t(
                    "voiceCommands.hint",
                    'Example: "lock computer" → rundll32.exe user32.dll,LockWorkStation',
                  )}
                </p>
              </div>
            ) : (
              (settings.voice_commands || []).map((cmd, index) => (
                <VoiceCommandCard
                  key={cmd.id}
                  command={cmd}
                  onUpdate={(updated) => handleUpdateCommand(index, updated)}
                  onDelete={() => handleDeleteCommand(index)}
                />
              ))
            )}
          </div>

          {/* Execution Settings Section */}
          <div className="execution-settings-section">
            <div className="section-divider">
              <span>
                {t("voiceCommands.executionDefaults", "Execution Defaults")}
              </span>
            </div>
            <p className="execution-defaults-description">
              {t(
                "voiceCommands.executionDefaultsDesc",
                "Default options for new commands and LLM fallback. Commands are executed via PowerShell.",
              )}
            </p>

            <div className="setting-row">
              <div className="setting-label">
                <span>
                  {t("voiceCommands.silentExecution", "Silent execution")}
                </span>
                <span className="setting-sublabel">
                  {t(
                    "voiceCommands.silentExecutionDesc",
                    "Hidden window, non-interactive, output captured",
                  )}
                </span>
              </div>
              <label className="toggle-switch">
                <input
                  type="checkbox"
                  checked={defaults.silent}
                  onChange={(e) =>
                    updateSetting("voice_command_defaults", {
                      ...defaults,
                      silent: e.target.checked,
                    })
                  }
                />
                <span className="slider"></span>
              </label>
            </div>

            <div className="setting-row">
              <div className="setting-label">
                <span>
                  {t("voiceCommands.skipProfile", "Skip profile loading")}
                </span>
                <span className="setting-sublabel">
                  {t(
                    "voiceCommands.skipProfileDesc",
                    "Disable to get the same experience as in terminal",
                  )}
                </span>
              </div>
              <label className="toggle-switch">
                <input
                  type="checkbox"
                  checked={defaults.no_profile}
                  onChange={(e) =>
                    updateSetting("voice_command_defaults", {
                      ...defaults,
                      no_profile: e.target.checked,
                    })
                  }
                />
                <span className="slider"></span>
              </label>
            </div>

            <div className="setting-row">
              <div className="setting-label">
                <span>
                  {t("voiceCommands.usePwsh", "Use PowerShell 7 (pwsh)")}
                </span>
                <span className="setting-sublabel">
                  {t(
                    "voiceCommands.usePwshDesc",
                    "Use modern PowerShell Core instead of Windows PowerShell 5.1",
                  )}
                </span>
              </div>
              <label className="toggle-switch">
                <input
                  type="checkbox"
                  checked={defaults.use_pwsh}
                  onChange={(e) =>
                    updateSetting("voice_command_defaults", {
                      ...defaults,
                      use_pwsh: e.target.checked,
                    })
                  }
                />
                <span className="slider"></span>
              </label>
            </div>

            <div className="setting-row">
              <div className="setting-label">
                <span>
                  {t("voiceCommands.executionPolicy", "Execution Policy")}
                </span>
                <span className="setting-sublabel">
                  {t(
                    "voiceCommands.executionPolicyDesc",
                    "Controls script execution permissions",
                  )}
                </span>
              </div>
              <select
                className="execution-policy-select"
                value={defaults.execution_policy}
                onChange={(e) =>
                  updateSetting("voice_command_defaults", {
                    ...defaults,
                    execution_policy: e.target.value as ExecutionPolicy,
                  })
                }
              >
                {EXECUTION_POLICY_OPTIONS.map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </select>
            </div>
          </div>

          <div className="setting-row">
            <div className="setting-label">
              <span>
                {t("voiceCommands.defaultThreshold", "Default Match Threshold")}
              </span>
              <span className="setting-sublabel">
                {Math.round(
                  (settings.voice_command_default_threshold || 0.75) * 100,
                )}
                %
              </span>
            </div>
            <input
              type="range"
              min="0.5"
              max="1"
              step="0.05"
              value={settings.voice_command_default_threshold || 0.75}
              onChange={(e) =>
                updateSetting(
                  "voice_command_default_threshold",
                  parseFloat(e.target.value),
                )
              }
              className="threshold-slider"
            />
          </div>

          {/* Fuzzy Matching Settings */}
          <div className="fuzzy-matching-section">
            <button
              type="button"
              className="fuzzy-matching-toggle"
              onClick={() => setIsFuzzyMatchingOpen((prev) => !prev)}
              aria-expanded={isFuzzyMatchingOpen}
            >
              <div className="fuzzy-matching-toggle-text">
                <span className="fuzzy-matching-title">
                  {t(
                    "voiceCommands.fuzzyMatching.title",
                    "Fuzzy Matching Settings",
                  )}
                </span>
                <span className="fuzzy-matching-sublabel">
                  {t(
                    "voiceCommands.fuzzyMatching.toggleDesc",
                    "Handle typos, mishearings, and similar-sounding words",
                  )}
                </span>
              </div>
              <span className="fuzzy-matching-toggle-icon">
                {isFuzzyMatchingOpen ? "−" : "+"}
              </span>
            </button>

            {isFuzzyMatchingOpen && (
              <div className="fuzzy-matching-content">
                <TellMeMore
                  title={t(
                    "voiceCommands.fuzzyMatching.tellMeMore",
                    "Tell me more: Fuzzy Matching",
                  )}
                >
                  <p className="mb-3">
                    {t(
                      "voiceCommands.fuzzyMatching.description",
                      "Fuzzy matching helps recognize voice commands even with slight variations, typos, or pronunciation differences.",
                    )}
                  </p>
                  <p className="mb-3">
                    <strong>
                      {t(
                        "voiceCommands.fuzzyMatching.levenshteinTitle",
                        "Character-level matching (Levenshtein):",
                      )}
                    </strong>{" "}
                    {t(
                      "voiceCommands.fuzzyMatching.levenshteinDesc",
                      "Handles typos and misheard letters. For example, 'srart' matches 'start' because only one letter is different.",
                    )}
                  </p>
                  <p className="mb-3">
                    <strong>
                      {t(
                        "voiceCommands.fuzzyMatching.phoneticTitle",
                        "Phonetic matching (Soundex):",
                      )}
                    </strong>{" "}
                    {t(
                      "voiceCommands.fuzzyMatching.phoneticDesc",
                      "Matches words that sound similar. For example, 'edge' and 'etch' have the same phonetic code.",
                    )}
                  </p>
                  <p className="text-text/70">
                    {t(
                      "voiceCommands.fuzzyMatching.tip",
                      "Tip: If commands aren't matching, try lowering the thresholds. If too many false matches occur, raise them.",
                    )}
                  </p>
                </TellMeMore>

                <div className="setting-row">
                  <div className="setting-label">
                    <span>
                      {t(
                        "voiceCommands.fuzzyMatching.useLevenshtein",
                        "Character-level matching",
                      )}
                    </span>
                    <span className="setting-sublabel">
                      {t(
                        "voiceCommands.fuzzyMatching.useLevenshteinDesc",
                        "Handles typos and transcription errors",
                      )}
                    </span>
                  </div>
                  <label className="toggle-switch">
                    <input
                      type="checkbox"
                      checked={settings.voice_command_use_levenshtein ?? true}
                      onChange={(e) =>
                        updateSetting(
                          "voice_command_use_levenshtein",
                          e.target.checked,
                        )
                      }
                    />
                    <span className="slider"></span>
                  </label>
                </div>

                {(settings.voice_command_use_levenshtein ?? true) && (
                  <div className="setting-row sub-setting">
                    <div className="setting-label">
                      <span>
                        {t(
                          "voiceCommands.fuzzyMatching.levenshteinThreshold",
                          "Character tolerance",
                        )}
                      </span>
                      <span className="setting-sublabel">
                        {Math.round(
                          (settings.voice_command_levenshtein_threshold ??
                            0.3) * 100,
                        )}
                        %
                      </span>
                    </div>
                    <input
                      type="range"
                      min="0.1"
                      max="0.5"
                      step="0.05"
                      value={
                        settings.voice_command_levenshtein_threshold ?? 0.3
                      }
                      onChange={(e) =>
                        updateSetting(
                          "voice_command_levenshtein_threshold",
                          parseFloat(e.target.value),
                        )
                      }
                      className="threshold-slider"
                    />
                  </div>
                )}

                <div className="setting-row">
                  <div className="setting-label">
                    <span>
                      {t(
                        "voiceCommands.fuzzyMatching.usePhonetic",
                        "Phonetic matching",
                      )}
                    </span>
                    <span className="setting-sublabel">
                      {t(
                        "voiceCommands.fuzzyMatching.usePhoneticDesc",
                        "Match similar-sounding words",
                      )}
                    </span>
                  </div>
                  <label className="toggle-switch">
                    <input
                      type="checkbox"
                      checked={settings.voice_command_use_phonetic ?? true}
                      onChange={(e) =>
                        updateSetting(
                          "voice_command_use_phonetic",
                          e.target.checked,
                        )
                      }
                    />
                    <span className="slider"></span>
                  </label>
                </div>

                {(settings.voice_command_use_phonetic ?? true) && (
                  <div className="setting-row sub-setting">
                    <div className="setting-label">
                      <span>
                        {t(
                          "voiceCommands.fuzzyMatching.phoneticBoost",
                          "Phonetic boost factor",
                        )}
                      </span>
                      <span className="setting-sublabel">
                        {Math.round(
                          (settings.voice_command_phonetic_boost ?? 0.5) * 100,
                        )}
                        %
                      </span>
                    </div>
                    <input
                      type="range"
                      min="0.3"
                      max="0.8"
                      step="0.05"
                      value={settings.voice_command_phonetic_boost ?? 0.5}
                      onChange={(e) =>
                        updateSetting(
                          "voice_command_phonetic_boost",
                          parseFloat(e.target.value),
                        )
                      }
                      className="threshold-slider"
                    />
                  </div>
                )}

                <div className="setting-row">
                  <div className="setting-label">
                    <span>
                      {t(
                        "voiceCommands.fuzzyMatching.wordSimilarityThreshold",
                        "Word match strictness",
                      )}
                    </span>
                    <span className="setting-sublabel">
                      {Math.round(
                        (settings.voice_command_word_similarity_threshold ??
                          0.7) * 100,
                      )}
                      %
                    </span>
                  </div>
                  <input
                    type="range"
                    min="0.5"
                    max="0.9"
                    step="0.05"
                    value={
                      settings.voice_command_word_similarity_threshold ?? 0.7
                    }
                    onChange={(e) =>
                      updateSetting(
                        "voice_command_word_similarity_threshold",
                        parseFloat(e.target.value),
                      )
                    }
                    className="threshold-slider"
                  />
                </div>
              </div>
            )}
          </div>

          <div className="setting-row">
            <div className="setting-label">
              <span>{t("voiceCommands.autoRun", "Auto Run")}</span>
              <span className="setting-sublabel">
                {t(
                  "voiceCommands.autoRunDescription",
                  "Auto-execute predefined commands after countdown",
                )}
              </span>
            </div>
            <div className="auto-run-controls">
              <input
                type="number"
                min="1"
                max="10"
                value={settings.voice_command_auto_run_seconds || 4}
                onChange={(e) =>
                  updateSetting(
                    "voice_command_auto_run_seconds",
                    Math.max(1, Math.min(10, parseInt(e.target.value) || 4)),
                  )
                }
                disabled={!settings.voice_command_auto_run}
                className="auto-run-seconds-input"
              />
              <span className="auto-run-seconds-label">
                {t("voiceCommands.seconds", "sec")}
              </span>
              <label className="toggle-switch">
                <input
                  type="checkbox"
                  checked={settings.voice_command_auto_run || false}
                  onChange={(e) =>
                    updateSetting("voice_command_auto_run", e.target.checked)
                  }
                />
                <span className="slider"></span>
              </label>
            </div>
          </div>

          {/* Execution Log Section */}
          <div className="execution-log-section">
            <div className="log-header">
              <h4>{t("voiceCommands.executionLog", "Execution Log")}</h4>
              <div className="log-actions">
                <button
                  className="btn-log-action"
                  onClick={handleCopyLog}
                  disabled={executionLog.length === 0}
                  title={t("voiceCommands.copyLog", "Copy log to clipboard")}
                >
                  📋 {t("voiceCommands.copy", "Copy")}
                </button>
                <button
                  className="btn-log-action"
                  onClick={handleClearLog}
                  disabled={executionLog.length === 0}
                  title={t("voiceCommands.clearLog", "Clear log")}
                >
                  🗑️ {t("voiceCommands.clear", "Clear")}
                </button>
              </div>
            </div>

            <div className="execution-log-container">
              {executionLog.length === 0 ? (
                <div className="log-empty">
                  {t(
                    "voiceCommands.noLogEntries",
                    "No commands executed yet. Run a command to see output here.",
                  )}
                </div>
              ) : (
                executionLog.map((entry) => (
                  <div
                    key={entry.id}
                    className={`log-entry ${entry.isError ? "error" : "success"}`}
                  >
                    <div className="log-entry-header">
                      <span className="log-time">
                        {formatTime(entry.timestamp)}
                      </span>
                      <div className="log-entry-actions">
                        <button
                          className="btn-copy-entry"
                          onClick={() => {
                            const text = `${entry.command}${entry.output ? `\n${entry.output}` : ""}`;
                            navigator.clipboard.writeText(text);
                          }}
                          title={t("voiceCommands.copyEntry", "Copy command")}
                        >
                          📋
                        </button>
                        <span
                          className={`log-status ${entry.isError ? "error" : entry.wasOpenedInWindow ? "opened" : "success"}`}
                        >
                          {entry.isError
                            ? "ERROR"
                            : entry.wasOpenedInWindow
                              ? "OPENED"
                              : "OK"}
                        </span>
                      </div>
                    </div>
                    <div className="log-command">{entry.command}</div>
                    {entry.spokenText && (
                      <div className="log-spoken">"{entry.spokenText}"</div>
                    )}
                    {entry.output && (
                      <div className="log-output">{entry.output}</div>
                    )}
                  </div>
                ))
              )}
              <div ref={logEndRef} />
            </div>
          </div>
          {/* Mock Testing Section */}
          <div className="mock-testing-section">
            <div className="section-divider">
              <span>{t("voiceCommands.mockTesting", "Mock Testing")}</span>
            </div>
            <p className="mock-description">
              {t(
                "voiceCommands.mockTestingDesc",
                "Test voice commands without speaking. Type text below and it will be processed as if spoken.",
              )}
            </p>
            <div className="mock-input-container">
              <input
                type="text"
                className="mock-input"
                value={mockInput}
                onChange={(e) => setMockInput(e.target.value)}
                placeholder={t(
                  "voiceCommands.mockPlaceholder",
                  "e.g., open notepad",
                )}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    handleMockTest();
                  }
                }}
              />
              <button
                className="btn-mock-test"
                onClick={handleMockTest}
                disabled={mockStatus?.type === "loading"}
              >
                {mockStatus?.type === "loading" ? "Testing..." : "🧪 Test"}
              </button>
            </div>
            {mockStatus && (
              <div className={`mock-status ${mockStatus.type}`}>
                {mockStatus.message}
              </div>
            )}
          </div>
        </>
      )}

      <style>{`
        .voice-command-settings {
          width: 100%;
        }
        .setting-section-header {
          margin-bottom: 20px;
        }
        .setting-section-header h3 {
          color: #f5f5f5;
          font-size: 16px;
          margin-bottom: 6px;
        }
        .setting-description {
          color: #888;
          font-size: 13px;
          line-height: 1.4;
        }
        .shortcut-row {
          padding: 14px 0;
          border-bottom: 1px solid rgba(255,255,255,0.06);
        }
        .shortcut-row > div {
          flex-direction: row !important;
          flex-wrap: nowrap !important;
          align-items: center !important;
          justify-content: space-between !important;
        }
        .setting-row {
          display: flex;
          justify-content: space-between;
          align-items: center;
          padding: 14px 0;
          border-bottom: 1px solid rgba(255,255,255,0.06);
        }
        .setting-label {
          display: flex;
          flex-direction: column;
          gap: 4px;
        }
        .setting-label > span:first-child {
          color: #e0e0e0;
          font-size: 14px;
        }
        .setting-sublabel {
          color: #666;
          font-size: 12px;
        }
        .threshold-slider {
          width: 140px;
        }

        /* LLM API Settings */
        .llm-api-section {
          margin-top: 12px;
          border: 1px solid rgba(255,255,255,0.08);
          border-radius: 10px;
          background: rgba(255,255,255,0.02);
          overflow: visible;
          position: relative;
          z-index: 5;
        }
        .llm-api-toggle {
          width: 100%;
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 12px;
          padding: 12px 14px;
          background: rgba(255,255,255,0.04);
          border: none;
          cursor: pointer;
          text-align: left;
        }
        .llm-api-toggle:hover {
          background: rgba(255,255,255,0.06);
        }
        .llm-api-toggle-text {
          display: flex;
          flex-direction: column;
          gap: 4px;
        }
        .llm-api-title {
          color: #e0e0e0;
          font-size: 13px;
          font-weight: 600;
        }
        .llm-api-sublabel {
          color: #666;
          font-size: 12px;
        }
        .llm-api-toggle-icon {
          width: 22px;
          text-align: center;
          color: #888;
          font-size: 16px;
        }
        .llm-api-content {
          display: flex;
          flex-direction: column;
          gap: 10px;
          padding: 6px 14px 12px;
          position: relative;
          z-index: 6;
        }
        .llm-api-row {
          position: relative;
          z-index: 1;
        }
        .llm-api-row-provider {
          z-index: 20;
        }
        .llm-api-control {
          min-width: 240px;
          position: relative;
          z-index: 7;
        }
        .llm-api-extended {
          margin-top: 6px;
        }
        .llm-api-note {
          background: rgba(138, 43, 226, 0.08);
          border: 1px solid rgba(138, 43, 226, 0.25);
          color: #b59cff;
          font-size: 12px;
          padding: 10px 12px;
          border-radius: 8px;
        }
        .llm-api-apple-note {
          background: rgba(255,255,255,0.04);
          border: 1px solid rgba(255,255,255,0.12);
          color: #bbb;
          font-size: 12px;
          padding: 10px 12px;
          border-radius: 8px;
        }
        .llm-api-apple-title {
          font-weight: 600;
          margin-bottom: 6px;
          color: #ddd;
        }
        .llm-api-apple-requirements {
          margin-top: 6px;
          color: #888;
        }
        .llm-api-model-row {
          display: flex;
          align-items: center;
          gap: 8px;
        }
        .llm-api-model-select {
          min-width: 320px;
        }
        .llm-api-refresh {
          height: 40px;
          width: 40px;
          display: inline-flex;
          align-items: center;
          justify-content: center;
        }
        
        /* Execution Settings */
        .execution-settings-section {
          margin-top: 16px;
          padding-top: 8px;
        }
        .section-divider {
          border-bottom: 1px solid rgba(138, 43, 226, 0.3);
          margin-bottom: 8px;
          padding-bottom: 8px;
        }
        .section-divider span {
          color: #8a2be2;
          font-size: 12px;
          font-weight: 600;
          text-transform: uppercase;
          letter-spacing: 0.5px;
        }
        .execution-defaults-description {
          color: #888;
          font-size: 12px;
          margin-bottom: 12px;
          line-height: 1.4;
        }
        .execution-policy-select {
          background: rgba(0,0,0,0.3);
          border: 1px solid rgba(255,255,255,0.1);
          border-radius: 8px;
          padding: 8px 12px;
          color: #e0e0e0;
          font-size: 13px;
          min-width: 200px;
          cursor: pointer;
        }
        .execution-policy-select:focus {
          outline: none;
          border-color: rgba(138, 43, 226, 0.5);
        }
        .timeout-input-container {
          display: flex;
          align-items: center;
          gap: 8px;
        }
        .timeout-input {
          width: 80px;
          background: rgba(0,0,0,0.3);
          border: 1px solid rgba(255,255,255,0.1);
          border-radius: 8px;
          padding: 8px 12px;
          color: #e0e0e0;
          font-size: 13px;
          text-align: center;
        }
        .timeout-input:focus {
          outline: none;
          border-color: rgba(138, 43, 226, 0.5);
        }
        .timeout-label {
          color: #888;
          font-size: 13px;
        }

        /* Fuzzy Matching Settings */
        .fuzzy-matching-section {
          margin-top: 12px;
          border: 1px solid rgba(255,255,255,0.08);
          border-radius: 10px;
          background: rgba(255,255,255,0.02);
          overflow: hidden;
        }
        .fuzzy-matching-toggle {
          width: 100%;
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 12px;
          padding: 12px 14px;
          background: rgba(255,255,255,0.04);
          border: none;
          cursor: pointer;
          text-align: left;
        }
        .fuzzy-matching-toggle:hover {
          background: rgba(255,255,255,0.06);
        }
        .fuzzy-matching-toggle-text {
          display: flex;
          flex-direction: column;
          gap: 4px;
        }
        .fuzzy-matching-title {
          color: #e0e0e0;
          font-size: 13px;
          font-weight: 600;
        }
        .fuzzy-matching-sublabel {
          color: #666;
          font-size: 12px;
        }
        .fuzzy-matching-toggle-icon {
          width: 22px;
          text-align: center;
          color: #888;
          font-size: 16px;
        }
        .fuzzy-matching-content {
          display: flex;
          flex-direction: column;
          gap: 10px;
          padding: 6px 14px 12px;
        }
        .sub-setting {
          padding-left: 20px;
          border-left: 2px solid rgba(138, 43, 226, 0.3);
          margin-left: 10px;
        }
        
        /* Voice Command Card Execution Options */
        .voice-command-execution-section {
          margin-top: 12px;
          border-top: 1px solid rgba(255,255,255,0.08);
          padding-top: 12px;
        }
        .execution-toggle {
          background: transparent;
          border: none;
          color: #888;
          font-size: 13px;
          cursor: pointer;
          padding: 4px 0;
          display: flex;
          align-items: center;
          gap: 6px;
        }
        .execution-toggle:hover {
          color: #aaa;
        }
        .execution-options-content {
          margin-top: 12px;
          padding-left: 16px;
          border-left: 2px solid rgba(138, 43, 226, 0.3);
          display: flex;
          flex-direction: column;
          gap: 10px;
        }
        .execution-option-row {
          display: flex;
          justify-content: space-between;
          align-items: center;
          font-size: 12px;
          color: #aaa;
        }
        .execution-option-row.column {
          flex-direction: column;
          align-items: flex-start;
          gap: 6px;
        }
        .execution-policy-select-small {
          background: rgba(0,0,0,0.3);
          border: 1px solid rgba(255,255,255,0.1);
          border-radius: 6px;
          padding: 5px 8px;
          color: #e0e0e0;
          font-size: 11px;
          min-width: 160px;
          cursor: pointer;
        }
        .execution-policy-select-small:focus {
          outline: none;
          border-color: rgba(138, 43, 226, 0.5);
        }
        .working-directory-input {
          width: 100%;
          background: rgba(0,0,0,0.3);
          border: 1px solid rgba(255,255,255,0.1);
          border-radius: 6px;
          padding: 6px 8px;
          color: #e0e0e0;
          font-size: 12px;
        }
        .working-directory-input:focus {
          outline: none;
          border-color: rgba(138, 43, 226, 0.5);
        }
        .working-directory-input::placeholder {
          color: #666;
          font-style: italic;
        }
        
        /* Execution Log */
        .execution-log-section {
          margin-top: 32px;
          border-top: 1px solid rgba(255,255,255,0.1);
          padding-top: 24px;
        }
        .log-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 12px;
        }
        .log-header h4 {
          color: #ccc;
          font-size: 14px;
          font-weight: 500;
        }
        .log-actions {
          display: flex;
          gap: 8px;
        }
        .btn-log-action {
          background: rgba(255,255,255,0.06);
          border: 1px solid rgba(255,255,255,0.1);
          border-radius: 6px;
          padding: 6px 12px;
          color: #999;
          font-size: 12px;
          cursor: pointer;
          transition: all 0.2s;
        }
        .btn-log-action:hover:not(:disabled) {
          background: rgba(255,255,255,0.1);
          color: #fff;
        }
        .btn-log-action:disabled {
          opacity: 0.4;
          cursor: not-allowed;
        }
        .execution-log-container {
          max-height: 300px;
          overflow-y: auto;
          background: rgba(0,0,0,0.2);
          border: 1px solid rgba(255,255,255,0.08);
          border-radius: 8px;
          padding: 8px;
        }
        .log-empty {
          color: #666;
          font-size: 13px;
          text-align: center;
          padding: 24px;
          font-style: italic;
        }
        .log-entry {
          background: rgba(255,255,255,0.03);
          border-radius: 6px;
          padding: 10px 12px;
          margin-bottom: 8px;
          border-left: 3px solid #4caf50;
        }
        .log-entry.error {
          border-left-color: #f44336;
        }
        .log-entry-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 6px;
        }
        .log-time {
          color: #888;
          font-size: 11px;
          font-family: monospace;
        }
        .log-status {
          font-size: 10px;
          font-weight: 600;
          padding: 2px 6px;
          border-radius: 4px;
        }
        .log-status.success {
          background: rgba(76, 175, 80, 0.2);
          color: #4caf50;
        }
        .log-status.error {
          background: rgba(244, 67, 54, 0.2);
          color: #f44336;
        }
        .log-status.opened {
          background: rgba(33, 150, 243, 0.2);
          color: #2196f3;
        }
        .log-entry-actions {
          display: flex;
          align-items: center;
          gap: 8px;
        }
        .btn-copy-entry {
          background: transparent;
          border: none;
          padding: 2px 4px;
          cursor: pointer;
          opacity: 0.5;
          font-size: 12px;
          transition: opacity 0.2s;
        }
        .btn-copy-entry:hover {
          opacity: 1;
        }
        .log-entry:hover .btn-copy-entry {
          opacity: 0.7;
        }
        .log-command {
          color: #4fc3f7;
          font-family: 'Consolas', monospace;
          font-size: 12px;
          word-break: break-all;
        }
        .log-spoken {
          color: #888;
          font-size: 11px;
          font-style: italic;
          margin-top: 4px;
        }
        .log-output {
          margin-top: 6px;
          padding: 8px;
          background: rgba(0,0,0,0.3);
          border-radius: 4px;
          color: #ccc;
          font-family: 'Consolas', monospace;
          font-size: 11px;
          white-space: pre-wrap;
          word-break: break-all;
          max-height: 100px;
          overflow-y: auto;
        }
        
        .voice-commands-list {
          margin-top: 24px;
        }
        .list-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 16px;
        }
        .list-header h4 {
          color: #ccc;
          font-size: 14px;
          font-weight: 500;
        }
        .btn-add {
          background: #9b5de5;
          color: white;
          border: none;
          padding: 8px 16px;
          border-radius: 8px;
          font-size: 13px;
          cursor: pointer;
          transition: transform 0.1s;
        }
        .btn-add:hover {
          transform: scale(1.02);
        }
        .empty-state {
          background: rgba(255,255,255,0.03);
          border: 1px dashed rgba(255,255,255,0.1);
          border-radius: 12px;
          padding: 24px;
          text-align: center;
        }
        .empty-state p {
          color: #888;
          font-size: 14px;
        }
        .empty-state .hint {
          color: #666;
          font-size: 12px;
          margin-top: 8px;
        }
        .voice-command-card {
          background: rgba(255,255,255,0.04);
          border: 1px solid rgba(255,255,255,0.08);
          border-radius: 12px;
          padding: 16px;
          margin-bottom: 12px;
        }
        .voice-command-card.disabled {
          opacity: 0.5;
        }
        .voice-command-card.editing {
          background: rgba(138, 43, 226, 0.08);
          border-color: rgba(138, 43, 226, 0.3);
        }
        .voice-command-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 8px;
        }
        .voice-command-name {
          color: #f5f5f5;
          font-weight: 600;
          font-size: 14px;
        }
        .voice-command-controls {
          display: flex;
          align-items: center;
          gap: 8px;
        }
        .voice-command-phrase {
          color: #888;
          font-size: 13px;
          font-style: italic;
          margin-bottom: 6px;
        }
        .voice-command-script {
          color: #4fc3f7;
          font-family: 'Consolas', monospace;
          font-size: 12px;
          background: rgba(0,0,0,0.2);
          padding: 6px 10px;
          border-radius: 6px;
          overflow-x: auto;
        }
        .voice-command-field {
          margin-bottom: 14px;
        }
        .voice-command-field label {
          display: block;
          color: #aaa;
          font-size: 12px;
          margin-bottom: 6px;
        }
        .voice-command-field input[type="text"] {
          width: 100%;
          background: rgba(0,0,0,0.3);
          border: 1px solid rgba(255,255,255,0.1);
          border-radius: 8px;
          padding: 10px 12px;
          color: #f5f5f5;
          font-size: 13px;
        }
        .voice-command-field input.mono {
          font-family: 'Consolas', monospace;
          color: #4fc3f7;
        }
        .voice-command-field input[type="range"] {
          width: 100%;
        }
        .voice-command-actions {
          display: flex;
          justify-content: flex-end;
          gap: 10px;
          margin-top: 16px;
        }
        .btn-cancel, .btn-save, .btn-edit, .btn-delete {
          padding: 8px 16px;
          border-radius: 8px;
          font-size: 13px;
          cursor: pointer;
          border: none;
        }
        .btn-cancel {
          background: rgba(255,255,255,0.08);
          color: #999;
        }
        .btn-save {
          background: #4caf50;
          color: white;
        }
        .btn-edit, .btn-delete {
          padding: 6px 10px;
          background: transparent;
          font-size: 14px;
        }
        .toggle-switch {
          position: relative;
          display: inline-block;
          width: 44px;
          height: 24px;
        }
        .toggle-switch.small {
          width: 36px;
          height: 20px;
        }
        .toggle-switch input {
          opacity: 0;
          width: 0;
          height: 0;
        }
        .toggle-switch .slider {
          position: absolute;
          cursor: pointer;
          inset: 0;
          background: rgba(255,255,255,0.1);
          border-radius: 24px;
          transition: 0.2s;
        }
        .toggle-switch .slider:before {
          content: "";
          position: absolute;
          height: 18px;
          width: 18px;
          left: 3px;
          bottom: 3px;
          background: white;
          border-radius: 50%;
          transition: 0.2s;
        }
        .toggle-switch.small .slider:before {
          height: 14px;
          width: 14px;
        }
        .toggle-switch input:checked + .slider {
          background: #8a2be2;
        }
        .toggle-switch input:checked + .slider:before {
          transform: translateX(20px);
        }
        .toggle-switch.small input:checked + .slider:before {
          transform: translateX(16px);
        }
        .system-prompt-row {
          flex-direction: column;
          align-items: stretch;
          gap: 12px;
        }
        .system-prompt-container {
          position: relative;
          width: 100%;
        }
        .system-prompt-textarea {
          width: 100%;
          min-height: 160px;
          background: rgba(0,0,0,0.3);
          border: 1px solid rgba(255,255,255,0.1);
          border-radius: 8px;
          padding: 12px;
          padding-right: 40px;
          color: #e0e0e0;
          font-size: 12px;
          font-family: 'Consolas', 'Monaco', monospace;
          line-height: 1.5;
          resize: vertical;
        }
        .system-prompt-textarea:focus {
          outline: none;
          border-color: rgba(138, 43, 226, 0.5);
        }
        .btn-reset-prompt {
          position: absolute;
          top: 8px;
          right: 8px;
          background: rgba(255,255,255,0.08);
          border: none;
          border-radius: 6px;
          padding: 6px 10px;
          color: #888;
          font-size: 16px;
          cursor: pointer;
          transition: all 0.2s;
        }
        .btn-reset-prompt:hover {
          background: rgba(138, 43, 226, 0.3);
          color: #fff;
        }
        
        /* Mock Testing */
        .mock-testing-section {
          margin-top: 32px;
          border-top: 1px solid rgba(255,255,255,0.1);
          padding-top: 24px;
        }
        .mock-description {
          color: #888;
          font-size: 13px;
          margin-bottom: 16px;
          line-height: 1.4;
        }
        .mock-input-container {
          display: flex;
          gap: 10px;
        }
        .mock-input {
          flex: 1;
          background: rgba(0,0,0,0.3);
          border: 1px solid rgba(255,255,255,0.1);
          border-radius: 8px;
          padding: 10px 14px;
          color: #f5f5f5;
          font-size: 14px;
        }
        .mock-input:focus {
          outline: none;
          border-color: rgba(138, 43, 226, 0.5);
        }
        .mock-input::placeholder {
          color: #666;
          font-style: italic;
        }
        .btn-mock-test {
          background: #9b5de5;
          color: white;
          border: none;
          padding: 10px 20px;
          border-radius: 8px;
          font-size: 14px;
          font-weight: 500;
          cursor: pointer;
          transition: all 0.2s;
          white-space: nowrap;
        }
        .btn-mock-test:hover:not(:disabled) {
          transform: scale(1.02);
          box-shadow: 0 4px 12px rgba(155, 93, 229, 0.3);
        }
        .btn-mock-test:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }
        .mock-status {
          margin-top: 12px;
          padding: 10px 14px;
          border-radius: 8px;
          font-size: 13px;
        }
        .mock-status.success {
          background: rgba(76, 175, 80, 0.15);
          border: 1px solid rgba(76, 175, 80, 0.3);
          color: #4caf50;
        }
        .mock-status.error {
          background: rgba(244, 67, 54, 0.15);
          border: 1px solid rgba(244, 67, 54, 0.3);
          color: #f44336;
        }
        .mock-status.loading {
          background: rgba(33, 150, 243, 0.15);
          border: 1px solid rgba(33, 150, 243, 0.3);
          color: #2196f3;
        }
      `}</style>
    </div>
  );
}
