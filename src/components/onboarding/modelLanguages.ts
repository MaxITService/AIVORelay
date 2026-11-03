import { languageIdentity } from "@handy-computer/models";
import { MODEL_CAPABILITY_LANGUAGES } from "@/lib/constants/languages";

const knownLanguages = new Set(MODEL_CAPABILITY_LANGUAGES.map((language) => languageIdentity(language.value)));

/** Locale suggestions are for model comparison, not dictation settings. */
export function suggestedModelLanguages(locales: readonly string[]): string[] {
  const result: string[] = [];
  for (const locale of locales) {
    const code = languageIdentity(locale.split(/[.@]/, 1)[0].replace(/_/g, "-"));
    if (knownLanguages.has(code) && !result.includes(code)) result.push(code);
  }
  return result;
}

export function missingModelLanguages(supported: readonly string[], requested: readonly string[]): string[] {
  const known = new Set(supported.map(languageIdentity));
  return requested.filter((language) => !known.has(languageIdentity(language)));
}
