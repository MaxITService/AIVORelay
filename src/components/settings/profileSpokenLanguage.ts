import type { AppSettings, ModelInfo } from "@/bindings";
import { isMoonshineStreamingModel } from "../model-selector/nativeStreamingModel";
import { recognitionLanguage, supportsLanguageCode } from "../../lib/constants/languages";

type LanguageProfile = {
  language: string;
  spoken_languages?: string[];
  language_detection?: "when_needed" | "always";
  preview_output_only_enabled?: boolean;
};
type LanguageModel = Pick<ModelInfo, "engine_type" | "supported_languages" | "supports_language_detection"> &
  Partial<Pick<ModelInfo, "id" | "supports_streaming">>;
type NativeStreamSettings = Pick<AppSettings, "preview_output_only_enabled" | "native_streaming_live_output_models">;

export const usesNativeLocalStreamingForProfile = (
  profile: Pick<LanguageProfile, "preview_output_only_enabled">,
  model?: LanguageModel,
  settings?: NativeStreamSettings | null,
): boolean => {
  if (model?.engine_type !== "TranscribeCpp" || !model.supports_streaming) return false;
  const previewOutput = Boolean(profile.preview_output_only_enabled ?? settings?.preview_output_only_enabled);
  const liveOutput = Boolean(
    model.id &&
    !isMoonshineStreamingModel({ id: model.id, name: "", filename: "" }) &&
    (settings?.native_streaming_live_output_models ?? []).includes(model.id),
  );
  return previewOutput || liveOutput;
};

// Expansion is deliberately absent: opening settings must not change the mode.
export const getProfileSpokenLanguageState = (
  profile: LanguageProfile,
  model?: LanguageModel,
  settings?: NativeStreamSettings | null,
) => {
  const preferred = profile.spoken_languages ?? [];
  const languages = model?.supported_languages ?? [];
  const modelLanguages = [...new Set(languages.map(recognitionLanguage))];
  const acceptsSelection = Boolean(model &&
    ["TranscribeCpp", "Whisper", "SenseVoice", "Canary", "Cohere"].includes(model.engine_type));
  const nativeStreamingAuto = Boolean(
    model?.supports_language_detection && usesNativeLocalStreamingForProfile(profile, model, settings),
  );
  const canUsePreferences = acceptsSelection && modelLanguages.length > 1 && !nativeStreamingAuto;
  const compatible = preferred.filter(code => supportsLanguageCode(languages, code))
    .filter((code, index, codes) => codes.findIndex(other => recognitionLanguage(other) === recognitionLanguage(code)) === index);
  const selected = profile.language === "auto" && canUsePreferences &&
    (profile.language_detection === "always" || (!model?.supports_language_detection && preferred.length > 0));
  const summaryKey = !acceptsSelection ? "inactiveModel"
    : !modelLanguages.length ? "unknownModelLanguages"
    : modelLanguages.length === 1 ? "singleModelLanguage"
    : nativeStreamingAuto ? "nativeStreamingAuto"
    : !selected ? (model?.supports_language_detection ? "modelAuto" : "modelDefaultAuto")
    : !preferred.length ? "chooseLanguages"
    : !compatible.length ? "noCompatible"
    : compatible.length === 1 ? "singleCompatible" : "autoSummary";
  return {
    canUsePreferences, selected, compatible, summaryKey,
    summaryLanguage: modelLanguages.length === 1 ? languages[0] : compatible[0],
  };
};
