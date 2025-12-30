import React from "react";
import { useTranslation } from "react-i18next";
import { Info } from "lucide-react";
import { ShowOverlay } from "../ShowOverlay";
import { TranslateToEnglish } from "../TranslateToEnglish";
import { ModelUnloadTimeoutSetting } from "../ModelUnloadTimeout";
import { CustomWords } from "../CustomWords";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { StartHidden } from "../StartHidden";
import { AutostartToggle } from "../AutostartToggle";
import { PasteMethodSetting } from "../PasteMethod";
import { ClipboardHandlingSetting } from "../ClipboardHandling";
import { RemoteSttSettings } from "../remote-stt/RemoteSttSettings";

export const AdvancedSettings: React.FC = () => {
  const { t } = useTranslation();
  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      {/* Help Banner */}
      <div className="rounded-lg border border-purple-500/30 bg-purple-500/10 p-4">
        <div className="flex items-start gap-3">
          <Info className="w-5 h-5 text-purple-400 mt-0.5 flex-shrink-0" />
          <div className="space-y-1 text-sm text-text/80">
            <p className="font-medium text-text">
              {t("settings.advanced.help.title")}
            </p>
            <p>
              {t("settings.advanced.help.description")}
            </p>
          </div>
        </div>
      </div>

      <SettingsGroup title={t("settings.advanced.title")}>
        <StartHidden descriptionMode="tooltip" grouped={true} />
        <AutostartToggle descriptionMode="tooltip" grouped={true} />
        <ShowOverlay descriptionMode="tooltip" grouped={true} />
        <PasteMethodSetting descriptionMode="tooltip" grouped={true} />
        <ClipboardHandlingSetting descriptionMode="tooltip" grouped={true} />
        <TranslateToEnglish descriptionMode="tooltip" grouped={true} />
        <ModelUnloadTimeoutSetting descriptionMode="tooltip" grouped={true} />
        <CustomWords descriptionMode="tooltip" grouped />
        <RemoteSttSettings descriptionMode="tooltip" grouped={true} />
      </SettingsGroup>
    </div>
  );
};
