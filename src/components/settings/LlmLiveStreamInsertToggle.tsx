import React, { useState } from "react";
import { Trans, useTranslation } from "react-i18next";
import { commands } from "@/bindings";
import { sessionToast as toast } from "@/lib/sessionToast";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";
import {
  getActiveProfilePostProcessingEnabled,
  getPostProcessingAvailability,
} from "../../lib/postProcessingAvailability";
import { openActiveTranscriptionProfile } from "../../lib/transcriptionProfileNavigation";

interface LlmLiveStreamInsertToggleProps {
  grouped?: boolean;
}

/** Live insertion of the streamed LLM answer for the active profile. */
export const LlmLiveStreamInsertToggle: React.FC<LlmLiveStreamInsertToggleProps> =
  React.memo(({ grouped = false }) => {
    const { t } = useTranslation();
    const { settings, updateSetting, isUpdating, refreshSettings } = useSettings();
    const [isSavingProfile, setIsSavingProfile] = useState(false);
    const activeProfileId = String(settings?.active_profile_id || "default");
    // A missing custom profile falls back to the Default profile, as post-processing does.
    const activeProfile = activeProfileId === "default"
      ? undefined
      : settings?.transcription_profiles?.find((profile) => profile.id === activeProfileId);
    const checked = activeProfile
      ? Boolean(activeProfile.llm_live_stream_insert)
      : Boolean(settings?.post_process_live_stream_insert);
    const postProcessingActive =
      getPostProcessingAvailability(settings).available &&
      getActiveProfilePostProcessingEnabled(settings);

    const handleChange = async (enabled: boolean) => {
      if (!activeProfile) {
        await updateSetting("post_process_live_stream_insert", enabled);
        return;
      }
      setIsSavingProfile(true);
      try {
        const result = await commands.changePostProcessLiveStreamInsertSetting(enabled, activeProfile.id);
        if (result.status === "error") throw new Error(String(result.error));
      } catch (error) {
        toast.error(String(error));
      } finally {
        await refreshSettings();
        setIsSavingProfile(false);
      }
    };

    return (
      <ToggleSwitch
        id="settings-llm-live-stream-insert"
        checked={checked}
        onChange={handleChange}
        isUpdating={isSavingProfile || isUpdating("post_process_live_stream_insert")}
        disabled={!postProcessingActive}
        label={t("settings.postProcessing.liveStreamInsert.title")}
        description={
          <>
            <Trans
              i18nKey="settings.postProcessing.liveStreamInsert.activeProfileDescription"
              components={{
                profile: (
                  <button
                    type="button"
                    className="underline underline-offset-2 hover:text-text"
                    onClick={openActiveTranscriptionProfile}
                  />
                ),
              }}
            />
            <span className="block mt-1 text-amber-400/90">
              {t("settings.postProcessing.liveStreamInsert.replacementWarning")}
            </span>
          </>
        }
        descriptionMode="inline"
        grouped={grouped}
      />
    );
  });
