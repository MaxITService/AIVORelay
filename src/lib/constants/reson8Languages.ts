import type { Language } from "./languages";

export const RESON8_RUMBLE_2_SUPPORTED_LANGUAGE_CODES = new Set<string>([
  "af",
  "ar",
  "az",
  "be",
  "bg",
  "bn",
  "bs",
  "ca",
  "ceb",
  "cs",
  "da",
  "de",
  "el",
  "en",
  "es",
  "et",
  "eu",
  "fa",
  "fi",
  "fil",
  "fr",
  "fy",
  "gl",
  "hi",
  "hr",
  "hu",
  "id",
  "is",
  "it",
  "ja",
  "jv",
  "ko",
  "lt",
  "lv",
  "mk",
  "mr",
  "ms",
  "mt",
  "nb",
  "nl",
  "pl",
  "pt",
  "ro",
  "ru",
  "sk",
  "sl",
  "sq",
  "sr",
  "sv",
  "ta",
  "th",
  "tr",
  "uk",
  "vi",
  "yue",
  "zh",
]);

const RESON8_LANGUAGE_ALIASES: Record<string, string> = {
  no: "nb",
  tl: "fil",
  jw: "jv",
};

/** Canonical Rumble-2 code for a user or OS language identity. */
export const normalizeLanguageForReson8 = (value: string): string | null => {
  const trimmed = value.trim();
  if (!trimmed) return null;

  const normalized = trimmed.toLowerCase().replace(/_/g, "-");
  if (normalized === "zh-hans" || normalized === "zh-hant") return "zh";

  const primary = normalized.split("-")[0]?.trim();
  if (!primary) return null;
  return RESON8_LANGUAGE_ALIASES[primary] ?? primary;
};

export const isLanguageSupportedByReson8 = (value: string): boolean => {
  if (value === "auto" || value === "os_input") return true;
  const normalized = normalizeLanguageForReson8(value);
  return (
    !!normalized && RESON8_RUMBLE_2_SUPPORTED_LANGUAGE_CODES.has(normalized)
  );
};

/** Rumble-2 supports these languages, but the shared UI catalog has no entries. */
export const RESON8_ADDITIONAL_LANGUAGE_OPTIONS: Language[] = [
  { value: "ceb", label: "Cebuano" },
  { value: "fy", label: "Frisian" },
];
