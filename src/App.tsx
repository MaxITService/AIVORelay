import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { sessionToast as toast } from "@/lib/sessionToast";
import "./App.css";
import AccessibilityPermissions from "./components/AccessibilityPermissions";
import Footer from "./components/footer";
import Onboarding from "./components/onboarding";
import { Sidebar, SidebarSection, SECTIONS_CONFIG } from "./components/Sidebar";
import { HotkeySidebar } from "./components/hotkey-sidebar";
import { useSettings } from "./hooks/useSettings";
import { commands } from "@/bindings";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { type } from "@tauri-apps/plugin-os";
import { useNavigationStore } from "./stores/navigationStore";
import { OPEN_FIRST_START_WIZARD_EVENT } from "./constants/appEvents";
import type { ModelStateEvent } from "./lib/types/events";
import type { WindowsMicrophonePermissionStatus } from "./lib/types/windowsPermissions";

type RecordingErrorPayload = {
  error_type: "microphone_permission_denied" | "no_input_device" | "unknown";
  detail: string;
};

type RemoteSttErrorPayload =
  | string
  | {
      message?: string;
      retryAction?: {
        command?: string;
        label?: string;
      };
    };

type ModelDownloadProgressPayload = {
  model_id: string;
};

const renderSettingsContent = (section: SidebarSection) => {
  const ActiveComponent =
    SECTIONS_CONFIG[section]?.component || SECTIONS_CONFIG.general.component;
  return <ActiveComponent />;
};

function App() {
  const { t } = useTranslation();
  const [showOnboarding, setShowOnboarding] = useState<boolean | null>(null);
  const [onboardingFromDebug, setOnboardingFromDebug] = useState(false);
  const [onboardingStartsWithPermissions, setOnboardingStartsWithPermissions] =
    useState(false);
  const [onboardingPermissionOnly, setOnboardingPermissionOnly] =
    useState(false);
  const { currentSection, setSection: setCurrentSection } =
    useNavigationStore();
  const { refreshSettings, refreshAudioDevices } = useSettings();
  const notifiedModelDownloadStarts = useRef(new Set<string>());

  useEffect(() => {
    checkOnboardingStatus();
  }, []);

  useEffect(() => {
    const handleOpenFirstStartWizard = () => {
      setOnboardingFromDebug(true);
      setOnboardingStartsWithPermissions(false);
      setOnboardingPermissionOnly(false);
      setShowOnboarding(true);
    };

    window.addEventListener(
      OPEN_FIRST_START_WIZARD_EVENT,
      handleOpenFirstStartWizard,
    );

    return () => {
      window.removeEventListener(
        OPEN_FIRST_START_WIZARD_EVENT,
        handleOpenFirstStartWizard,
      );
    };
  }, []);

  useEffect(() => {
    const ERROR_TOAST_DURATION_MS = 8000;

    const unlistenRemote = listen<RemoteSttErrorPayload>(
      "remote-stt-error",
      (event) => {
        const message =
          typeof event.payload === "string"
            ? event.payload
            : event.payload.message ||
              t("overlay.errors.unknown.title", "Transcription failed");
        const retryAction =
          typeof event.payload === "object" && event.payload?.retryAction
            ? event.payload.retryAction
            : null;

        toast.error(message, {
          duration: ERROR_TOAST_DURATION_MS,
          action: retryAction
            ? {
                label: t("common.retry", "Retry"),
                onClick: () => {
                  void invoke(
                    retryAction.command || "retry_last_remote_transcription",
                  );
                },
              }
            : undefined,
        });
      },
    );
    const unlistenScreenshot = listen<string>("screenshot-error", (event) => {
      toast.error(event.payload, { duration: ERROR_TOAST_DURATION_MS });
    });
    const unlistenVoiceCommand = listen<string>(
      "voice-command-error",
      (event) => {
        toast.error(event.payload, { duration: ERROR_TOAST_DURATION_MS });
      },
    );
    const unlistenRecording = listen<RecordingErrorPayload>(
      "recording-error",
      (event) => {
        const { error_type, detail } = event.payload;

        if (error_type === "no_input_device") {
          toast.error(t("errors.noInputDeviceTitle"), {
            duration: ERROR_TOAST_DURATION_MS,
            description: t("errors.noInputDevice"),
          });
          return;
        }

        toast.error(detail, { duration: ERROR_TOAST_DURATION_MS });
      },
    );
    const unlistenPaste = listen("paste-error", () => {
      toast.error(t("errors.pasteFailedTitle"), {
        duration: ERROR_TOAST_DURATION_MS,
        description: t("errors.pasteFailed"),
      });
    });
    const unlistenModelState = listen<ModelStateEvent>(
      "model-state-changed",
      (event) => {
        if (event.payload.event_type !== "loading_failed") {
          return;
        }

        toast.error(
          t("errors.modelLoadFailed", {
            model:
              event.payload.model_name || t("errors.modelLoadFailedUnknown"),
          }),
          {
            duration: ERROR_TOAST_DURATION_MS,
            description: event.payload.error,
          },
        );
      },
    );
    const unlistenModelDownloadFailed = listen<{
      model_id: string;
      error: string;
    }>("model-download-failed", (event) => {
      notifiedModelDownloadStarts.current.delete(event.payload.model_id);
      toast.error(
        t("errors.modelDownloadFailed", {
          model:
            event.payload.model_id || t("errors.modelDownloadFailedUnknown"),
        }),
        {
          duration: ERROR_TOAST_DURATION_MS,
          description: event.payload.error,
        },
      );
    });
    const unlistenModelDownloadProgress = listen<ModelDownloadProgressPayload>(
      "model-download-progress",
      (event) => {
        const modelId = event.payload.model_id;
        if (!modelId || notifiedModelDownloadStarts.current.has(modelId)) {
          return;
        }

        notifiedModelDownloadStarts.current.add(modelId);
        toast(t("modelSelector.downloadActivationNoticeTitle"), {
          duration: 6000,
          description: t("modelSelector.downloadActivationNoticeDescription"),
        });
      },
    );
    const unlistenModelDownloadCancelled = listen<string>(
      "model-download-cancelled",
      (event) => {
        notifiedModelDownloadStarts.current.delete(event.payload);
      },
    );

    const unlistenAuthFailed = listen<{ message: string }>(
      "connector-auth-failed",
      (event) => {
        toast.warning(
          event.payload.message || "Connector authentication failed",
          { duration: 5000 },
        );
      },
    );

    return () => {
      unlistenRemote.then((unlisten) => unlisten());
      unlistenScreenshot.then((unlisten) => unlisten());
      unlistenVoiceCommand.then((unlisten) => unlisten());
      unlistenRecording.then((unlisten) => unlisten());
      unlistenPaste.then((unlisten) => unlisten());
      unlistenModelState.then((unlisten) => unlisten());
      unlistenModelDownloadFailed.then((unlisten) => unlisten());
      unlistenModelDownloadProgress.then((unlisten) => unlisten());
      unlistenModelDownloadCancelled.then((unlisten) => unlisten());
      unlistenAuthFailed.then((unlisten) => unlisten());
    };
  }, [t]);

  // Sync soniox_live_preview_enabled when the active profile changes (e.g. via shortcut)
  useEffect(() => {
    const unlisten = listen("active-profile-changed", async () => {
      try {
        const result = await commands.getAppSettings();
        if (result.status === "ok") {
          const s = result.data as any;
          const id = s?.active_profile_id ?? "default";
          const previewEnabled =
            id === "default"
              ? Boolean(s?.preview_output_only_enabled ?? false)
              : Boolean(
                  (s?.transcription_profiles ?? []).find(
                    (p: any) => p.id === id,
                  )?.preview_output_only_enabled ?? false,
                );
          await commands.changeSonioxLivePreviewEnabledSetting(previewEnabled);
        }
      } catch (e) {
        console.error("Failed to sync preview setting after profile change", e);
      }
      refreshSettings();
    });
    return () => {
      unlisten.then((u) => u());
    };
  }, [refreshSettings]);

  useEffect(() => {
    const unlisten = listen("audio-input-state-changed", async () => {
      await Promise.all([refreshSettings(), refreshAudioDevices()]);
    });

    return () => {
      unlisten.then((u) => u());
    };
  }, [refreshAudioDevices, refreshSettings]);

  const checkOnboardingStatus = async () => {
    try {
      const currentPlatform = type();
      const [settingsResult, modelResult] = await Promise.all([
        commands.getAppSettings(),
        commands.hasAnyModelsAvailable(),
      ]);
      let windowsMicPermissionDenied = false;

      if (currentPlatform === "windows") {
        try {
          const permissionStatus =
            await invoke<WindowsMicrophonePermissionStatus>(
              "get_windows_microphone_permission_status",
            );
          windowsMicPermissionDenied =
            permissionStatus.supported &&
            permissionStatus.overall_access === "denied";
        } catch (permissionError) {
          console.warn(
            "Failed to check Windows microphone permissions:",
            permissionError,
          );
        }
      }

      if (settingsResult.status === "ok") {
        const provider = String(settingsResult.data.transcription_provider);
        if (
          provider === "remote_openai_compatible" ||
          provider === "remote_soniox" ||
          provider === "remote_deepgram"
        ) {
          if (windowsMicPermissionDenied) {
            setOnboardingStartsWithPermissions(true);
            setOnboardingPermissionOnly(true);
            setShowOnboarding(true);
            return;
          }
          setOnboardingStartsWithPermissions(false);
          setOnboardingPermissionOnly(false);
          setShowOnboarding(false);
          return;
        }
      }

      if (modelResult.status === "ok") {
        if (modelResult.data && windowsMicPermissionDenied) {
          setOnboardingStartsWithPermissions(true);
          setOnboardingPermissionOnly(true);
          setShowOnboarding(true);
          return;
        }

        setOnboardingStartsWithPermissions(windowsMicPermissionDenied);
        setOnboardingPermissionOnly(false);
        setShowOnboarding(!modelResult.data);
      } else {
        setOnboardingStartsWithPermissions(windowsMicPermissionDenied);
        setOnboardingPermissionOnly(false);
        setShowOnboarding(true);
      }
    } catch (error) {
      console.error("Failed to check onboarding status:", error);
      setOnboardingStartsWithPermissions(false);
      setOnboardingPermissionOnly(false);
      setShowOnboarding(true);
    }
  };

  const handleModelSelected = () => {
    // Transition to main app - user has started a download
    setOnboardingFromDebug(false);
    setOnboardingStartsWithPermissions(false);
    setOnboardingPermissionOnly(false);
    setShowOnboarding(false);
  };

  const handleRemoteSelected = () => {
    setOnboardingFromDebug(false);
    setOnboardingStartsWithPermissions(false);
    setOnboardingPermissionOnly(false);
    setShowOnboarding(false);
    setCurrentSection("general");
    refreshSettings();
  };

  const handlePermissionResolved = () => {
    setOnboardingStartsWithPermissions(false);
    if (onboardingPermissionOnly) {
      setOnboardingPermissionOnly(false);
      setShowOnboarding(false);
    }
  };

  if (showOnboarding) {
    return (
      <Onboarding
        onModelSelected={handleModelSelected}
        onRemoteSelected={handleRemoteSelected}
        showFullCatalog={onboardingFromDebug}
        startWithPermissionStep={onboardingStartsWithPermissions}
        permissionOnly={onboardingPermissionOnly}
        onPermissionResolved={handlePermissionResolved}
      />
    );
  }

  return (
    <div className="h-screen flex flex-col bg-[#121212]">
      {/* Main content area that takes remaining space */}
      <div className="flex-1 flex overflow-hidden">
        <Sidebar
          activeSection={currentSection}
          onSectionChange={setCurrentSection}
        />
        {/* Scrollable content area with gradient background */}
        <div className="flex-1 flex flex-col overflow-hidden bg-gradient-to-br from-[#121212] via-[#161616] to-[#0f0f0f]">
          <div className="flex-1 overflow-y-auto">
            <div className="flex flex-col items-center p-6 gap-5 max-w-3xl mx-auto min-h-full">
              <AccessibilityPermissions />
              {renderSettingsContent(currentSection)}
            </div>
          </div>
        </div>
      </div>
      {/* Fixed footer at bottom */}
      <Footer />
      {/* Hotkey sidebar on the right edge */}
      <HotkeySidebar />
    </div>
  );
}

export default App;
