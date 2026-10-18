import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { platform } from "@tauri-apps/plugin-os";
import {
  checkAccessibilityPermission,
  requestAccessibilityPermission,
} from "tauri-plugin-macos-permissions-api";
import { commands } from "@/bindings";
import { initializeInputSystem } from "@/lib/initializeInput";

// The backend creates the input system and shortcuts on macOS only once the
// Accessibility permission exists; both commands are idempotent.
const initializeInput = async (): Promise<void> => {
  await initializeInputSystem(commands);
};

// Define permission state type
type PermissionState = "request" | "verify" | "granted";

// Define button configuration type
interface ButtonConfig {
  text: string;
  className: string;
}

const AccessibilityPermissions: React.FC = () => {
  const { t } = useTranslation();
  const [hasAccessibility, setHasAccessibility] = useState<boolean>(false);
  const [permissionState, setPermissionState] =
    useState<PermissionState>("request");
  const [isMacOS, setIsMacOS] = useState<boolean | null>(null);
  const [inputReady, setInputReady] = useState(false);
  const [inputError, setInputError] = useState<string | null>(null);
  const checking = useRef(false);

  // Check permissions without requesting (with error handling)
  const checkPermissions = async (initial = false): Promise<boolean> => {
    if (checking.current) return false;
    checking.current = true;
    let granted = false;
    try {
      const hasPermissions: boolean = await checkAccessibilityPermission();
      granted = hasPermissions;
      setHasAccessibility(hasPermissions);
      setPermissionState(hasPermissions ? "granted" : initial ? "request" : "verify");
      if (hasPermissions) {
        await initializeInput();
        setInputReady(true);
        setInputError(null);
      }
      return hasPermissions;
    } catch (error) {
      console.error("Error checking accessibility permissions:", error);
      if (granted) {
        setInputError(error instanceof Error ? error.message : String(error));
      } else {
        setPermissionState(initial ? "request" : "verify");
      }
      return false;
    } finally {
      checking.current = false;
    }
  };

  // Handle the unified button action based on current state
  const handleButtonClick = async (): Promise<void> => {
    if (permissionState === "request") {
      try {
        await requestAccessibilityPermission();
        // After system prompt, transition to verification state
        setPermissionState("verify");
      } catch (error) {
        console.error("Error requesting permissions:", error);
        setPermissionState("verify");
      }
    } else {
      // State is "verify" - check if permission was granted
      await checkPermissions();
    }
  };

  // On app boot - check platform and permissions
  useEffect(() => {
    const initialSetup = async (): Promise<void> => {
      // Check if running on macOS first
      const currentPlatform = platform();
      const isMac = currentPlatform === "macos";
      setIsMacOS(isMac);

      // Only check permissions on macOS
      if (!isMac) {
        return;
      }

      await checkPermissions(true);
    };

    initialSetup();
  }, []);

  // The user grants the permission in System Settings, outside this window. Poll
  // quietly (never prompting) so input and shortcuts start without another click.
  useEffect(() => {
    if (isMacOS !== true || inputReady) return;
    const interval = window.setInterval(() => {
      void checkPermissions();
    }, 2000);
    return () => window.clearInterval(interval);
  }, [isMacOS, inputReady]);

  // Don't render on non-macOS platforms or while checking platform
  if (isMacOS === null || !isMacOS) {
    return null;
  }

  // Don't render if permissions granted (also prevents null config issue)
  if (hasAccessibility && inputReady) {
    return null;
  }

  // Configure button text and style based on state
  const buttonConfig: Record<"request" | "verify", ButtonConfig> = {
    request: {
      text: t("accessibility.openSettings"),
      className:
        "px-4 py-2 text-sm font-medium bg-[#9b5de5] text-white rounded-md hover:shadow-[0_4px_16px_rgba(155,93,229,0.35)] hover:-translate-y-0.5 transition-all duration-200 cursor-pointer",
    },
    verify: {
      text: t("accessibility.openSettings"),
      className:
        "bg-[#1a1a1a] hover:bg-[#222222] text-[#f5f5f5] font-medium py-2 px-4 rounded-md text-sm flex items-center justify-center cursor-pointer border border-[#333333] transition-all duration-200",
    },
  };

  const config = buttonConfig[permissionState === "granted" ? "verify" : permissionState];

  return (
    <div className="p-5 w-full rounded-xl glass-panel border border-[#ff6b9d]/30">
      <div className="flex justify-between items-center gap-4">
        <div>
          <p className="text-sm font-medium text-[#e8e8e8]">
            {hasAccessibility ? inputError ?? t("common.loading") : t("accessibility.permissionsDescription")}
          </p>
        </div>
        <button
          onClick={handleButtonClick}
          className={`min-h-10 shrink-0 ${config.className}`}
        >
          {hasAccessibility ? t("common.retry") : config.text}
        </button>
      </div>
    </div>
  );
};

export default AccessibilityPermissions;
