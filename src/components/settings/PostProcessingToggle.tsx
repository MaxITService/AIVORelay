import React from "react";
import { Trans, useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";
import {
  getActiveProfilePostProcessingEnabled,
  getPostProcessingAvailability,
} from "../../lib/postProcessingAvailability";
import { openLivePreviewSettings } from "../../lib/transcriptionProfileNavigation";

interface PostProcessingToggleProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const PostProcessingToggle: React.FC<PostProcessingToggleProps> =
  React.memo(({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { settings, updateSetting, isUpdating } = useSettings();

    const configuredEnabled = getActiveProfilePostProcessingEnabled(settings);
    const availability = getPostProcessingAvailability(settings);
    const enabled = availability.available && configuredEnabled;
    const activeProfileId = String(settings?.active_profile_id || "default");
    const activeProfileName =
      activeProfileId === "default"
        ? t(
            "settings.postProcessing.defaultProfileName",
            "Default (Global)",
          )
        : settings?.transcription_profiles?.find(
            (profile) => profile.id === activeProfileId,
          )?.name || activeProfileId;

    return (
      <ToggleSwitch
        id="settings-llm-postprocessing-enabled"
        checked={enabled}
        onChange={(enabled) => updateSetting("post_process_enabled", enabled)}
        isUpdating={isUpdating("post_process_enabled")}
        disabled={!availability.available}
        label={t("settings.debug.postProcessingToggle.activeProfileLabel", {
          profile: activeProfileName,
          defaultValue: "Enable for active profile: {{profile}}",
        })}
        description={
          availability.available ? (
            t("settings.debug.postProcessingToggle.description")
          ) : (
            <Trans
              i18nKey="settings.postProcessing.unavailableDirectRealtime"
              defaultValue="LLM post-processing is unavailable while realtime text is inserted directly into the target application. Use <preview>Preview</preview> or a non-live output route. Your saved choice is preserved for compatible routes."
              components={{
                preview: (
                  <button
                    type="button"
                    className="underline underline-offset-2 hover:text-text"
                    onClick={openLivePreviewSettings}
                  />
                ),
              }}
            />
          )
        }
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  });
