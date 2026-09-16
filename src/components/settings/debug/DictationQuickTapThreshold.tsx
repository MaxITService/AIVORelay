import { useTranslation } from "react-i18next";
import { useSettings } from "../../../hooks/useSettings";
import { SettingContainer } from "../../ui/SettingContainer";
import { CommittedNumberInput } from "../text-to-speech/CommittedNumberInput";

export const DictationQuickTapThreshold = () => {
  const { t } = useTranslation();
  const { settings, updateSetting, isUpdating } = useSettings();
  const value = (settings as { dictation_quick_tap_threshold_ms?: number } | null)
    ?.dictation_quick_tap_threshold_ms ?? 500;

  return (
    <div id="settings-dictation-quick-tap-threshold">
      <SettingContainer
        title={t("settings.debug.dictationQuickTap.title")}
        description={t("settings.debug.dictationQuickTap.description")}
        descriptionMode="inline"
        grouped
      >
        <CommittedNumberInput
          value={value}
          min={0}
          max={5000}
          step={1}
          onCommit={(nextValue) => updateSetting("dictation_quick_tap_threshold_ms" as any, nextValue)}
          disabled={isUpdating("dictation_quick_tap_threshold_ms")}
          aria-label={t("settings.debug.dictationQuickTap.title")}
          className="w-24"
        />
      </SettingContainer>
    </div>
  );
};
