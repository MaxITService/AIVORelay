import React from "react";
import { AlertTriangle } from "lucide-react";
import { useTranslation } from "react-i18next";

export type ProfileScopedSttFeature =
  | "language"
  | "translateToEnglish"
  | "systemPrompt"
  | "geminiLanguage"
  | "customVocabulary"
  | "strictLanguageHint"
  | "contextJson"
  | "contextText"
  | "contextTerms";

const FEATURE_FALLBACKS: Record<ProfileScopedSttFeature, string> = {
  language: "Language",
  translateToEnglish: "Translate to English",
  systemPrompt: "System prompt",
  geminiLanguage: "Gemini language",
  customVocabulary: "Custom vocabulary",
  strictLanguageHint: "Strict language hint",
  contextJson: "Context JSON",
  contextText: "Context text",
  contextTerms: "Context terms",
};

export const ProfileScopedSttSettingsNotice: React.FC<{
  features: ProfileScopedSttFeature[];
  className?: string;
}> = ({ features, className = "" }) => {
  const { t } = useTranslation();

  if (features.length === 0) return null;

  return (
    <div
      className={`rounded-lg border border-red-500/45 bg-red-500/10 px-3 py-3 text-red-100 ${className}`}
    >
      <div className="flex items-start gap-2.5">
        <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-red-400" />
        <div className="min-w-0">
          <p className="text-xs font-semibold text-red-300">
            {t(
              "modelSelector.profileSettingsNotice.title",
              "This model has additional settings in transcription profiles",
            )}
          </p>
          <p className="mt-1 text-[11px] leading-snug text-red-200/80">
            {t(
              "modelSelector.profileSettingsNotice.description",
              "Check Speech / Mic → Manage Profiles. Profile settings override or extend the model settings shown here.",
            )}
          </p>
          <div className="mt-2 flex flex-wrap gap-1.5">
            {features.map((feature) => (
              <span
                key={feature}
                className="rounded-full border border-red-400/35 bg-red-500/15 px-2 py-0.5 text-[10px] font-medium text-red-200"
              >
                {t(
                  `modelSelector.profileSettingsNotice.features.${feature}`,
                  FEATURE_FALLBACKS[feature],
                )}
              </span>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};
