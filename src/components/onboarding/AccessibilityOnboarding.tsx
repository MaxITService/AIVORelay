import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { type } from "@tauri-apps/plugin-os";
import { invoke } from "@tauri-apps/api/core";
import { Check, Loader2, Mic } from "lucide-react";
import type { WindowsMicrophonePermissionStatus } from "@/lib/types/windowsPermissions";

interface AccessibilityOnboardingProps {
  onComplete: () => void;
}

type PermissionState = "checking" | "needed" | "waiting" | "granted";

const AccessibilityOnboarding: React.FC<AccessibilityOnboardingProps> = ({
  onComplete,
}) => {
  const { t } = useTranslation();
  const [permissionState, setPermissionState] =
    useState<PermissionState>("checking");
  const [error, setError] = useState<string | null>(null);
  const pollingRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isWindows = type() === "windows";

  const finishOnboarding = useCallback(() => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
    }
    timeoutRef.current = setTimeout(() => onComplete(), 300);
  }, [onComplete]);

  const checkPermission = useCallback(async (): Promise<boolean> => {
    try {
      const status = await invoke<WindowsMicrophonePermissionStatus>(
        "get_windows_microphone_permission_status",
      );
      const granted =
        !status.supported || status.overall_access !== "denied";

      setPermissionState(granted ? "granted" : "needed");
      return granted;
    } catch (checkError) {
      console.warn("Failed to check Windows microphone permissions:", checkError);
      setPermissionState("granted");
      return true;
    }
  }, []);

  const stopPolling = useCallback(() => {
    if (pollingRef.current) {
      clearInterval(pollingRef.current);
      pollingRef.current = null;
    }
  }, []);

  const startPolling = useCallback(() => {
    if (pollingRef.current) {
      return;
    }

    pollingRef.current = setInterval(async () => {
      const granted = await checkPermission();
      if (!granted) {
        return;
      }

      stopPolling();
      finishOnboarding();
    }, 1000);
  }, [checkPermission, finishOnboarding, stopPolling]);

  useEffect(() => {
    if (!isWindows) {
      onComplete();
      return;
    }

    checkPermission().then((granted) => {
      if (granted) {
        finishOnboarding();
      }
    });

    return () => {
      stopPolling();
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, [checkPermission, finishOnboarding, isWindows, onComplete, stopPolling]);

  const handleOpenSettings = async () => {
    setError(null);

    try {
      await invoke("open_microphone_privacy_settings");
      setPermissionState("waiting");
      startPolling();
    } catch (openError) {
      console.error("Failed to open Windows microphone privacy settings:", openError);
      setError(
        t(
          "onboarding.permissions.errors.openSettingsFailed",
          "Failed to open Windows microphone privacy settings.",
        ),
      );
    }
  };

  if (permissionState === "checking") {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-10 text-[#a0a0a0]">
        <Loader2 className="h-8 w-8 animate-spin text-[#ff4d8d]" />
        <p className="text-sm">
          {t("onboarding.permissions.checking", "Checking microphone access...")}
        </p>
      </div>
    );
  }

  if (permissionState === "granted") {
    return (
      <div className="flex flex-col items-center justify-center gap-4 py-10 text-center">
        <div className="flex h-16 w-16 items-center justify-center rounded-full bg-emerald-500/20">
          <Check className="h-8 w-8 text-emerald-400" />
        </div>
        <p className="text-base font-medium text-[#f5f5f5]">
          {t("onboarding.permissions.allGranted", "Microphone access is ready.")}
        </p>
      </div>
    );
  }

  return (
    <div className="glass-panel mx-auto flex w-full max-w-[520px] flex-col gap-5 rounded-2xl border border-[#ff4d8d]/20 p-6 text-left">
      <div className="flex items-start gap-4">
        <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-full bg-[#ff4d8d]/15">
          <Mic className="h-7 w-7 text-[#ff4d8d]" />
        </div>
        <div className="space-y-2">
          <h2 className="text-xl font-semibold text-[#f5f5f5]">
            {t(
              "onboarding.permissions.title",
              "Allow microphone access",
            )}
          </h2>
          <p className="text-sm leading-relaxed text-[#b8b8b8]">
            {t(
              "onboarding.permissions.description",
              "AivoRelay needs Windows microphone permission before voice recording can work.",
            )}
          </p>
        </div>
      </div>

      <div className="rounded-xl border border-[#333333] bg-[#1a1a1a]/80 p-4">
        <h3 className="text-sm font-semibold text-[#f5f5f5]">
          {t(
            "onboarding.permissions.microphone.title",
            "Windows microphone privacy",
          )}
        </h3>
        <p className="mt-2 text-sm leading-relaxed text-[#a0a0a0]">
          {t(
            "onboarding.permissions.microphone.description",
            "Open the Windows privacy settings page, allow microphone access, then return here. AivoRelay will detect the change automatically.",
          )}
        </p>
      </div>

      {error && (
        <div className="rounded-xl border border-[#ff453a]/30 bg-[#ff453a]/10 p-3 text-sm text-[#ff7b73]">
          {error}
        </div>
      )}

      {permissionState === "waiting" ? (
        <div className="flex items-center gap-2 text-sm text-[#a0a0a0]">
          <Loader2 className="h-4 w-4 animate-spin text-[#ff4d8d]" />
          {t(
            "onboarding.permissions.waiting",
            "Waiting for Windows microphone access...",
          )}
        </div>
      ) : (
        <button
          className="self-start rounded-xl bg-[#ff4d8d] px-5 py-3 text-sm font-semibold text-white transition-colors hover:bg-[#ff3377]"
          onClick={handleOpenSettings}
          type="button"
        >
          {t("onboarding.permissions.openSettings", "Open Windows settings")}
        </button>
      )}
    </div>
  );
};

export default AccessibilityOnboarding;
