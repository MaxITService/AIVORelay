import React, { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { Wifi, WifiOff, Server, AlertTriangle, Copy, Check } from "lucide-react";
import { SettingContainer } from "../../ui/SettingContainer";

// Types matching Rust backend
type ExtensionStatus = "online" | "offline" | "unknown";

interface ConnectorStatusResponse {
  status: ExtensionStatus;
  last_poll_at: number;
  server_running: boolean;
  port: number;
  server_error: string | null;
}

interface ConnectorStatusIndicatorProps {
  grouped?: boolean;
  descriptionMode?: "inline" | "tooltip" | "none";
}

/**
 * Format a timestamp into a human-readable "time ago" string
 */
function formatTimeAgo(timestamp: number, t: (key: string, options?: any) => string): string {
  if (timestamp === 0) {
    return t("settings.browserConnector.status.never");
  }

  const now = Date.now();
  const diffMs = now - timestamp;
  const diffSec = Math.floor(diffMs / 1000);
  const diffMin = Math.floor(diffSec / 60);
  const diffHour = Math.floor(diffMin / 60);

  if (diffSec < 10) {
    return t("settings.browserConnector.status.justNow");
  } else if (diffSec < 60) {
    return t("settings.browserConnector.status.secondsAgo", { count: diffSec });
  } else if (diffMin < 60) {
    return t("settings.browserConnector.status.minutesAgo", { count: diffMin });
  } else {
    return t("settings.browserConnector.status.hoursAgo", { count: diffHour });
  }
}

export const ConnectorStatusIndicator: React.FC<ConnectorStatusIndicatorProps> = ({
  grouped = false,
  descriptionMode = "tooltip",
}) => {
  const { t } = useTranslation();
  const [status, setStatus] = useState<ConnectorStatusResponse | null>(null);
  const [lastSeenText, setLastSeenText] = useState<string>("");
  const [errorCopied, setErrorCopied] = useState(false);

  // Fetch status from backend
  const fetchStatus = useCallback(async () => {
    try {
      const result = await invoke<ConnectorStatusResponse>("connector_get_status");
      setStatus(result);
    } catch (error) {
      console.error("Failed to get connector status:", error);
    }
  }, []);

  // Update "last seen" text periodically
  useEffect(() => {
    if (status && status.status === "offline" && status.last_poll_at > 0) {
      setLastSeenText(formatTimeAgo(status.last_poll_at, t));

      const interval = setInterval(() => {
        setLastSeenText(formatTimeAgo(status.last_poll_at, t));
      }, 10000); // Update every 10 seconds

      return () => clearInterval(interval);
    } else {
      setLastSeenText("");
    }
  }, [status, t]);

  // Initial fetch and periodic polling
  useEffect(() => {
    fetchStatus();

    const interval = setInterval(fetchStatus, 5000); // Poll every 5 seconds

    return () => clearInterval(interval);
  }, [fetchStatus]);

  // Listen for status change events from backend
  useEffect(() => {
    const unlisten = listen<ExtensionStatus>("extension-status-changed", () => {
      // Refetch full status when event received
      fetchStatus();
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [fetchStatus]);

  // Listen for server error events from backend
  useEffect(() => {
    const unlisten = listen<string>("connector-server-error", () => {
      // Refetch full status to get the error
      fetchStatus();
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [fetchStatus]);

  // Copy error to clipboard
  const handleCopyError = () => {
    if (status?.server_error) {
      void navigator.clipboard.writeText(status.server_error);
      setErrorCopied(true);
      setTimeout(() => setErrorCopied(false), 1500);
    }
  };

  const getStatusColor = (): string => {
    if (!status || !status.server_running) {
      return "text-gray-400";
    }
    switch (status.status) {
      case "online":
        return "text-green-500";
      case "offline":
        return "text-red-500";
      default:
        return "text-yellow-500";
    }
  };

  const getStatusIcon = () => {
    if (!status || !status.server_running) {
      return <Server className="w-5 h-5 text-gray-400" />;
    }
    if (status.status === "online") {
      return <Wifi className={`w-5 h-5 ${getStatusColor()}`} />;
    }
    return <WifiOff className={`w-5 h-5 ${getStatusColor()}`} />;
  };

  const getStatusText = (): string => {
    if (!status) {
      return t("settings.browserConnector.status.loading");
    }
    if (!status.server_running) {
      return t("settings.browserConnector.status.serverStopped");
    }
    switch (status.status) {
      case "online":
        return t("settings.browserConnector.status.online");
      case "offline":
        return t("settings.browserConnector.status.offline");
      default:
        return t("settings.browserConnector.status.waiting");
    }
  };

  const getStatusBadgeClass = (): string => {
    if (!status || !status.server_running) {
      return "bg-gray-500/20 border-gray-500/30";
    }
    switch (status.status) {
      case "online":
        return "bg-green-500/20 border-green-500/30";
      case "offline":
        return "bg-red-500/20 border-red-500/30";
      default:
        return "bg-yellow-500/20 border-yellow-500/30";
    }
  };

  return (
    <SettingContainer
      title={t("settings.browserConnector.status.title")}
      description={t("settings.browserConnector.status.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
      layout="stacked"
    >
      <div className="flex flex-col gap-3">
        <div className="flex items-center gap-3">
          <div
            className={`flex items-center gap-2 px-3 py-1.5 rounded-lg border ${getStatusBadgeClass()}`}
          >
            {getStatusIcon()}
            <span className={`text-sm font-medium ${getStatusColor()}`}>
              {getStatusText()}
            </span>
          </div>

          {/* Show last seen time when offline */}
          {status?.status === "offline" && lastSeenText && (
            <span className="text-xs text-text/50">
              {t("settings.browserConnector.status.lastSeen", { time: lastSeenText })}
            </span>
          )}

          {/* Show port info when server is running */}
          {status?.server_running && (
            <span className="text-xs text-text/40">
              {t("settings.browserConnector.status.port", { port: status.port })}
            </span>
          )}
        </div>

        {/* Show server error if present */}
        {status?.server_error && (
          <div className="flex flex-col gap-2 p-3 rounded-lg border border-red-500/30 bg-red-500/10">
            <div className="flex items-start gap-2">
              <AlertTriangle className="w-4 h-4 text-red-400 mt-0.5 flex-shrink-0" />
              <div className="flex-1">
                <div className="text-sm font-medium text-red-400">
                  {t("settings.browserConnector.status.serverError")}
                </div>
                <div className="text-xs text-red-300/80 mt-1 font-mono break-all select-all">
                  {status.server_error}
                </div>
              </div>
              <button
                onClick={handleCopyError}
                className="p-1.5 rounded hover:bg-red-500/20 transition-colors text-red-400 hover:text-red-300"
                title={t("settings.browserConnector.status.copyError")}
              >
                {errorCopied ? (
                  <Check className="w-4 h-4" />
                ) : (
                  <Copy className="w-4 h-4" />
                )}
              </button>
            </div>
            <div className="text-xs text-text/50 italic">
              {t("settings.browserConnector.status.errorHint")}
            </div>
          </div>
        )}
      </div>
    </SettingContainer>
  );
};
