import React, { useState, useEffect } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { commands } from "@/bindings";

import ModelSelector from "../model-selector";
import UpdateChecker from "../update-checker";
import VramMeter from "./VramMeter";
import FooterCpuWarning from "./FooterCpuWarning";
import { useSettings } from "../../hooks/useSettings";
import { sessionToast as toast } from "../../lib/sessionToast";

const buildLabel = import.meta.env.VITE_BUILD_LABEL?.trim();

const Footer: React.FC = () => {
  const { t } = useTranslation();
  const { updateSetting, isUpdating } = useSettings();
  const [version, setVersion] = useState("");
  const [vramRefreshNonce, setVramRefreshNonce] = useState(0);
  const [isRestartingLowMemory, setIsRestartingLowMemory] = useState(false);
  const [showLowMemoryRecovery, setShowLowMemoryRecovery] = useState(false);

  useEffect(() => {
    const fetchVersion = async () => {
      try {
        const appVersion = await getVersion();
        setVersion(appVersion);
      } catch (error) {
        console.error("Failed to get app version:", error);
        setVersion("0.1.2");
      }
    };

    fetchVersion();
  }, []);

  useEffect(() => {
    void commands
      .consumeSpeechOnlyRecoveryNotice()
      .then((result) => {
        if (result.status !== "ok") {
          console.warn(
            "Failed to read speech-only recovery notice:",
            result.error,
          );
          return;
        }
        if (!result.data) return;
        setShowLowMemoryRecovery(true);
        toast.info(
          t("settings.advanced.neverLaunchWebview.restoredToastTitle"),
          {
            description: t(
              "settings.advanced.neverLaunchWebview.restoredToastDescription",
            ),
            duration: 12_000,
          },
        );
      })
      .catch((error) => {
        console.warn("Failed to read speech-only recovery notice:", error);
      });
  }, [t]);

  const returnToLowMemoryMode = async () => {
    setIsRestartingLowMemory(true);
    try {
      await updateSetting("never_launch_webview", true, {
        throwOnError: true,
      });
      await relaunch();
    } catch (error) {
      setIsRestartingLowMemory(false);
      toast.error(
        t("settings.advanced.neverLaunchWebview.restartError"),
        {
          description: error instanceof Error ? error.message : String(error),
        },
      );
    }
  };

  return (
    <div className="w-full shrink-0 bg-[#0f0f0f] border-t border-[#282828] pt-3">
      <div className="flex justify-between items-center text-xs px-4 pb-3 text-[#b8b8b8]">
        <div className="flex items-center gap-4">
          <ModelSelector
            onInteraction={() => setVramRefreshNonce((prev) => prev + 1)}
          />
          <VramMeter refreshNonce={vramRefreshNonce} />
          <FooterCpuWarning />
          {buildLabel && (
            <span className="shrink-0 rounded border border-amber-500/30 bg-amber-500/10 px-2 py-0.5 text-[10px] font-medium text-amber-300">
              {buildLabel}
            </span>
          )}
          {showLowMemoryRecovery && (
            <button
              type="button"
              onClick={() => void returnToLowMemoryMode()}
              disabled={
                isRestartingLowMemory || isUpdating("never_launch_webview")
              }
              title={t(
                "settings.advanced.neverLaunchWebview.footerReturnDescription",
              )}
              className="inline-flex shrink-0 items-center gap-1.5 rounded border border-[#ff4d8d]/35 bg-[#ff4d8d]/10 px-2 py-1 text-[10px] font-semibold text-[#ff8ebb] transition-colors hover:bg-[#ff4d8d]/20 disabled:cursor-not-allowed disabled:opacity-50"
            >
              <RefreshCw
                className={`h-3 w-3 ${isRestartingLowMemory ? "animate-spin" : ""}`}
                aria-hidden="true"
              />
              {t("settings.advanced.neverLaunchWebview.footerReturn")}
            </button>
          )}
        </div>

        {/* Update Status */}
        <div className="flex shrink-0 items-center gap-2">
          <UpdateChecker />
          <span className="text-[#333333]">•</span>
          {/* eslint-disable-next-line i18next/no-literal-string */}
          <span className="font-medium">v{version}</span>
        </div>
      </div>
    </div>
  );
};

export default Footer;
