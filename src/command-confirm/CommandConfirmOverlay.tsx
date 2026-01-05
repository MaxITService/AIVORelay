import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { commands } from "@/bindings";

interface CommandConfirmPayload {
  command: string;
  spoken_text: string;
  from_llm: boolean;
}

type Status = null | { type: "success"; message: string } | { type: "error"; message: string };

export default function CommandConfirmOverlay() {
  const [payload, setPayload] = useState<CommandConfirmPayload | null>(null);
  const [isEditing, setIsEditing] = useState(false);
  const [editedCommand, setEditedCommand] = useState("");
  const [status, setStatus] = useState<Status>(null);
  const [isExecuting, setIsExecuting] = useState(false);

  useEffect(() => {
    const unlisten = listen<CommandConfirmPayload>("show-command-confirm", (event) => {
      setPayload(event.payload);
      setEditedCommand(event.payload.command);
      setIsEditing(false);
      setStatus(null);
      setIsExecuting(false);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleRun = async () => {
    if (!payload || isExecuting) return;
    
    setIsExecuting(true);
    const commandToRun = isEditing ? editedCommand : payload.command;
    
    try {
      const result = await commands.executeVoiceCommand(commandToRun);
      if (result.status === "ok") {
        setStatus({ type: "success", message: "Command executed successfully" });
        // Auto-hide after success
        setTimeout(() => {
          getCurrentWindow().hide();
        }, 1000);
      } else {
        setStatus({ type: "error", message: result.error || "Execution failed" });
      }
    } catch (err) {
      setStatus({ type: "error", message: String(err) });
    } finally {
      setIsExecuting(false);
    }
  };

  const handleEdit = () => {
    setIsEditing(true);
    setStatus(null);
  };

  const handleCancel = () => {
    getCurrentWindow().hide();
  };

  // Handle keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        handleCancel();
      } else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
        handleRun();
      }
    };
    
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [payload, isEditing, editedCommand, isExecuting]);

  if (!payload) {
    return null;
  }

  return (
    <div className="command-confirm-container">
      <div className="command-confirm-header">
        <svg className="command-confirm-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <path d="M4 17l6-6-6-6M12 19h8" strokeLinecap="round" strokeLinejoin="round"/>
        </svg>
        <span className="command-confirm-title">Voice Command</span>
        <span className={`command-confirm-source ${payload.from_llm ? "llm" : ""}`}>
          {payload.from_llm ? "AI Generated" : "Matched"}
        </span>
      </div>

      {payload.spoken_text && (
        <div className="command-confirm-spoken">
          "{payload.spoken_text}"
        </div>
      )}

      {isEditing ? (
        <textarea
          className="command-confirm-edit-area"
          value={editedCommand}
          onChange={(e) => setEditedCommand(e.target.value)}
          autoFocus
          spellCheck={false}
        />
      ) : (
        <div className="command-confirm-code">
          {payload.command}
        </div>
      )}

      <div className="command-confirm-buttons">
        <button 
          className="command-confirm-btn cancel" 
          onClick={handleCancel}
          disabled={isExecuting}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M18 6L6 18M6 6l12 12" strokeLinecap="round"/>
          </svg>
          Cancel
        </button>
        
        {!isEditing && (
          <button 
            className="command-confirm-btn edit" 
            onClick={handleEdit}
            disabled={isExecuting}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z" strokeLinecap="round" strokeLinejoin="round"/>
            </svg>
            Edit
          </button>
        )}
        
        <button 
          className="command-confirm-btn run" 
          onClick={handleRun}
          disabled={isExecuting}
        >
          <svg viewBox="0 0 24 24" fill="currentColor">
            <path d="M8 5v14l11-7z"/>
          </svg>
          {isExecuting ? "Running..." : "Run"}
        </button>
      </div>

      {status && (
        <div className={`command-confirm-status ${status.type}`}>
          {status.message}
        </div>
      )}
    </div>
  );
}
