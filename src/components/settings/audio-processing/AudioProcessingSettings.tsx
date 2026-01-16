import React from "react";
import { useTranslation } from "react-i18next";
import { HelpCircle, ChevronDown, RotateCcw } from "lucide-react";
import { useSettings } from "@/hooks/useSettings";
import { SettingsGroup } from "@/components/ui/SettingsGroup";

export const AudioProcessingSettings: React.FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting, isUpdating } = useSettings();

  const handleResetVad = () => {
    updateSetting("vad_threshold", 0.3);
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6 pb-12">
      <SettingsGroup
        title={t("audioProcessing.title", "Speech Processing")}
        description={t(
          "audioProcessing.description",
          "Configure voice activity detection and speech artifact filtering."
        )}
      >
        {/* VAD Threshold */}
        <div className="px-4 py-4">
          <div className="flex items-center justify-between mb-2">
            <label className="text-sm text-[#f5f5f5]">
              {t("audioProcessing.vadThreshold", "Voice Detection Sensitivity")}
            </label>
            <div className="flex items-center gap-3">
              <span className="text-sm text-[#9b5de5] font-mono min-w-[24px] text-right">
                {(settings?.vad_threshold ?? 0.3).toFixed(1)}
              </span>
              <button
                onClick={handleResetVad}
                disabled={isUpdating("vad_threshold")}
                className="p-1.5 text-[#606060] hover:text-[#f5f5f5] transition-colors rounded-md hover:bg-[#333333]"
                title={t("common.reset", "Reset")}
              >
                <RotateCcw className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>
          
          <input
            type="range"
            min="0.1"
            max="0.9"
            step="0.1"
            value={settings?.vad_threshold ?? 0.3}
            onChange={(e) => updateSetting("vad_threshold", parseFloat(e.target.value))}
            className="w-full h-2 bg-[#252525] rounded-lg appearance-none cursor-pointer accent-[#9b5de5]"
            disabled={isUpdating("vad_threshold")}
          />
          
          <div className="flex justify-between text-xs text-[#606060] mt-2">
            <span>{t("audioProcessing.moreSensitive", "More sensitive")}</span>
            <span>{t("audioProcessing.lessSensitive", "Less sensitive")}</span>
          </div>
        </div>

        {/* VAD Threshold Help */}
        <div className="px-4 py-3 border-t border-white/[0.05]">
          <details className="group">
            <summary className="flex items-center gap-2 text-sm text-[#9b5de5] hover:text-[#b47eff] transition-colors cursor-pointer list-none">
              <HelpCircle className="w-4 h-4" />
              {t("audioProcessing.vadHelpTitle", "Tell me more about voice detection sensitivity")}
              <ChevronDown className="w-4 h-4 group-open:rotate-180 transition-transform" />
            </summary>
            <div className="mt-3 p-4 bg-[#1a1a1a] rounded-lg border border-[#333333] text-sm">
              <h4 className="font-medium text-[#f5f5f5] mb-2">
                {t("audioProcessing.vadExplanation", "Voice Activity Detection (VAD)")}
              </h4>
              <p className="text-[#b8b8b8] mb-3">
                {t(
                  "audioProcessing.vadDescription",
                  "VAD determines when you're speaking vs when there's silence or background noise. The threshold controls how confident the system must be before recording audio."
                )}
              </p>
              <div className="space-y-3">
                <div className="flex items-start gap-3 p-2 bg-[#252525] rounded">
                  <div className="px-2 py-1 bg-[#4ade80]/20 text-[#4ade80] rounded text-xs font-mono">0.3</div>
                  <div>
                    <p className="text-[#f5f5f5] font-medium text-xs">{t("audioProcessing.lowThreshold", "Low (Default)")}</p>
                    <p className="text-[#808080] text-xs">{t("audioProcessing.lowThresholdDesc", "Very sensitive — captures quiet speech but may include noise. Good for quiet environments.")}</p>
                  </div>
                </div>
                <div className="flex items-start gap-3 p-2 bg-[#252525] rounded">
                  <div className="px-2 py-1 bg-[#f59e0b]/20 text-[#f59e0b] rounded text-xs font-mono">0.5</div>
                  <div>
                    <p className="text-[#f5f5f5] font-medium text-xs">{t("audioProcessing.midThreshold", "Medium")}</p>
                    <p className="text-[#808080] text-xs">{t("audioProcessing.midThresholdDesc", "Balanced — reduces background noise while still capturing most speech.")}</p>
                  </div>
                </div>
                <div className="flex items-start gap-3 p-2 bg-[#252525] rounded">
                  <div className="px-2 py-1 bg-[#ef4444]/20 text-[#ef4444] rounded text-xs font-mono">0.7</div>
                  <div>
                    <p className="text-[#f5f5f5] font-medium text-xs">{t("audioProcessing.highThreshold", "High")}</p>
                    <p className="text-[#808080] text-xs">{t("audioProcessing.highThresholdDesc", "Conservative — only passes clear speech. Reduces stuttering artifacts but may miss quiet words.")}</p>
                  </div>
                </div>
              </div>
              <div className="mt-3 p-3 bg-[#252525] rounded border border-[#444444]">
                <p className="text-[#b8b8b8] text-xs">
                  <strong className="text-[#f5f5f5]">
                    {t("audioProcessing.tipTitle", "Tip:")}
                  </strong>{" "}
                  {t(
                    "audioProcessing.vadTip",
                    "If you're experiencing stuttering artifacts like 'wh wh wh why', try increasing this value to 0.5 or 0.6. The trade-off is that very quiet speech might get clipped."
                  )}
                </p>
              </div>
            </div>
          </details>
        </div>
      </SettingsGroup>
    </div>
  );
};

export default AudioProcessingSettings;
