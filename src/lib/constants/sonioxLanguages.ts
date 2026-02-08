export const SONIOX_SUPPORTED_LANGUAGE_CODES = new Set<string>([
  "af",
  "sq",
  "ar",
  "az",
  "eu",
  "be",
  "bn",
  "bs",
  "bg",
  "ca",
  "zh",
  "hr",
  "cs",
  "da",
  "nl",
  "en",
  "et",
  "fi",
  "fr",
  "gl",
  "de",
  "el",
  "gu",
  "he",
  "hi",
  "hu",
  "id",
  "it",
  "ja",
  "kn",
  "kk",
  "ko",
  "lv",
  "lt",
  "mk",
  "ms",
  "ml",
  "mr",
  "no",
  "fa",
  "pl",
  "pt",
  "pa",
  "ro",
  "ru",
  "sr",
  "sk",
  "sl",
  "es",
  "sw",
  "sv",
  "tl",
  "ta",
  "te",
  "th",
  "tr",
  "uk",
  "ur",
  "vi",
  "cy",
]);

const canonicalizeLanguageCode = (value: string): string | null => {
  const trimmed = value.trim();
  if (!trimmed) return null;

  const normalized = trimmed.toLowerCase().replace(/_/g, "-");
  if (normalized === "zh-hans" || normalized === "zh-hant") {
    return "zh";
  }

  const primary = normalized.split("-")[0]?.trim();
  if (!primary) return null;
  return primary;
};

export const normalizeLanguageForSoniox = (value: string): string | null => {
  if (!value) return null;
  if (value === "auto" || value === "os_input") return null;
  return canonicalizeLanguageCode(value);
};

export const isLanguageSupportedBySoniox = (value: string): boolean => {
  if (value === "auto" || value === "os_input") return true;
  const normalized = normalizeLanguageForSoniox(value);
  return !!normalized && SONIOX_SUPPORTED_LANGUAGE_CODES.has(normalized);
};

export const parseAndNormalizeSonioxLanguageHints = (
  input: string,
): { normalized: string[]; rejected: string[] } => {
  const seen = new Set<string>();
  const normalized: string[] = [];
  const rejected: string[] = [];

  for (const rawPart of input.split(",")) {
    const raw = rawPart.trim();
    if (!raw) continue;

    const canonical = canonicalizeLanguageCode(raw);
    if (canonical && SONIOX_SUPPORTED_LANGUAGE_CODES.has(canonical)) {
      if (!seen.has(canonical)) {
        seen.add(canonical);
        normalized.push(canonical);
      }
      continue;
    }

    rejected.push(raw);
  }

  return { normalized, rejected };
};
