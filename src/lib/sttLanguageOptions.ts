import type { ModelInfo } from "@/bindings";
import {
  ELEVENLABS_ADDITIONAL_LANGUAGE_OPTIONS,
  filterLanguageOptionsForElevenLabs,
} from "./constants/elevenlabsLanguages";
import {
  LANGUAGES,
  supportsLanguageCode,
  type Language,
} from "./constants/languages";
import {
  isLanguageSupportedByReson8,
  RESON8_ADDITIONAL_LANGUAGE_OPTIONS,
} from "./constants/reson8Languages";
import { isLanguageSupportedBySoniox } from "./constants/sonioxLanguages";
import {
  isElevenLabsSelection,
  isReson8Selection,
  type SttModelSelection,
} from "./sttModelSelection";

/** Provider/model-scoped language choices for a profile STT selection. */
export const sttLanguageOptions = (
  selection: SttModelSelection,
  localModels: ModelInfo[],
): Language[] => {
  if (selection.provider === "remote_soniox") {
    return LANGUAGES.filter((language) =>
      isLanguageSupportedBySoniox(language.value),
    );
  }

  if (isReson8Selection(selection)) {
    const options = LANGUAGES.filter((language) =>
      isLanguageSupportedByReson8(language.value),
    );
    return [
      ...options,
      ...RESON8_ADDITIONAL_LANGUAGE_OPTIONS.filter(
        (extra) => !options.some((option) => option.value === extra.value),
      ),
    ];
  }

  if (isElevenLabsSelection(selection)) {
    const options = filterLanguageOptionsForElevenLabs(LANGUAGES);
    return [
      ...options,
      ...ELEVENLABS_ADDITIONAL_LANGUAGE_OPTIONS.filter(
        (extra) => !options.some((option) => option.value === extra.value),
      ),
    ];
  }

  const localModel =
    selection.provider === "local"
      ? localModels.find((model) => model.id === selection.model_id)
      : undefined;
  if (
    selection.provider === "local" &&
    localModel?.supported_languages?.length
  ) {
    return LANGUAGES.filter(
      (language) =>
        language.value === "auto" ||
        language.value === "os_input" ||
        supportsLanguageCode(localModel.supported_languages, language.value),
    );
  }

  return LANGUAGES;
};
