import { languageIdentity } from "@handy-computer/models";
import { MODEL_CAPABILITY_LANGUAGES } from "@/lib/constants/languages";
import { safeLocalStorage } from "@/lib/safeStorage";

const MODEL_LANGUAGES_KEY = "aivorelay-model-comparison-languages-v1";

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

/** Null means no preference yet; an empty list is a deliberate choice. */
export function readModelLanguagePreference(): string[] | null {
  const raw = safeLocalStorage.getItem(MODEL_LANGUAGES_KEY);
  return parseModelLanguagePreference(typeof raw === "string" ? raw : null);
}

export function parseModelLanguagePreference(raw: string | null): string[] | null {
  if (raw === null) return null;
  try {
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed) && parsed.every((value) => typeof value === "string")
      ? suggestedModelLanguages(parsed)
      : null;
  } catch {
    return null;
  }
}

export function saveModelLanguagePreference(languages: string[]): void {
  safeLocalStorage.setItem(MODEL_LANGUAGES_KEY, JSON.stringify(suggestedModelLanguages(languages)));
}
