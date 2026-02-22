import { useEffect, useState } from "react";
import { Toaster, toast } from "sonner";
import "./App.css";
import AccessibilityPermissions from "./components/AccessibilityPermissions";
import Footer from "./components/footer";
import Onboarding from "./components/onboarding";
import { Sidebar, SidebarSection, SECTIONS_CONFIG } from "./components/Sidebar";
import { HotkeySidebar } from "./components/hotkey-sidebar";
import { useSettings } from "./hooks/useSettings";
import { commands } from "@/bindings";
import { listen } from "@tauri-apps/api/event";
import { useNavigationStore } from "./stores/navigationStore";
import { OPEN_FIRST_START_WIZARD_EVENT } from "./constants/appEvents";

const renderSettingsContent = (section: SidebarSection) => {
  const ActiveComponent =
    SECTIONS_CONFIG[section]?.component || SECTIONS_CONFIG.general.component;
  return <ActiveComponent />;
};

function App() {
  const [showOnboarding, setShowOnboarding] = useState<boolean | null>(null);
  const [onboardingFromDebug, setOnboardingFromDebug] = useState(false);
  const { currentSection, setSection: setCurrentSection } =
    useNavigationStore();
  const { refreshSettings } = useSettings();

  useEffect(() => {
    checkOnboardingStatus();
  }, []);

  useEffect(() => {
    const handleOpenFirstStartWizard = () => {
      setOnboardingFromDebug(true);
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

    const unlistenRemote = listen<string>("remote-stt-error", (event) => {
      toast.error(event.payload, { duration: ERROR_TOAST_DURATION_MS });
    });
    const unlistenScreenshot = listen<string>("screenshot-error", (event) => {
      toast.error(event.payload, { duration: ERROR_TOAST_DURATION_MS });
    });
    const unlistenVoiceCommand = listen<string>(
      "voice-command-error",
      (event) => {
        toast.error(event.payload, { duration: ERROR_TOAST_DURATION_MS });
      },
    );

    return () => {
      unlistenRemote.then((unlisten) => unlisten());
      unlistenScreenshot.then((unlisten) => unlisten());
      unlistenVoiceCommand.then((unlisten) => unlisten());
    };
  }, []);

  const checkOnboardingStatus = async () => {
    try {
      const [settingsResult, modelResult] = await Promise.all([
        commands.getAppSettings(),
        commands.hasAnyModelsAvailable(),
      ]);

      if (
        settingsResult.status === "ok" &&
        (settingsResult.data.transcription_provider ===
          "remote_openai_compatible" ||
          settingsResult.data.transcription_provider === "remote_soniox")
      ) {
        setShowOnboarding(false);
        return;
      }

      if (modelResult.status === "ok") {
        setShowOnboarding(!modelResult.data);
      } else {
        setShowOnboarding(true);
      }
    } catch (error) {
      console.error("Failed to check onboarding status:", error);
      setShowOnboarding(true);
    }
  };

  const handleModelSelected = () => {
    // Transition to main app - user has started a download
    setOnboardingFromDebug(false);
    setShowOnboarding(false);
  };

  const handleRemoteSelected = () => {
    setOnboardingFromDebug(false);
    setShowOnboarding(false);
    setCurrentSection("general");
    refreshSettings();
  };

  if (showOnboarding) {
    return (
      <Onboarding
        onModelSelected={handleModelSelected}
        onRemoteSelected={handleRemoteSelected}
        showFullCatalog={onboardingFromDebug}
      />
    );
  }

  return (
    <div className="h-screen flex flex-col bg-[#121212]">
      <Toaster
        theme="dark"
        toastOptions={{
          style: {
            background: "rgba(26, 26, 26, 0.98)",
            border: "1px solid #333333",
            color: "#f5f5f5",
            backdropFilter: "blur(12px)",
          },
        }}
      />
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
