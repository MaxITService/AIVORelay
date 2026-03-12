import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Wifi, WifiOff, Server, AlertTriangle, Copy, Check } from "lucide-react";
import type { ConnectorStatus } from "@/bindings";
import { SettingContainer } from "../../ui/SettingContainer";

type DisplayState = "loading" | "disabled" | "starting" | "unavailable" | "waiting" | "online" | "offline";

interface ConnectorStatusIndicatorProps {
  grouped?: boolean;
  descriptionMode?: "inline" | "tooltip" | "none";
  connectorEnabled: boolean;
  status: ConnectorStatus | null;
  restartNotice?: string | null;
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
  connectorEnabled,
  status,
  restartNotice = null,
}) => {
  const { t } = useTranslation();
  const [lastSeenText, setLastSeenText] = useState<string>("");
  const [errorCopied, setErrorCopied] = useState(false);

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

  // Copy error to clipboard
  const handleCopyError = () => {
    if (status?.server_error) {
      void navigator.clipboard.writeText(status.server_error);
      setErrorCopied(true);
      setTimeout(() => setErrorCopied(false), 1500);
    }
  };

  const getDisplayState = (): DisplayState => {
    if (!connectorEnabled) {
      return "disabled";
    }
    if (!status) {
      return "loading";
    }
    if (!status.server_running) {
      return status.server_error ? "unavailable" : "starting";
    }
    if (status.status === "online") {
      return "online";
    }
    if (status.status === "offline") {
      return "offline";
    }
    return "waiting";
  };

  const displayState = getDisplayState();

  const getStatusColor = (): string => {
    switch (displayState) {
      case "online":
        return "text-green-500";
      case "offline":
      case "unavailable":
        return "text-red-500";
      case "waiting":
      case "starting":
        return "text-yellow-500";
      default:
        return "text-gray-400";
    }
  };

  const getStatusIcon = () => {
    if (displayState === "online") {
      return <Wifi className={`w-5 h-5 ${getStatusColor()}`} />;
    }
    if (displayState === "offline") {
      return <WifiOff className={`w-5 h-5 ${getStatusColor()}`} />;
    }
    return <Server className={`w-5 h-5 ${getStatusColor()}`} />;
  };

  const getStatusText = (): string => {
    switch (displayState) {
      case "loading":
        return t("settings.browserConnector.status.loading");
      case "disabled":
        return t("settings.browserConnector.status.serverStopped");
      case "starting":
        return t("settings.browserConnector.status.applyingSettings");
      case "unavailable":
        return t("settings.browserConnector.status.serverUnavailable");
      case "online":
        return t("settings.browserConnector.status.online");
      case "offline":
        return t("settings.browserConnector.status.offline");
      default:
        return t("settings.browserConnector.status.waitingForExtension");
    }
  };

  const getStatusBadgeClass = (): string => {
    switch (displayState) {
      case "online":
        return "bg-green-500/20 border-green-500/30";
      case "offline":
      case "unavailable":
        return "bg-red-500/20 border-red-500/30";
      case "waiting":
      case "starting":
        return "bg-yellow-500/20 border-yellow-500/30";
      default:
        return "bg-gray-500/20 border-gray-500/30";
    }
  };

  return (
    <SettingContainer
      title={t("settings.browserConnector.status.title")}
      description={t("settings.browserConnector.status.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
      layout="horizontal"
    >
      <div className="flex flex-col gap-2.5">
        <div className="flex items-center gap-2">
          <div
            className={`flex items-center gap-1.5 px-2 py-1 rounded border text-xs ${getStatusBadgeClass()}`}
            title={t("settings.browserConnector.status.activityHintTooltip")}
          >
            {React.cloneElement(getStatusIcon(), { className: `w-3.5 h-3.5 ${getStatusColor()}` })}
            <span className={`font-medium ${getStatusColor()}`}>
              {getStatusText()}
            </span>
          </div>

          {status?.server_running && (
            <span className="text-xs text-text/40">
              {t("settings.browserConnector.status.port", { port: status.port })}
            </span>
          )}

          {displayState === "offline" && lastSeenText && (
            <span className="text-xs text-text/50">
              {t("settings.browserConnector.status.lastSeen", { time: lastSeenText })}
            </span>
          )}
        </div>

        {connectorEnabled && (
          <div
            className="text-xs text-text/50 underline decoration-dotted underline-offset-2"
            title={t("settings.browserConnector.status.activityHintTooltip")}
          >
            {t("settings.browserConnector.status.activityHintLabel")}
          </div>
        )}

        {restartNotice && (
          <div className="rounded border border-yellow-500/30 bg-yellow-500/10 px-3 py-2 text-xs text-yellow-100">
            {restartNotice}
          </div>
        )}

        {connectorEnabled && status?.server_error && (
          <div className="flex flex-col gap-1.5 p-2 rounded border border-red-500/30 bg-red-500/10">
            <div className="flex items-start gap-1.5">
              <AlertTriangle className="w-3.5 h-3.5 text-red-400 mt-0.5 flex-shrink-0" />
              <div className="flex-1 min-w-0">
                <div className="text-xs font-medium text-red-400">
                  {t("settings.browserConnector.status.serverError")}
                </div>
                <div className="text-xs text-red-300/80 mt-0.5 font-mono break-all select-all">
                  {status.server_error}
                </div>
              </div>
              <button
                onClick={handleCopyError}
                className="p-1 rounded hover:bg-red-500/20 transition-colors text-red-400 hover:text-red-300"
                title={t("settings.browserConnector.status.copyError")}
              >
                {errorCopied ? (
                  <Check className="w-3.5 h-3.5" />
                ) : (
                  <Copy className="w-3.5 h-3.5" />
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
