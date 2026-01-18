import { useEffect, useState } from "react";
import { listen, emit } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { commands } from "@/bindings";

interface CommandConfirmPayload {
  command: string;
  spoken_text: string;
  from_llm: boolean;
  // Execution settings passed from backend
  template?: string;
  keep_window_open?: boolean;
  // Auto-run settings (only for predefined commands)
  auto_run?: boolean;
  auto_run_seconds?: number;
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
  // Auto-run countdown state
  const [countdownMs, setCountdownMs] = useState<number>(0);
  const [isPaused, setIsPaused] = useState(false);

  // Whether auto-run is active for current payload
  const isAutoRunActive = payload?.auto_run && !payload.from_llm && countdownMs > 0 && !isEditing && !status;

  useEffect(() => {
    const unlisten = listen<CommandConfirmPayload>("show-command-confirm", (event) => {
      setPayload(event.payload);
      setEditedCommand(event.payload.command);
      setIsEditing(false);
      setStatus(null);
      setIsExecuting(false);
      setIsPaused(false);
      // Initialize countdown if auto_run is enabled for predefined commands
      if (event.payload.auto_run && !event.payload.from_llm && event.payload.auto_run_seconds) {
        setCountdownMs(event.payload.auto_run_seconds * 1000);
      } else {
        setCountdownMs(0);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Countdown timer effect
  useEffect(() => {
    if (!payload?.auto_run || payload.from_llm || isPaused || isEditing || status || isExecuting) {
      return;
    }

    if (countdownMs <= 0) {
      return;
    }

    const interval = setInterval(() => {
      setCountdownMs((prev) => {
        const next = prev - 50;
        if (next <= 0) {
          return 0;
        }
        return next;
      });
    }, 50);

    return () => clearInterval(interval);
  }, [payload, isPaused, isEditing, status, isExecuting, countdownMs > 0]);

  // Auto-execute when countdown reaches 0
  useEffect(() => {
    if (countdownMs === 0 && payload?.auto_run && !payload.from_llm && !isEditing && !status && !isExecuting) {
      // Check if we actually had a countdown (auto_run_seconds > 0)
      if (payload.auto_run_seconds && payload.auto_run_seconds > 0) {
        handleRun();
      }
    }
  }, [countdownMs]);

  const handleRun = async () => {
    if (!payload || isExecuting) return;
    
    setIsExecuting(true);
    const commandToRun = isEditing ? editedCommand : payload.command;
    
    // Get execution settings with defaults
    const template = payload.template ?? 'powershell -NonInteractive -Command "${command}"';
    const keepWindowOpen = payload.keep_window_open ?? false;
    
    try {
      const result = await commands.executeVoiceCommand(
        commandToRun,
        template,
        keepWindowOpen
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
    setCountdownMs(0); // Stop auto-run when editing
  };

  const handleContainerClick = (e: React.MouseEvent) => {
    // Only toggle pause if clicking on the container background (not buttons/inputs)
    if ((e.target as HTMLElement).closest('button, textarea, .command-confirm-code')) {
      return;
    }
    if (isAutoRunActive || (isPaused && payload?.auto_run && !payload.from_llm && !isEditing && !status)) {
      setIsPaused((prev) => !prev);
    }
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

  // Calculate progress percentage for auto-run bar
  const totalMs = (payload.auto_run_seconds ?? 0) * 1000;
  const progressPercent = totalMs > 0 ? (countdownMs / totalMs) * 100 : 0;
  const showAutoRunBar = payload.auto_run && !payload.from_llm && !isEditing && !status && (countdownMs > 0 || isPaused);

  return (
    <div
      className={`command-confirm-container ${isPaused && showAutoRunBar ? "paused" : ""}`}
      onClick={handleContainerClick}
    >
      {/* Auto-run progress bar */}
      {showAutoRunBar && (
        <div className="auto-run-progress-container">
          <div
            className={`auto-run-progress-bar ${isPaused ? "paused" : ""}`}
            style={{ width: `${progressPercent}%` }}
          />
          {isPaused && <span className="auto-run-paused-text">Paused</span>}
        </div>
      )}

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
