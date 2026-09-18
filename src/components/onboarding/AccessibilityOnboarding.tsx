import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Check, Loader2, Mic } from "lucide-react";
import { useWindowsMicrophonePermission } from "@/hooks/useWindowsMicrophonePermission";

interface AccessibilityOnboardingProps {
  onComplete: () => void;
}

const GRANTED_DISMISS_DELAY_MS = 300;

const AccessibilityOnboarding: React.FC<AccessibilityOnboardingProps> = ({
  onComplete,
}) => {
  const { t } = useTranslation();
  const { permissionState, openError, openSettings } =
    useWindowsMicrophonePermission();
  const completedRef = useRef(false);

  // Leave the step on its own once access is granted; the microphone is not
  // required for the rest of the app, so the user may also continue without it.
  useEffect(() => {
    if (permissionState !== "granted" || completedRef.current) {
      return;
    }

    completedRef.current = true;
    const timeout = setTimeout(onComplete, GRANTED_DISMISS_DELAY_MS);
    return () => clearTimeout(timeout);
  }, [onComplete, permissionState]);

  const handleContinueWithout = () => {
    if (completedRef.current) {
      return;
    }
    completedRef.current = true;
    onComplete();
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

      {openError && (
        <div className="rounded-xl border border-[#ff453a]/30 bg-[#ff453a]/10 p-3 text-sm text-[#ff7b73]">
          {t(
            "onboarding.permissions.errors.openSettingsFailed",
            "Failed to open Windows microphone privacy settings.",
          )}
        </div>
      )}

      <div className="flex flex-wrap items-center gap-x-4 gap-y-3">
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
            className="rounded-xl bg-[#ff4d8d] px-5 py-3 text-sm font-semibold text-white transition-colors hover:bg-[#ff3377]"
            onClick={() => void openSettings()}
            type="button"
          >
            {t("onboarding.permissions.openSettings", "Open Windows settings")}
          </button>
        )}
        <button
          className="text-sm font-medium text-[#a0a0a0] underline-offset-4 transition-colors hover:text-[#f5f5f5] hover:underline"
          onClick={handleContinueWithout}
          type="button"
        >
          {t(
            "onboarding.permissions.continueWithout",
            "Continue without microphone",
          )}
        </button>
      </div>
      <p className="text-xs leading-relaxed text-[#7a7a7a]">
        {t(
          "onboarding.permissions.continueWithoutHint",
          "You can allow access later. AivoRelay shows a reminder in its settings until Windows grants it.",
        )}
      </p>
    </div>
  );
};

export default AccessibilityOnboarding;
