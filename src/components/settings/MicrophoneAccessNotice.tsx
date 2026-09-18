import React from "react";
import { useTranslation } from "react-i18next";
import { Loader2, MicOff } from "lucide-react";
import { useWindowsMicrophonePermission } from "@/hooks/useWindowsMicrophonePermission";

/**
 * In-app reminder that Windows privacy settings block microphone access.
 * Renders nothing while access is granted or still being checked.
 */
export const MicrophoneAccessNotice: React.FC = () => {
  const { t } = useTranslation();
  const { permissionState, openError, openSettings } =
    useWindowsMicrophonePermission();

  if (permissionState === "granted" || permissionState === "checking") {
    return null;
  }

  return (
    <div className="flex flex-col gap-3 rounded-lg border border-amber-300/30 bg-amber-300/5 p-4 sm:flex-row sm:items-center sm:justify-between">
      <div className="flex min-w-0 items-start gap-3">
        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-amber-300/15">
          <MicOff className="h-4 w-4 text-amber-200" aria-hidden="true" />
        </div>
        <div className="min-w-0">
          <p className="text-sm font-semibold text-[#f5f5f5]">
            {t("settings.microphoneAccess.title")}
          </p>
          <p className="mt-1 text-xs leading-relaxed text-[#b8b8b8]">
            {t("settings.microphoneAccess.description")}
          </p>
          {openError && (
            <p className="mt-1 text-xs text-[#ff7b73]">
              {t("onboarding.permissions.errors.openSettingsFailed")}
            </p>
          )}
        </div>
      </div>
      {permissionState === "waiting" ? (
        <div className="flex shrink-0 items-center gap-2 text-xs text-[#a0a0a0]">
          <Loader2 className="h-4 w-4 animate-spin text-amber-200" />
          {t("onboarding.permissions.waiting")}
        </div>
      ) : (
        <button
          className="shrink-0 rounded-lg bg-[#ff4d8d] px-4 py-2 text-sm font-semibold text-white transition-colors hover:bg-[#ff3377]"
          onClick={() => void openSettings()}
          type="button"
        >
          {t("onboarding.permissions.openSettings")}
        </button>
      )}
    </div>
  );
};
