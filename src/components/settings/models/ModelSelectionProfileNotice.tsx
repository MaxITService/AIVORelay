import React from "react";
import { useTranslation } from "react-i18next";
import {
  getTranscriptionProfileAnchorId,
  openActiveTranscriptionProfile,
} from "@/lib/transcriptionProfileNavigation";
import { useSettingsStore } from "@/stores/settingsStore";

interface ModelSelectionProfileNoticeProps {
  compact?: boolean;
  onNavigate?: () => void;
}

export const ModelSelectionProfileNotice: React.FC<ModelSelectionProfileNoticeProps> = ({
  compact = false,
  onNavigate,
}) => {
  const { t } = useTranslation();
  const settings = useSettingsStore((state) => state.settings);
  const activeProfileId = settings?.active_profile_id ?? "default";
  const activeProfile = activeProfileId === "default"
    ? undefined
    : settings?.transcription_profiles?.find(
        (profile) => profile.id === activeProfileId,
      );
  const profileId = activeProfile?.id ?? "default";
  const profileName = activeProfile?.name ?? t("settings.transcriptionProfiles.defaultProfile");
  const label = t("modelSelector.modelSelectionProfile.title", { profile: profileName });
  const description = activeProfile
    ? t("modelSelector.modelSelectionProfile.customDescription")
    : t("modelSelector.modelSelectionProfile.defaultDescription");

  const href = `#${getTranscriptionProfileAnchorId(profileId)}`;
  const handleClick = (event: React.MouseEvent) => {
    event.preventDefault();
    onNavigate?.();
    openActiveTranscriptionProfile();
  };

  if (compact) {
    return (
      <a
        href={href}
        onClick={handleClick}
        title={`${label}. ${description}`}
        aria-label={`${label}. ${description}`}
        className="group flex min-w-0 items-center gap-2.5 rounded-lg border border-[#ff4d8d]/25 bg-[#ff4d8d]/[0.07] px-2.5 py-2 transition-colors hover:border-[#ff4d8d]/50 hover:bg-[#ff4d8d]/[0.13] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#ff8ebb]"
      >
        <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-[#ff4d8d]/15 text-[#ff8ebb]">
          <svg className="h-4 w-4" fill="none" stroke="currentColor" strokeWidth={1.75} viewBox="0 0 24 24" aria-hidden="true">
            <path strokeLinecap="round" strokeLinejoin="round" d="M15.75 6a3.75 3.75 0 11-7.5 0 3.75 3.75 0 017.5 0zM4.5 20.118a7.5 7.5 0 0115 0A17.933 17.933 0 0112 21.75c-2.676 0-5.216-.584-7.5-1.632z" />
          </svg>
        </span>
        <span className="min-w-0 flex-1">
          <span className="block text-[10px] font-semibold uppercase leading-3 tracking-wider text-[#8a8a8a]">
            {t("modelSelector.modelSelectionProfile.compactPrefix").replace(/\s*[:：]\s*$/, "")}
          </span>
          <span className="mt-0.5 block truncate text-sm font-semibold leading-5 text-[#f0f0f0] group-hover:text-[#ffc2d8]">
            {profileName}
          </span>
        </span>
        <svg className="h-3.5 w-3.5 shrink-0 text-[#8a8a8a] transition-all group-hover:translate-x-0.5 group-hover:text-[#ff8ebb]" fill="none" stroke="currentColor" strokeWidth={2} viewBox="0 0 24 24" aria-hidden="true">
          <path strokeLinecap="round" strokeLinejoin="round" d="M9 5l7 7-7 7" />
        </svg>
      </a>
    );
  }

  const profileLink = (
    <a
      href={href}
      onClick={handleClick}
      title={`${label}. ${description}`}
      aria-label={`${label}. ${description}`}
      className="inline-block max-w-full min-w-0 truncate align-bottom font-semibold text-[#ff8ebb] underline decoration-[#ff4d8d]/60 underline-offset-2 hover:text-[#ffc2d8] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#ff8ebb]"
    >
      {profileName}
    </a>
  );

  return (
    <div className="rounded-xl border border-[#ff4d8d]/30 bg-[#ff4d8d]/[0.07] px-4 py-3 text-sm text-[#d8d8d8]">
      <p className="flex flex-wrap items-baseline gap-x-1.5">
        <span>{t("modelSelector.modelSelectionProfile.prefix")}</span>
        {profileLink}
      </p>
      <p className="mt-1 text-xs leading-relaxed text-[#a0a0a0]">{description}</p>
    </div>
  );
};
