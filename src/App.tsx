import { useEffect, useState } from "react";
import { Toaster, toast } from "sonner";
import "./App.css";
import AccessibilityPermissions from "./components/AccessibilityPermissions";
import Footer from "./components/footer";
import Onboarding from "./components/onboarding";
import { Sidebar, SidebarSection, SECTIONS_CONFIG } from "./components/Sidebar";
import { useSettings } from "./hooks/useSettings";
import { commands } from "@/bindings";
import { listen } from "@tauri-apps/api/event";

const renderSettingsContent = (section: SidebarSection) => {
  const ActiveComponent =
    SECTIONS_CONFIG[section]?.component || SECTIONS_CONFIG.general.component;
  return <ActiveComponent />;
};

function App() {
  const [showOnboarding, setShowOnboarding] = useState<boolean | null>(null);
  const [currentSection, setCurrentSection] =
    useState<SidebarSection>("general");
  const { settings, updateSetting, refreshSettings } = useSettings();

  useEffect(() => {
    checkOnboardingStatus();
  }, []);

  useEffect(() => {
    const unlistenRemote = listen<string>("remote-stt-error", (event) => {
      toast.error(event.payload);
    });
    const unlistenAiReplace = listen<string>("ai-replace-error", (event) => {
      toast.error(event.payload);
    });
    const unlistenScreenshot = listen<string>("screenshot-error", (event) => {
      toast.error(event.payload, { duration: 5000 });
    });

    return () => {
      unlistenRemote.then((unlisten) => unlisten());
      unlistenAiReplace.then((unlisten) => unlisten());
      unlistenScreenshot.then((unlisten) => unlisten());
    };
  }, []);

  // Handle keyboard shortcuts for debug mode toggle
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      // Check for Ctrl+Shift+D (Windows/Linux) or Cmd+Shift+D (macOS)
      const isDebugShortcut =
        event.shiftKey &&
        event.key.toLowerCase() === "d" &&
        (event.ctrlKey || event.metaKey);

      if (isDebugShortcut) {
        event.preventDefault();
        const currentDebugMode = settings?.debug_mode ?? false;
        updateSetting("debug_mode", !currentDebugMode);
      }
    };

    // Add event listener when component mounts
    document.addEventListener("keydown", handleKeyDown);

    // Cleanup event listener when component unmounts
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [settings?.debug_mode, updateSetting]);

  const checkOnboardingStatus = async () => {
    try {
      const [settingsResult, modelResult] = await Promise.all([
        commands.getAppSettings(),
        commands.hasAnyModelsAvailable(),
      ]);

      if (
        settingsResult.status === "ok" &&
        settingsResult.data.transcription_provider ===
          "remote_openai_compatible"
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
    setShowOnboarding(false);
  };

  const handleRemoteSelected = () => {
    setShowOnboarding(false);
    setCurrentSection("advanced");
    refreshSettings();
  };

  if (showOnboarding) {
    return (
      <Onboarding
        onModelSelected={handleModelSelected}
        onRemoteSelected={handleRemoteSelected}
      />
    );
  }

  return (
    <div className="h-screen flex flex-col bg-[#121212]">
      <Toaster 
        theme="dark"
        toastOptions={{
          style: {
            background: 'rgba(26, 26, 26, 0.98)',
            border: '1px solid #333333',
            color: '#f5f5f5',
            backdropFilter: 'blur(12px)',
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
            <div className="flex flex-col items-center p-6 gap-5 max-w-3xl mx-auto">
              <AccessibilityPermissions />
              {renderSettingsContent(currentSection)}
            </div>
          </div>
        </div>
      </div>
      {/* Fixed footer at bottom */}
      <Footer />
    </div>
  );
}

export default App;
