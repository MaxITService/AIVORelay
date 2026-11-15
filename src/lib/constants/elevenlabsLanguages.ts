import type { Language } from "./languages";

/**
 * Canonical language codes officially documented as supported by ElevenLabs
 * Scribe v3 (`scribe_v3`) and Scribe v3 Live (`scribe_v3_live`).
 *
 * Matches `ELEVENLABS_SCRIBE_SUPPORTED_LANGUAGE_CODES` in
 * `src-tauri/src/elevenlabs_language.rs`.
 */
export const ELEVENLABS_SUPPORTED_LANGUAGE_CODES = new Set<string>([
  "af",
  "am",
  "ar",
  "as",
  "ast",
  "az",
  "be",
  "bg",
  "bn",
  "bs",
  "ca",
  "ceb",
  "cs",
  "cy",
  "da",
  "de",
  "el",
  "en",
  "es",
  "et",
  "fa",
  "ff",
  "fi",
  "fil",
  "fr",
  "ga",
  "gl",
  "gu",
  "ha",
  "he",
  "hi",
  "hr",
  "hu",
  "hy",
  "id",
  "ig",
  "is",
  "it",
  "ja",
  "jv",
  "ka",
  "kea",
  "kk",
  "km",
  "kn",
  "ko",
  "ku",
  "ky",
  "lb",
  "lg",
  "ln",
  "lo",
  "lt",
  "luo",
  "lv",
  "mi",
  "mk",
  "ml",
  "mn",
  "mr",
  "ms",
  "mt",
  "my",
  "ne",
  "nl",
  "no",
  "nso",
  "ny",
  "oc",
  "or",
  "pa",
  "pl",
  "ps",
  "pt",
  "ro",
  "ru",
  "sd",
  "si",
  "sk",
  "sl",
  "sn",
  "so",
  "sr",
  "sv",
  "sw",
  "ta",
  "te",
  "tg",
  "th",
  "tr",
  "uk",
  "umb",
  "ur",
  "uz",
  "vi",
  "wo",
  "xh",
  "yo",
  "yue",
  "zh",
  "zu",
]);

const ELEVENLABS_LANGUAGE_ALIASES: Record<string, string> = {
  // Legacy / AivoRelay / Norwegian / Filipino / Javanese / Hebrew / Indonesian / Yiddish / Chinese aliases
  nb: "no",
  nn: "no",
  nob: "no",
  nno: "no",
  nor: "no",
  tl: "fil",
  tgl: "fil",
  jw: "jv",
  jav: "jv",
  iw: "he",
  heb: "he",
  in: "id",
  ind: "id",
  ji: "yi",
  yid: "yi",
  zho: "zh",
  cmn: "zh",
  chi: "zh",
  // Official ElevenLabs ISO 639-3 codes -> canonical ISO 639-1 codes
  afr: "af",
  amh: "am",
  ara: "ar",
  asm: "as",
  aze: "az",
  bel: "be",
  ben: "bn",
  bos: "bs",
  bul: "bg",
  cat: "ca",
  ces: "cs",
  cze: "cs",
  cym: "cy",
  wel: "cy",
  dan: "da",
  deu: "de",
  ger: "de",
  ell: "el",
  gre: "el",
  eng: "en",
  est: "et",
  fas: "fa",
  per: "fa",
  fin: "fi",
  fra: "fr",
  fre: "fr",
  ful: "ff",
  gle: "ga",
  glg: "gl",
  guj: "gu",
  hau: "ha",
  hin: "hi",
  hrv: "hr",
  hun: "hu",
  hye: "hy",
  arm: "hy",
  ibo: "ig",
  isl: "is",
  ice: "is",
  ita: "it",
  jpn: "ja",
  kan: "kn",
  kat: "ka",
  geo: "ka",
  kaz: "kk",
  khm: "km",
  kir: "ky",
  kor: "ko",
  kur: "ku",
  lao: "lo",
  lav: "lv",
  lin: "ln",
  lit: "lt",
  ltz: "lb",
  lug: "lg",
  mal: "ml",
  mar: "mr",
  mkd: "mk",
  mac: "mk",
  mlt: "mt",
  mon: "mn",
  mri: "mi",
  mao: "mi",
  msa: "ms",
  may: "ms",
  mya: "my",
  bur: "my",
  nep: "ne",
  nld: "nl",
  dut: "nl",
  nya: "ny",
  oci: "oc",
  ori: "or",
  pan: "pa",
  pol: "pl",
  por: "pt",
  pus: "ps",
  ron: "ro",
  rum: "ro",
  rus: "ru",
  sin: "si",
  slk: "sk",
  slo: "sk",
  slv: "sl",
  sna: "sn",
  snd: "sd",
  som: "so",
  spa: "es",
  srp: "sr",
  swa: "sw",
  swe: "sv",
  tam: "ta",
  tel: "te",
  tgk: "tg",
  tha: "th",
  tur: "tr",
  ukr: "uk",
  urd: "ur",
  uzb: "uz",
  vie: "vi",
  wol: "wo",
  xho: "xh",
  yor: "yo",
  zul: "zu",
};

/**
 * Normalizes a raw language tag, UI alias, or ISO 639-3 code into its canonical
 * code for ElevenLabs Scribe v3 / v3 Live.
 *
 * Returns `null` for empty inputs, `"auto"`, `"os_input"`, or malformed tags
 * without a primary subtag.
 */
export const normalizeLanguageForElevenLabs = (
  value: string,
): string | null => {
  const trimmed = value.trim();
  if (!trimmed) return null;

  const normalized = trimmed.toLowerCase().replace(/_/g, "-");
  if (normalized === "auto" || normalized === "os_input" || normalized === "os-input") {
    return null;
  }

  if (normalized === "zh-hans" || normalized === "zh-hant") {
    return "zh";
  }
  if (
    normalized === "zh-yue" ||
    normalized === "yue-hant" ||
    normalized === "yue-hans"
  ) {
    return "yue";
  }

  const primary = normalized.split("-")[0]?.trim();
  if (!primary) return null;

  return ELEVENLABS_LANGUAGE_ALIASES[primary] ?? primary;
};

export const isLanguageSupportedByElevenLabs = (value: string): boolean => {
  const normalizedToken = value.trim().toLowerCase();
  if (normalizedToken === "auto" || normalizedToken === "os_input") {
    return true;
  }
  const canonical = normalizeLanguageForElevenLabs(value);
  return (
    !!canonical && ELEVENLABS_SUPPORTED_LANGUAGE_CODES.has(canonical)
  );
};

export const isSupportedElevenLabsLanguage = isLanguageSupportedByElevenLabs;

/** ElevenLabs Scribe supports these canonical languages absent from the shared UI `LANGUAGES` list. */
export const ELEVENLABS_ADDITIONAL_LANGUAGE_OPTIONS: Language[] = [
  { value: "ast", label: "Asturian" },
  { value: "ceb", label: "Cebuano" },
  { value: "ff", label: "Fulah" },
  { value: "ig", label: "Igbo" },
  { value: "kea", label: "Kabuverdianu" },
  { value: "ku", label: "Kurdish" },
  { value: "ky", label: "Kyrgyz" },
  { value: "lg", label: "Ganda" },
  { value: "luo", label: "Luo" },
  { value: "nso", label: "Northern Sotho" },
  { value: "ny", label: "Chichewa" },
  { value: "or", label: "Odia" },
  { value: "umb", label: "Umbundu" },
  { value: "wo", label: "Wolof" },
  { value: "xh", label: "Xhosa" },
  { value: "zu", label: "Zulu" },
];

export const filterLanguageOptionsForElevenLabs = (
  options: readonly Language[],
): Language[] =>
  options.filter((language) => isLanguageSupportedByElevenLabs(language.value));
