import React from "react";
import { useTranslation } from "react-i18next";
import { ModelUnloadTimeoutSetting } from "../ModelUnloadTimeout";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { StartHidden } from "../StartHidden";
import { AutostartToggle } from "../AutostartToggle";
import { TellMeMore } from "../../ui/TellMeMore";
import { RecordingAutoStop } from "../RecordingAutoStop";
import { AccelerationSelector } from "../AccelerationSelector";
import { MuteWhileRecording } from "../MuteWhileRecording";
import { PauseMediaWhileRecording } from "../PauseMediaWhileRecording";
import { ChineseScriptSetting } from "../ChineseScript";
import { NeverLaunchWebView } from "../NeverLaunchWebView";
import { openDefaultProfileOutputSettings } from "@/lib/transcriptionProfileNavigation";

export const AdvancedSettings: React.FC = () => {
  const { t } = useTranslation();
  return (
    <div className="max-w-3xl w-full mx-auto space-y-8 pb-12">
      {/* Help Section */}
      <TellMeMore title={t("settings.advanced.tellMeMore.title")}>
        <div className="space-y-3">
          <p>
            <strong>{t("settings.advanced.tellMeMore.headline")}</strong>
          </p>
          <p className="opacity-90">
            {t("settings.advanced.tellMeMore.intro")}
          </p>
          <ul className="list-disc list-inside space-y-2 ml-1 opacity-90">
            <li>
              <strong>{t("settings.advanced.tellMeMore.startup.title")}</strong>{" "}
              {t("settings.advanced.tellMeMore.startup.description")}
            </li>
            <li>
              <strong>
                {t("settings.advanced.tellMeMore.customWords.title")}
              </strong>{" "}
              {t("settings.advanced.tellMeMore.customWords.description")}
            </li>
          </ul>
          <p className="pt-2 text-xs text-text/70">
            {t("settings.advanced.tellMeMore.tip")}
          </p>
        </div>
      </TellMeMore>

      <SettingsGroup title={t("settings.advanced.title")}>
        <ChineseScriptSetting descriptionMode="tooltip" grouped={true} />
        <StartHidden descriptionMode="tooltip" grouped={true} />
        <AutostartToggle descriptionMode="tooltip" grouped={true} />
        <RecordingAutoStop descriptionMode="tooltip" grouped={true} />
        <MuteWhileRecording descriptionMode="tooltip" grouped={true} />
        <PauseMediaWhileRecording descriptionMode="tooltip" grouped={true} />
        <div className="px-6 pt-4">
          <TellMeMore
            title={t("settings.advanced.tellMeMore.modelUnload.title")}
          >
            <p className="text-text/90">
              {t("settings.advanced.tellMeMore.modelUnload.description")}
            </p>
          </TellMeMore>
        </div>
        <ModelUnloadTimeoutSetting descriptionMode="tooltip" grouped={true} />
        <div className="px-6 pt-4">
          <TellMeMore
            title={t("settings.advanced.tellMeMore.acceleration.title", {
              defaultValue: "Tell me more: Acceleration",
            })}
          >
            <div className="space-y-3 text-text/90">
              <p className="text-sm">
                <strong>
                  {t(
                    "settings.advanced.tellMeMore.acceleration.whisper.title",
                    {
                      defaultValue: "Whisper Acceleration:",
                    },
                  )}
                </strong>{" "}
                {t(
                  "settings.advanced.tellMeMore.acceleration.whisper.description",
                  {
                    defaultValue:
                      "Hardware acceleration for Whisper models. Auto uses GPU if available.",
                  },
                )}
              </p>
              <p className="text-sm">
                <strong>
                  {t("settings.advanced.tellMeMore.acceleration.ort.title", {
                    defaultValue: "ONNX Acceleration:",
                  })}
                </strong>{" "}
                {t(
                  "settings.advanced.tellMeMore.acceleration.ort.description",
                  {
                    defaultValue:
                      "Hardware acceleration for ONNX models like Parakeet, Canary, Moonshine, SenseVoice, and GigaAM.",
                  },
                )}
              </p>
              <p className="text-xs text-text/70">
                {t("settings.advanced.tellMeMore.acceleration.note", {
                  defaultValue:
                    "If you are unsure, leave both settings on Auto. Changing acceleration unloads the current local model so the new backend is used the next time it loads.",
                })}
              </p>
            </div>
          </TellMeMore>
        </div>
        <AccelerationSelector descriptionMode="tooltip" grouped={true} />
      </SettingsGroup>

      <SettingsGroup
        id="speech-only-low-memory-settings"
        title={t("settings.advanced.neverLaunchWebview.groupTitle")}
      >
        <NeverLaunchWebView />
      </SettingsGroup>

      <button
        type="button"
        id="advanced-paste-settings"
        onClick={openDefaultProfileOutputSettings}
        className="text-sm text-mid-gray underline underline-offset-2 hover:text-text"
      >
        {t("profileOutput.advancedLink")}
      </button>
    </div>
  );
};
