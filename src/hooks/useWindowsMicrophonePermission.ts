import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { type } from "@tauri-apps/plugin-os";
import { commands } from "@/bindings";

export type WindowsMicrophonePermissionState =
  | "checking"
  | "granted"
  | "needed"
  | "waiting";

interface RecordingErrorPayload {
  error_type: string;
  detail: string;
}

const POLL_INTERVAL_MS = 1000;

/**
 * Tracks the Windows microphone privacy permission for the app.
 *
 * The registry-backed status is re-read on mount, when the window regains
 * focus (the user returns from Windows Settings), once a second while the
 * user is expected to be flipping the toggle, and whenever a recording fails
 * with a permission error. On non-Windows platforms the permission is always
 * reported as granted.
 */
export const useWindowsMicrophonePermission = () => {
  const isWindows = type() === "windows";
  const [permissionState, setPermissionState] =
    useState<WindowsMicrophonePermissionState>(
      isWindows ? "checking" : "granted",
    );
  const [openError, setOpenError] = useState<string | null>(null);
  const pollingRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const stopPolling = useCallback(() => {
    if (pollingRef.current) {
      clearInterval(pollingRef.current);
      pollingRef.current = null;
    }
  }, []);

  const checkPermission = useCallback(async (): Promise<boolean> => {
    if (!isWindows) {
      return true;
    }

    try {
      const status = await commands.getWindowsMicrophonePermissionStatus();
      const granted = !status.supported || status.overall_access !== "denied";

      if (granted) {
        stopPolling();
      }
      setPermissionState((current) =>
        granted ? "granted" : current === "waiting" ? "waiting" : "needed",
      );
      return granted;
    } catch (checkError) {
      console.warn(
        "Failed to check Windows microphone permissions:",
        checkError,
      );
      stopPolling();
      setPermissionState("granted");
      return true;
    }
  }, [isWindows, stopPolling]);

  const startPolling = useCallback(() => {
    if (pollingRef.current) {
      return;
    }

    pollingRef.current = setInterval(() => {
      void checkPermission();
    }, POLL_INTERVAL_MS);
  }, [checkPermission]);

  const openSettings = useCallback(async () => {
    setOpenError(null);

    const result = await commands.openMicrophonePrivacySettings();
    if (result.status === "error") {
      console.error(
        "Failed to open Windows microphone privacy settings:",
        result.error,
      );
      setOpenError(result.error);
      return;
    }

    setPermissionState("waiting");
    startPolling();
  }, [startPolling]);

  useEffect(() => {
    if (!isWindows) {
      return;
    }

    void checkPermission();

    const handleFocus = () => {
      void checkPermission();
    };
    window.addEventListener("focus", handleFocus);

    const unlisten = listen<RecordingErrorPayload>(
      "recording-error",
      (event) => {
        if (event.payload.error_type === "microphone_permission_denied") {
          void checkPermission();
        }
      },
    );

    return () => {
      window.removeEventListener("focus", handleFocus);
      unlisten.then((dispose) => dispose());
      stopPolling();
    };
  }, [checkPermission, isWindows, stopPolling]);

  return {
    permissionState,
    isGranted: permissionState === "granted",
    openError,
    openSettings,
    refresh: checkPermission,
  };
};
