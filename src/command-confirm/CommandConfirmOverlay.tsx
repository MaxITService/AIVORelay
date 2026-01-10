import { useEffect, useState } from "react";
import { listen, emit } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { commands } from "@/bindings";

interface CommandConfirmPayload {
  command: string;
  spoken_text: string;
  from_llm: boolean;
  // Execution settings passed from backend
  ps_args?: string;
  keep_window_open?: boolean;
  use_windows_terminal?: boolean;
}

/** Payload emitted after command execution (for history tracking) */
export interface VoiceCommandResultPayload {
  timestamp: number;
  command: string;
  spokenText: string;
  output: string;
  isError: boolean;
  wasOpenedInWindow: boolean;
}

type Status = null | { type: "success"; message: string } | { type: "error"; message: string };

/** Helper to hide the current window - handles the async nature of hide() */
const hideWindow = () => {
  getCurrentWindow().hide().catch((err) => {
    console.error("Failed to hide window:", err);
  });
};

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
    
    // Get execution settings with defaults
    const psArgs = payload.ps_args ?? "-NoProfile -NonInteractive";
    const keepWindowOpen = payload.keep_window_open ?? false;
    const useWindowsTerminal = payload.use_windows_terminal ?? true;
    
    try {
      const result = await commands.executeVoiceCommand(
        commandToRun,
        psArgs,
        keepWindowOpen,
        useWindowsTerminal
      );
      
      if (result.status === "ok") {
        const output = result.data;
        setStatus({ type: "success", message: keepWindowOpen ? "Opened in terminal" : "Command executed successfully" });
        
        // Emit result for history tracking
        await emit("voice-command-result", {
          timestamp: Date.now(),
          command: commandToRun,
          spokenText: payload.spoken_text,
          output: output,
          isError: false,
          wasOpenedInWindow: keepWindowOpen,
        } as VoiceCommandResultPayload);
        
        // Auto-hide after success
        setTimeout(() => {
          hideWindow();
        }, 1000);
      } else {
        const errorMsg = result.error || "Execution failed";
        setStatus({ type: "error", message: errorMsg });
        
        // Emit error for history tracking
        await emit("voice-command-result", {
          timestamp: Date.now(),
          command: commandToRun,
          spokenText: payload.spoken_text,
          output: errorMsg,
          isError: true,
          wasOpenedInWindow: false,
        } as VoiceCommandResultPayload);
      }
    } catch (err) {
      const errorMsg = String(err);
      setStatus({ type: "error", message: errorMsg });
      
      // Emit error for history tracking
      await emit("voice-command-result", {
        timestamp: Date.now(),
        command: commandToRun,
        spokenText: payload.spoken_text,
        output: errorMsg,
        isError: true,
        wasOpenedInWindow: false,
      } as VoiceCommandResultPayload);
    } finally {
      setIsExecuting(false);
    }
  };

  const handleEdit = () => {
    setIsEditing(true);
    setStatus(null);
  };

  const handleCancel = () => {
    hideWindow();
  };

  // Handle keyboard shortcuts - separate effect for ESC to ensure it always works
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        hideWindow();
      }
    };
    
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);
  
  // Handle Ctrl+Enter to run command
  useEffect(() => {
    if (!payload) return;
    
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Enter" && (e.ctrlKey || e.metaKey) && !isExecuting) {
        e.preventDefault();
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
