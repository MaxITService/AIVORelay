import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { relaunch } from "@tauri-apps/plugin-process";
import { AlertTriangle, RefreshCw } from "lucide-react";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

export const NeverLaunchWebView: React.FC = React.memo(() => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const [isRestarting, setIsRestarting] = useState(false);
  const [restartError, setRestartError] = useState<string | null>(null);
  const enabled = getSetting("never_launch_webview") ?? false;

  const changeMode = async (nextEnabled: boolean) => {
    setRestartError(null);
    try {
      await updateSetting("never_launch_webview", nextEnabled, {
        throwOnError: true,
      });
    } catch (error) {
      setRestartError(error instanceof Error ? error.message : String(error));
    }
  };

  const restartWithoutWebView = async () => {
    setIsRestarting(true);
    setRestartError(null);
    try {
      await relaunch();
    } catch (error) {
      setRestartError(error instanceof Error ? error.message : String(error));
      setIsRestarting(false);
    }
  };

  return (
    <div>
      <ToggleSwitch
        checked={enabled}
        onChange={(nextEnabled) => void changeMode(nextEnabled)}
        disabled={isRestarting}
        isUpdating={isUpdating("never_launch_webview")}
        label={t("settings.advanced.neverLaunchWebview.label")}
        description={t("settings.advanced.neverLaunchWebview.description")}
        descriptionMode="tooltip"
        grouped={true}
      />

      {restartError && (
        <p className="mx-6 mb-3 text-xs text-red-300" role="alert">
          {restartError}
        </p>
      )}

      {enabled && (
        <div className="mx-4 mb-4 rounded-lg border border-orange-500/30 bg-orange-500/10 p-3">
          <div className="flex items-start gap-2">
            <AlertTriangle className="mt-0.5 h-4 w-4 flex-shrink-0 text-orange-400" />
            <div className="min-w-0 flex-1 space-y-2 text-xs text-orange-100/80">
              <div>
                <p className="font-semibold text-orange-100">
                  {t(
                    "settings.advanced.neverLaunchWebview.warningTitle",
                  )}
                </p>
                <p>{t("settings.advanced.neverLaunchWebview.warning")}</p>
              </div>
              <p className="text-text/70">
                {t("settings.advanced.neverLaunchWebview.recovery")}
              </p>
              <button
                type="button"
                onClick={restartWithoutWebView}
                disabled={isRestarting || isUpdating("never_launch_webview")}
                className="inline-flex items-center gap-1.5 rounded-lg border border-orange-500/50 bg-orange-500/20 px-3 py-1.5 font-medium text-orange-100 transition-colors hover:bg-orange-500/30 disabled:cursor-not-allowed disabled:opacity-50"
              >
                <RefreshCw
                  className={`h-3.5 w-3.5 ${isRestarting ? "animate-spin" : ""}`}
                />
                {t("settings.advanced.neverLaunchWebview.restart")}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
});
