import React from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../../ui/Button";
import { SettingContainer } from "../../ui/SettingContainer";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { Textarea } from "../../ui/Textarea";
import { useSettings } from "../../../hooks/useSettings";
import { TranscriptionProfiles } from "../TranscriptionProfiles";

export const LiveSoundTranscriptionSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings } = useSettings();

  const provider = String(settings?.transcription_provider ?? "local");
  const providerLabel =
    provider === "remote_soniox"
      ? "Soniox"
      : provider === "remote_deepgram"
        ? "Deepgram"
        : provider === "remote_openai_compatible"
          ? "OpenAI-compatible API"
          : t("settings.liveSoundTranscription.session.providerUnsupported");

  const modelLabel =
    provider === "remote_soniox"
      ? String((settings as any)?.soniox_model ?? "stt-rt-v4")
      : provider === "remote_deepgram"
        ? String((settings as any)?.deepgram_model ?? "nova-3")
        : String(settings?.selected_model ?? t("settings.liveSoundTranscription.session.notAvailable"));

  const liveModeEnabled =
    provider === "remote_soniox"
      ? Boolean((settings as any)?.soniox_live_enabled ?? true)
      : provider === "remote_deepgram"
        ? Boolean((settings as any)?.deepgram_live_enabled ?? true)
        : false;

  const previewEnabled = Boolean(
    (settings as any)?.soniox_live_preview_enabled ?? false,
  );

  const liveProviderReady =
    provider === "remote_soniox" || provider === "remote_deepgram";

  return (
    <div className="max-w-3xl w-full mx-auto space-y-8 pb-12">
      <SettingsGroup
        title={t("settings.liveSoundTranscription.title")}
        description={t("settings.liveSoundTranscription.description")}
      >
        <SettingContainer
          title={t("settings.liveSoundTranscription.session.title")}
          description={t("settings.liveSoundTranscription.session.description")}
          grouped={true}
          layout="stacked"
        >
          <div className="space-y-4">
            <div className="grid gap-3 md:grid-cols-2">
              <div className="rounded-lg border border-[#333333] bg-[#121212]/70 px-4 py-3">
                <p className="text-[11px] uppercase tracking-[0.18em] text-[#8a8a8a]">
                  {t("settings.liveSoundTranscription.session.providerLabel")}
                </p>
                <p className="mt-1 text-sm font-medium text-[#f5f5f5]">
                  {providerLabel}
                </p>
              </div>
              <div className="rounded-lg border border-[#333333] bg-[#121212]/70 px-4 py-3">
                <p className="text-[11px] uppercase tracking-[0.18em] text-[#8a8a8a]">
                  {t("settings.liveSoundTranscription.session.modelLabel")}
                </p>
                <p className="mt-1 text-sm font-medium text-[#f5f5f5]">
                  {modelLabel}
                </p>
              </div>
              <div className="rounded-lg border border-[#333333] bg-[#121212]/70 px-4 py-3">
                <p className="text-[11px] uppercase tracking-[0.18em] text-[#8a8a8a]">
                  {t("settings.liveSoundTranscription.session.liveModeLabel")}
                </p>
                <p className="mt-1 text-sm font-medium text-[#f5f5f5]">
                  {liveModeEnabled
                    ? t("settings.liveSoundTranscription.session.enabled")
                    : t("settings.liveSoundTranscription.session.disabled")}
                </p>
              </div>
              <div className="rounded-lg border border-[#333333] bg-[#121212]/70 px-4 py-3">
                <p className="text-[11px] uppercase tracking-[0.18em] text-[#8a8a8a]">
                  {t("settings.liveSoundTranscription.session.previewLabel")}
                </p>
                <p className="mt-1 text-sm font-medium text-[#f5f5f5]">
                  {previewEnabled
                    ? t("settings.liveSoundTranscription.session.enabled")
                    : t("settings.liveSoundTranscription.session.disabled")}
                </p>
              </div>
            </div>

            {!liveProviderReady && (
              <div className="rounded-lg border border-amber-500/25 bg-amber-500/10 px-4 py-3 text-sm text-amber-100">
                {t("settings.liveSoundTranscription.session.remoteOnly")}
              </div>
            )}

            <div className="flex flex-wrap gap-2">
              <Button variant="primary" disabled={true}>
                {t("settings.liveSoundTranscription.session.start")}
              </Button>
              <Button variant="secondary" disabled={true}>
                {t("settings.liveSoundTranscription.session.flush")}
              </Button>
              <Button variant="danger" disabled={true}>
                {t("settings.liveSoundTranscription.session.stop")}
              </Button>
            </div>

            <p className="text-sm text-[#9a9a9a]">
              {t("settings.liveSoundTranscription.session.placeholder")}
            </p>
          </div>
        </SettingContainer>
      </SettingsGroup>

      <SettingsGroup
        title={t("settings.liveSoundTranscription.transcript.title")}
        description={t("settings.liveSoundTranscription.transcript.description")}
      >
        <SettingContainer
          title={t("settings.liveSoundTranscription.transcript.previewTitle")}
          description={t("settings.liveSoundTranscription.transcript.previewDescription")}
          grouped={true}
          layout="stacked"
        >
          <Textarea
            readOnly
            value=""
            placeholder={t("settings.liveSoundTranscription.transcript.empty")}
            className="w-full"
          />
        </SettingContainer>
      </SettingsGroup>

      <TranscriptionProfiles />
    </div>
  );
};
