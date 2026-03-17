import React from "react";
import { useTranslation } from "react-i18next";
import { ModelUnloadTimeoutSetting } from "../ModelUnloadTimeout";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { StartHidden } from "../StartHidden";
import { AutostartToggle } from "../AutostartToggle";
import { PasteMethodSetting } from "../PasteMethod";
import { ClipboardHandlingSetting } from "../ClipboardHandling";
import { ConvertLfToCrlfSetting } from "../ConvertLfToCrlfSetting";
import { AutoSubmit } from "../AutoSubmit";
import { TellMeMore } from "../../ui/TellMeMore";
import { RecordingAutoStop } from "../RecordingAutoStop";
import { AccelerationSelector } from "../AccelerationSelector";

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
              <strong>{t("settings.advanced.tellMeMore.customWords.title")}</strong>{" "}
              {t("settings.advanced.tellMeMore.customWords.description")}
            </li>
          </ul>
          <p className="pt-2 text-xs text-text/70">
            {t("settings.advanced.tellMeMore.tip")}
          </p>
        </div>
      </TellMeMore>

      <SettingsGroup title={t("settings.advanced.title")}>
        <StartHidden descriptionMode="tooltip" grouped={true} />
        <AutostartToggle descriptionMode="tooltip" grouped={true} />
        <AutoSubmit descriptionMode="tooltip" grouped={true} />
        <RecordingAutoStop descriptionMode="tooltip" grouped={true} />
        <div className="px-6 pt-4">
          <TellMeMore title={t("settings.advanced.tellMeMore.modelUnload.title")}>
            <p className="text-text/90">
              {t("settings.advanced.tellMeMore.modelUnload.description")}
            </p>
          </TellMeMore>
        </div>
        <ModelUnloadTimeoutSetting descriptionMode="tooltip" grouped={true} />
        <AccelerationSelector descriptionMode="tooltip" grouped={true} />
      </SettingsGroup>

      <SettingsGroup title={t("settings.advanced.pasteMethod.title")}>
        <div className="px-6 pt-4">
          <TellMeMore title={t("settings.advanced.pasteMethod.tellMeMore.title")}>
            <div className="space-y-3">
              <p className="mb-2">
                <strong>{t("settings.advanced.pasteMethod.tellMeMore.headline")}</strong>
              </p>
              <p className="mb-2">
                {t("settings.advanced.pasteMethod.tellMeMore.intro")}
              </p>

              <div className="space-y-2 ml-2">
                <p>
                  <strong>{t("settings.advanced.pasteMethod.tellMeMore.ctrlV.title")}</strong>{" "}
                  {t("settings.advanced.pasteMethod.tellMeMore.ctrlV.description")}
                </p>
                <p>
                  <strong>{t("settings.advanced.pasteMethod.tellMeMore.direct.title")}</strong>{" "}
                  {t("settings.advanced.pasteMethod.tellMeMore.direct.description")}
                </p>
                <p>
                  <strong>{t("settings.advanced.pasteMethod.tellMeMore.none.title")}</strong>{" "}
                  {t("settings.advanced.pasteMethod.tellMeMore.none.description")}
                </p>
                <p>
                  <strong>{t("settings.advanced.pasteMethod.tellMeMore.pasteDelay.title")}</strong>{" "}
                  {t("settings.advanced.pasteMethod.tellMeMore.pasteDelay.description")}
                </p>
              </div>

              <p className="mt-3 text-text/70 text-xs">
                {t("settings.advanced.pasteMethod.tellMeMore.tip")}
              </p>
            </div>
          </TellMeMore>
        </div>
        <PasteMethodSetting descriptionMode="tooltip" grouped={true} />
        <div className="px-6 pt-4">
          <TellMeMore title={t("settings.advanced.tellMeMore.clipboardHandling.title")}>
            <div className="space-y-3 text-text/90">
              <p>{t("settings.advanced.tellMeMore.clipboardHandling.intro")}</p>
              <p className="text-sm">
                <strong>{t("settings.advanced.tellMeMore.clipboardHandling.safestChoiceTitle")}</strong>{" "}
                {t("settings.advanced.tellMeMore.clipboardHandling.safestChoiceDescription")}
              </p>
              <div className="space-y-2 ml-2">
                <p>
                  <strong>{t("settings.advanced.tellMeMore.clipboardHandling.dontModify.title")}</strong>{" "}
                  {t("settings.advanced.tellMeMore.clipboardHandling.dontModify.description")}
                </p>
                <p>
                  <strong>{t("settings.advanced.tellMeMore.clipboardHandling.copyToClipboard.title")}</strong>{" "}
                  {t("settings.advanced.tellMeMore.clipboardHandling.copyToClipboard.description")}
                </p>
                <p>
                  <strong>{t("settings.advanced.tellMeMore.clipboardHandling.restoreAdvanced.title")}</strong>{" "}
                  {t("settings.advanced.tellMeMore.clipboardHandling.restoreAdvanced.description")}
                </p>
              </div>
              <p className="text-xs text-text/70">
                {t("settings.advanced.tellMeMore.clipboardHandling.note")}
              </p>
            </div>
          </TellMeMore>
        </div>
        <ClipboardHandlingSetting descriptionMode="tooltip" grouped={true} />
        <ConvertLfToCrlfSetting descriptionMode="tooltip" grouped={true} />
      </SettingsGroup>

    </div>
  );
};
