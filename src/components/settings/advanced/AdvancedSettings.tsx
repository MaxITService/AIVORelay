import React from "react";
import { useTranslation } from "react-i18next";
import { ModelUnloadTimeoutSetting } from "../ModelUnloadTimeout";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { StartHidden } from "../StartHidden";
import { AutostartToggle } from "../AutostartToggle";
import { PasteMethodSetting } from "../PasteMethod";
import { ClipboardHandlingSetting } from "../ClipboardHandling";
import { RemoteSttSettings } from "../remote-stt/RemoteSttSettings";
import { TellMeMore } from "../../ui/TellMeMore";

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
              <strong>{t("settings.advanced.tellMeMore.remoteStt.title")}</strong>{" "}
              {t("settings.advanced.tellMeMore.remoteStt.description")}
              <div className="ml-5 mt-2 p-2 bg-accent/10 border border-accent/20 rounded-md text-xs">
                <p className="mb-1">{t("settings.advanced.tellMeMore.remoteStt.recommendation")}</p>
                <a
                  href="https://console.groq.com"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-accent hover:underline font-medium"
                >
                  console.groq.com
                </a>
                <p className="mt-1 text-text/70">{t("settings.advanced.tellMeMore.remoteStt.freeTier")}</p>
              </div>
            </li>
            <li>
              <strong>{t("settings.advanced.tellMeMore.startup.title")}</strong>{" "}
              {t("settings.advanced.tellMeMore.startup.description")}
            </li>
            <li>
              <strong>{t("settings.advanced.tellMeMore.pasteMethod.title")}</strong>{" "}
              {t("settings.advanced.tellMeMore.pasteMethod.description")}
              <ul className="list-disc list-inside ml-5 mt-1 text-text/80 text-xs">
                <li><em>{t("settings.advanced.tellMeMore.pasteMethod.clipboard")}</em></li>
                <li><em>{t("settings.advanced.tellMeMore.pasteMethod.direct")}</em></li>
              </ul>
            </li>
            <li>
              <strong>{t("settings.advanced.tellMeMore.clipboardHandling.title")}</strong>{" "}
              {t("settings.advanced.tellMeMore.clipboardHandling.description")}
            </li>
            <li>
              <strong>{t("settings.advanced.tellMeMore.modelUnload.title")}</strong>{" "}
              {t("settings.advanced.tellMeMore.modelUnload.description")}
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
        <PasteMethodSetting descriptionMode="tooltip" grouped={true} />
        <ClipboardHandlingSetting descriptionMode="tooltip" grouped={true} />
        <ModelUnloadTimeoutSetting descriptionMode="tooltip" grouped={true} />
      </SettingsGroup>

      <SettingsGroup title={t("settings.advanced.voiceApiTitle")}>
        <RemoteSttSettings descriptionMode="tooltip" grouped={true} />
      </SettingsGroup>
    </div>
  );
};
