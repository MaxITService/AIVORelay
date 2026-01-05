import { useState } from "react";
import { useSettings } from "@/hooks/useSettings";
import { useTranslation } from "react-i18next";
import { VoiceCommand } from "@/bindings";

interface VoiceCommandCardProps {
  command: VoiceCommand;
  onUpdate: (updated: VoiceCommand) => void;
  onDelete: () => void;
}

function VoiceCommandCard({ command, onUpdate, onDelete }: VoiceCommandCardProps) {
  const [isEditing, setIsEditing] = useState(false);
  const [editName, setEditName] = useState(command.name);
  const [editPhrase, setEditPhrase] = useState(command.trigger_phrase);
  const [editScript, setEditScript] = useState(command.script);
  const [editThreshold, setEditThreshold] = useState(command.similarity_threshold);

  const handleSave = () => {
    onUpdate({
      ...command,
      name: editName,
      trigger_phrase: editPhrase,
      script: editScript,
      similarity_threshold: editThreshold,
    });
    setIsEditing(false);
  };

  const handleCancel = () => {
    setEditName(command.name);
    setEditPhrase(command.trigger_phrase);
    setEditScript(command.script);
    setEditThreshold(command.similarity_threshold);
    setIsEditing(false);
  };

  if (isEditing) {
    return (
      <div className="voice-command-card editing">
        <div className="voice-command-field">
          <label>Name</label>
          <input
            type="text"
            value={editName}
            onChange={(e) => setEditName(e.target.value)}
            placeholder="Lock Computer"
          />
        </div>
        <div className="voice-command-field">
          <label>Trigger Phrase</label>
          <input
            type="text"
            value={editPhrase}
            onChange={(e) => setEditPhrase(e.target.value)}
            placeholder="lock computer"
          />
        </div>
        <div className="voice-command-field">
          <label>Script/Command</label>
          <input
            type="text"
            value={editScript}
            onChange={(e) => setEditScript(e.target.value)}
            placeholder="rundll32.exe user32.dll,LockWorkStation"
            className="mono"
          />
        </div>
        <div className="voice-command-field">
          <label>Match Threshold: {Math.round(editThreshold * 100)}%</label>
          <input
            type="range"
            min="0.5"
            max="1"
            step="0.05"
            value={editThreshold}
            onChange={(e) => setEditThreshold(parseFloat(e.target.value))}
          />
        </div>
        <div className="voice-command-actions">
          <button className="btn-cancel" onClick={handleCancel}>Cancel</button>
          <button className="btn-save" onClick={handleSave}>Save</button>
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
              onChange={(e) => onUpdate({ ...command, enabled: e.target.checked })}
            />
            <span className="slider"></span>
          </label>
          <button className="btn-edit" onClick={() => setIsEditing(true)}>✏️</button>
          <button className="btn-delete" onClick={onDelete}>🗑️</button>
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
  
  if (!settings) return null;

  const handleAddCommand = () => {
    const newCommand: VoiceCommand = {
      id: `vc_${Date.now()}`,
      name: "New Command",
      trigger_phrase: "",
      script: "",
      similarity_threshold: settings.voice_command_default_threshold || 0.75,
      enabled: true,
    };
    updateSetting("voice_commands", [...(settings.voice_commands || []), newCommand]);
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

  return (
    <div className="voice-command-settings">
      <div className="setting-section-header">
        <h3>{t("voiceCommands.title", "Voice Command Center")}</h3>
        <p className="setting-description">
          {t("voiceCommands.description", "Define trigger phrases that execute scripts. If no match is found, an LLM can suggest a PowerShell command.")}
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
            onChange={(e) => updateSetting("voice_command_enabled", e.target.checked)}
          />
          <span className="slider"></span>
        </label>
      </div>

      {settings.voice_command_enabled && (
        <>
          <div className="setting-row">
            <div className="setting-label">
              <span>{t("voiceCommands.llmFallback", "LLM Fallback")}</span>
              <span className="setting-sublabel">
                {t("voiceCommands.llmFallbackDesc", "Use AI to generate commands when no predefined match is found")}
              </span>
            </div>
            <label className="toggle-switch">
              <input
                type="checkbox"
                checked={settings.voice_command_llm_fallback ?? true}
                onChange={(e) => updateSetting("voice_command_llm_fallback", e.target.checked)}
              />
              <span className="slider"></span>
            </label>
          </div>

          <div className="setting-row">
            <div className="setting-label">
              <span>{t("voiceCommands.defaultThreshold", "Default Match Threshold")}</span>
              <span className="setting-sublabel">
                {Math.round((settings.voice_command_default_threshold || 0.75) * 100)}%
              </span>
            </div>
            <input
              type="range"
              min="0.5"
              max="1"
              step="0.05"
              value={settings.voice_command_default_threshold || 0.75}
              onChange={(e) => updateSetting("voice_command_default_threshold", parseFloat(e.target.value))}
              className="threshold-slider"
            />
          </div>

          <div className="voice-commands-list">
            <div className="list-header">
              <h4>{t("voiceCommands.predefinedCommands", "Predefined Commands")}</h4>
              <button className="btn-add" onClick={handleAddCommand}>
                + {t("voiceCommands.addCommand", "Add Command")}
              </button>
            </div>

            {(settings.voice_commands || []).length === 0 ? (
              <div className="empty-state">
                <p>{t("voiceCommands.noCommands", "No commands defined yet. Add one to get started!")}</p>
                <p className="hint">{t("voiceCommands.hint", "Example: \"lock computer\" → rundll32.exe user32.dll,LockWorkStation")}</p>
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
          background: linear-gradient(135deg, #8a2be2 0%, #9c27b0 100%);
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
      `}</style>
    </div>
  );
}
