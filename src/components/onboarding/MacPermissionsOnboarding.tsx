import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  checkAccessibilityPermission,
  checkMicrophonePermission,
  requestAccessibilityPermission,
  requestMicrophonePermission,
} from "tauri-plugin-macos-permissions-api";
import { commands } from "@/bindings";
import { initializeInputSystem } from "@/lib/initializeInput";

interface Props {
  onComplete: () => void;
}

const MacPermissionsOnboarding: React.FC<Props> = ({ onComplete }) => {
  const { t } = useTranslation();
  const [microphone, setMicrophone] = useState<boolean | null>(null);
  const [accessibility, setAccessibility] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  const checking = useRef(false);
  const inputReady = useRef(false);
  const mounted = useRef(false);

  const refresh = useCallback(async () => {
    if (checking.current) return;
    checking.current = true;
    try {
      const [mic, access] = await Promise.all([
        checkMicrophonePermission(),
        checkAccessibilityPermission(),
      ]);
      if (access && !inputReady.current) {
        await initializeInputSystem(commands);
        inputReady.current = true;
      }
      if (mounted.current) {
        setMicrophone(mic);
        setAccessibility(access);
        setError(null);
      }
    } catch (e) {
      if (mounted.current) setError(e instanceof Error ? e.message : String(e));
    } finally {
      checking.current = false;
    }
  }, []);

  useEffect(() => {
    mounted.current = true;
    void refresh();
    const timer = window.setInterval(() => void refresh(), 2000);
    return () => {
      mounted.current = false;
      window.clearInterval(timer);
    };
  }, [refresh]);

  const request = async (permission: "microphone" | "accessibility") => {
    try {
      if (permission === "microphone") {
        if (microphone === false) await requestMicrophonePermission();
      } else {
        await requestAccessibilityPermission();
      }
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const openMicrophoneSettings = async () => {
    const result = await commands.openMicrophonePrivacySettings();
    if (result.status === "error") setError(result.error);
  };

  return (
    <div className="glass-panel mx-auto flex w-full max-w-[520px] flex-col gap-5 rounded-2xl border border-[#ff4d8d]/20 p-6 text-left">
      <h2 className="text-xl font-semibold text-[#f5f5f5]">{t("onboarding.macPermissions.title", "Allow access for dictation")}</h2>
      <p className="text-sm text-[#b8b8b8]">{t("onboarding.macPermissions.description", "Microphone access records your voice. Accessibility access lets AivoRelay use shortcuts and insert text.")}</p>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <span>{t("onboarding.macPermissions.microphone", "Microphone")}: {microphone === null ? t("common.loading") : microphone ? t("onboarding.macPermissions.allowed", "Allowed") : t("onboarding.macPermissions.needed", "Not allowed")}</span>
        {!microphone && <div className="flex gap-2">
          <button type="button" className="rounded-lg border border-[#555] px-3 py-2" onClick={() => void request("microphone")}>{t("onboarding.macPermissions.request", "Request access")}</button>
          <button type="button" className="rounded-lg border border-[#555] px-3 py-2" onClick={() => void openMicrophoneSettings()}>{t("onboarding.macPermissions.openSettings", "Open settings")}</button>
        </div>}
      </div>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <span>{t("onboarding.macPermissions.accessibility", "Accessibility")}: {accessibility === null ? t("common.loading") : accessibility ? t("onboarding.macPermissions.allowed", "Allowed") : t("onboarding.macPermissions.needed", "Not allowed")}</span>
        {!accessibility && <button type="button" className="rounded-lg border border-[#555] px-3 py-2" onClick={() => void request("accessibility")}>{t("onboarding.macPermissions.openSettings", "Open settings")}</button>}
      </div>
      {error && <p role="alert" className="text-sm text-[#ff7b73]">{error}</p>}
      <button type="button" className="self-start rounded-xl bg-[#ff4d8d] px-5 py-3 text-sm font-semibold text-white" onClick={onComplete}>
        {microphone && accessibility ? t("common.continue", "Continue") : t("onboarding.macPermissions.continueWithout", "Continue and set up access later")}
      </button>
    </div>
  );
};

export default MacPermissionsOnboarding;
