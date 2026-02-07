import React, { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { commands, type GpuVramStatus } from "@/bindings";

interface ModelStateEvent {
  event_type: string;
}

interface VramMeterProps {
  refreshNonce?: number;
}

const BYTES_PER_GIB = 1024 ** 3;

const formatGiB = (bytes: number): string => {
  const value = bytes / BYTES_PER_GIB;
  return value >= 10 ? value.toFixed(0) : value.toFixed(1);
};

const VramMeter: React.FC<VramMeterProps> = ({ refreshNonce = 0 }) => {
  const { t } = useTranslation();
  const [status, setStatus] = useState<GpuVramStatus | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const refreshTimerRef = useRef<number | null>(null);

  const refreshVram = useCallback(async () => {
    setIsLoading(true);
    try {
      const result = await commands.getActiveGpuVramStatus();
      if (result.status === "ok") {
        setStatus(result.data);
      } else {
        setStatus({
          is_supported: false,
          adapter_name: null,
          used_bytes: 0,
          budget_bytes: 0,
          system_used_bytes: 0,
          system_free_bytes: 0,
          total_vram_bytes: 0,
          updated_at_unix_ms: Date.now(),
          error: `${result.error}`,
        });
      }
    } catch (error) {
      setStatus({
        is_supported: false,
        adapter_name: null,
        used_bytes: 0,
        budget_bytes: 0,
        system_used_bytes: 0,
        system_free_bytes: 0,
        total_vram_bytes: 0,
        updated_at_unix_ms: Date.now(),
        error: `${error}`,
      });
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void refreshVram();
  }, [refreshVram]);

  useEffect(() => {
    if (refreshNonce > 0) {
      void refreshVram();
    }
  }, [refreshNonce, refreshVram]);

  useEffect(() => {
    const unlistenPromise = listen<ModelStateEvent>(
      "model-state-changed",
      (event) => {
        const eventType = event.payload.event_type;
        if (
          eventType === "loading_completed" ||
          eventType === "unloaded" ||
          eventType === "loading_failed"
        ) {
          if (refreshTimerRef.current !== null) {
            window.clearTimeout(refreshTimerRef.current);
          }
          refreshTimerRef.current = window.setTimeout(() => {
            void refreshVram();
          }, 200);
        }
      },
    );

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
      if (refreshTimerRef.current !== null) {
        window.clearTimeout(refreshTimerRef.current);
      }
    };
  }, [refreshVram]);

  const usagePercent =
    status && status.is_supported && status.budget_bytes > 0
      ? Math.round((status.used_bytes / status.budget_bytes) * 100)
      : null;
  const appFreeBytes =
    status && status.is_supported && status.budget_bytes > 0
      ? Math.max(0, status.budget_bytes - status.used_bytes)
      : 0;

  const usageText =
    status && status.is_supported
      ? `${formatGiB(status.used_bytes)}/${formatGiB(status.budget_bytes)} GB`
      : t("footer.vramUnavailable");
  const freeTextCompact =
    status &&
    status.is_supported &&
    status.total_vram_bytes > 0
      ? t("footer.vramSystemFreeCompact", {
          free: formatGiB(status.system_free_bytes),
          total: formatGiB(status.total_vram_bytes),
        })
      : null;

  const displayText =
    isLoading && !status
      ? t("footer.vramLoading")
      : `${t("footer.vramLabel")} ${usageText}${usagePercent !== null ? ` (${usagePercent}%)` : ""}`;

  const lastUpdatedText = status
    ? new Date(status.updated_at_unix_ms).toLocaleTimeString()
    : t("common.loading");
  const tooltipText = status?.is_supported
    ? [
        t("footer.vramTooltipAdapter", {
          adapter: status.adapter_name || t("footer.vramUnknownAdapter"),
        }),
        t("footer.vramTooltipAppUsed", {
          used: formatGiB(status.used_bytes),
        }),
        t("footer.vramTooltipAppBudget", {
          budget: formatGiB(status.budget_bytes),
        }),
        t("footer.vramTooltipAppFree", {
          free: formatGiB(appFreeBytes),
        }),
        t("footer.vramTooltipSystemUsed", {
          used: formatGiB(status.system_used_bytes),
        }),
        t("footer.vramTooltipSystemFree", {
          free: formatGiB(status.system_free_bytes),
          total: formatGiB(status.total_vram_bytes),
        }),
        t("footer.vramClickToRefresh"),
      ].join("\n")
    : [
        t("footer.vramTooltipAdapter", {
          adapter: status?.adapter_name || t("footer.vramUnknownAdapter"),
        }),
        t("footer.vramUnavailable"),
        status?.error || "",
        t("footer.vramClickToRefresh"),
      ]
        .filter(Boolean)
        .join("\n");

  return (
    <button
      onClick={() => void refreshVram()}
      className="flex items-center gap-2 transition-colors hover:text-text"
      title={tooltipText}
    >
      <div
        className={`w-2 h-2 rounded-full ${
          status?.is_supported ? "bg-sky-400" : "bg-mid-gray/60"
        } ${isLoading ? "animate-pulse" : ""}`}
      />
      <span className="tabular-nums">{displayText}</span>
      {freeTextCompact && (
        <span className="text-[10px] text-text/60 tabular-nums">
          {freeTextCompact}
        </span>
      )}
      <span className="text-[10px] text-text/50 tabular-nums">
        {t("footer.vramLastUpdated", { time: lastUpdatedText })}
      </span>
    </button>
  );
};

export default VramMeter;
