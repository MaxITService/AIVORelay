import React, { useRef } from "react";
import { useTranslation } from "react-i18next";
import { AutomaticMicrophoneMask } from "../AutomaticMicrophoneMask";
import { MicrophoneInputBoost } from "../MicrophoneInputBoost";
import { MicrophoneSelector } from "../MicrophoneSelector";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { OutputDeviceSelector } from "../OutputDeviceSelector";
import { AudioFeedback } from "../AudioFeedback";
import { useSettings } from "../../../hooks/useSettings";
import { VolumeSlider } from "../VolumeSlider";
import { TranscriptionProfiles } from "../TranscriptionProfiles";
import { Button } from "../../ui/Button";
import type { SidebarSection } from "../../Sidebar";
import { useNavigationStore } from "../../../stores/navigationStore";

type ReadinessTarget = SidebarSection | "profiles" | "sound";

export const GeneralSettings: React.FC = () => {
  const { t } = useTranslation();
  const { audioFeedbackEnabled, settings } = useSettings();
  const { setSection } = useNavigationStore();
  const profilesRef = useRef<HTMLDivElement | null>(null);
  const soundRef = useRef<HTMLDivElement | null>(null);

  const transcriptionProvider = String(settings?.transcription_provider || "local");
  const profiles = settings?.transcription_profiles || [];
  const activeProfileId = settings?.active_profile_id || "default";
  const activeProfile =
    activeProfileId === "default"
      ? null
      : profiles.find((profile) => profile.id === activeProfileId) || null;
  const activeProfileShortcut =
    settings?.bindings?.transcribe_active_profile?.current_binding?.trim() || "";
  const localModelReady = Boolean(settings?.selected_model);
  const remoteModelReady =
    transcriptionProvider === "remote_openai_compatible"
      ? Boolean(settings?.remote_stt?.model_id)
      : transcriptionProvider === "remote_soniox"
        ? Boolean(settings?.soniox_model)
        : transcriptionProvider === "remote_deepgram"
          ? Boolean(settings?.deepgram_model)
          : false;
  const transcriptionPathReady =
    transcriptionProvider === "local" ? localModelReady : remoteModelReady;
  const currentLlmProviderId = settings?.post_process_provider_id || "";
  const llmApiKey =
    currentLlmProviderId.length > 0
      ? settings?.post_process_api_keys?.[currentLlmProviderId]?.trim() || ""
      : "";
  const llmReady = Boolean(
    settings?.post_process_enabled ||
      (currentLlmProviderId &&
        (currentLlmProviderId === "apple_intelligence" || llmApiKey)),
  );
  const previewReady =
    activeProfileId === "default"
      ? Boolean(settings?.preview_output_only_enabled ?? false)
      : Boolean(activeProfile?.preview_output_only_enabled ?? false);

  const scrollToSection = (target: "profiles" | "sound") => {
    const ref = target === "profiles" ? profilesRef : soundRef;
    ref.current?.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  const handleReadinessAction = (target: ReadinessTarget) => {
    if (target === "profiles" || target === "sound") {
      scrollToSection(target);
      return;
    }
    setSection(target);
  };

  const readinessItems = [
    {
      key: "microphone",
      done: Boolean(settings?.selected_microphone),
      title: t("settings.generalReadiness.microphone.title"),
      detail: t("settings.generalReadiness.microphone.detail"),
      actionLabel: t("settings.generalReadiness.microphone.action"),
      target: "sound" as const,
    },
    {
      key: "transcriptionPath",
      done: transcriptionPathReady,
      title: t("settings.generalReadiness.transcriptionPath.title"),
      detail: t("settings.generalReadiness.transcriptionPath.detail", {
        mode:
          transcriptionProvider === "local"
            ? t("settings.generalReadiness.transcriptionPath.localMode")
            : t("settings.generalReadiness.transcriptionPath.remoteMode"),
      }),
      actionLabel: t("settings.generalReadiness.transcriptionPath.action"),
      target: "models" as const,
    },
    {
      key: "shortcut",
      done: activeProfileShortcut.length > 0,
      title: t("settings.generalReadiness.shortcut.title"),
      detail:
        activeProfileShortcut.length > 0
          ? t("settings.generalReadiness.shortcut.readyDetail", {
              binding: activeProfileShortcut,
            })
          : t("settings.generalReadiness.shortcut.detail"),
      actionLabel: t("settings.generalReadiness.shortcut.action"),
      target: "profiles" as const,
    },
    {
      key: "llm",
      done: llmReady,
      title: t("settings.generalReadiness.llm.title"),
      detail: t("settings.generalReadiness.llm.detail"),
      actionLabel: t("settings.generalReadiness.llm.action"),
      target: "postprocessing" as const,
    },
    {
      key: "preview",
      done: previewReady,
      title: t("settings.generalReadiness.preview.title"),
      detail: t("settings.generalReadiness.preview.detail"),
      actionLabel: t("settings.generalReadiness.preview.action"),
      target: "userInterface" as const,
    },
  ];

  return (
    <div className="max-w-3xl w-full mx-auto space-y-8 pb-12">
      <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/8 p-4">
        <div className="space-y-3">
          <div>
            <p className="text-sm font-semibold text-text">
              {t("settings.generalReadiness.title")}
            </p>
            <p className="text-xs text-text/70">
              {t("settings.generalReadiness.description")}
            </p>
          </div>
          <div className="space-y-2">
            {readinessItems.map((item) => (
              <div
                key={item.key}
                className="flex flex-col gap-3 rounded-lg border border-white/8 bg-black/10 px-3 py-3 sm:flex-row sm:items-center sm:justify-between"
              >
                <div className="min-w-0">
                  <div className="flex flex-wrap items-center gap-2">
                    <span
                      className={`inline-flex rounded-full border px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide ${
                        item.done
                          ? "border-emerald-500/30 bg-emerald-500/15 text-emerald-300"
                          : "border-amber-500/30 bg-amber-500/15 text-amber-300"
                      }`}
                    >
                      {item.done
                        ? t("settings.generalReadiness.status.ready")
                        : t("settings.generalReadiness.status.todo")}
                    </span>
                    <span className="text-sm font-medium text-text">{item.title}</span>
                  </div>
                  <p className="mt-1 text-xs text-text/70">{item.detail}</p>
                </div>
                <Button
                  variant={item.done ? "ghost" : "secondary"}
                  size="sm"
                  onClick={() => handleReadinessAction(item.target)}
                  className="shrink-0"
                >
                  {item.actionLabel}
                </Button>
              </div>
            ))}
          </div>
        </div>
      </div>

      <div ref={profilesRef}>
        <TranscriptionProfiles />
      </div>

      <div ref={soundRef}>
        <SettingsGroup title={t("settings.sound.title")}>
          <MicrophoneSelector descriptionMode="tooltip" grouped={true} />
          <MicrophoneInputBoost descriptionMode="tooltip" grouped={true} />
          <AutomaticMicrophoneMask descriptionMode="tooltip" grouped={true} />
          <AudioFeedback descriptionMode="tooltip" grouped={true} />
          <OutputDeviceSelector
            descriptionMode="tooltip"
            grouped={true}
            disabled={!audioFeedbackEnabled}
          />
          <VolumeSlider disabled={!audioFeedbackEnabled} />
        </SettingsGroup>
      </div>
    </div>
  );
};
