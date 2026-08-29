export type GeminiTranscriptionMode = "smart" | "verbatim";
export type GeminiLanguageSelection = "auto" | "os_input" | GeminiExactLocale;

export const GEMINI_VOCABULARY_RECOMMENDED_MAX = 100;
export const GEMINI_VOCABULARY_HARD_MAX = 1000;
export const GEMINI_LIVE_PROVIDER_LIMIT_SECONDS = 10 * 60;
export const GEMINI_LIVE_SAFE_FINALIZE_SECONDS = 9 * 60 + 50;

export const GEMINI_LOCALES = [
  ["Afrikaans", "af-ZA"], ["Amharic", "am-ET"], ["Arabic (Egypt)", "ar-EG"],
  ["Armenian", "hy-AM"], ["Assamese", "as-IN"], ["Azerbaijani", "az-AZ"],
  ["Belarusian", "be-BY"], ["Bengali (Bangladesh)", "bn-BD"], ["Bengali (India)", "bn-IN"],
  ["Bosnian", "bs-BA"], ["Bulgarian", "bg-BG"], ["Bulgarian (Aromanian)", "rup-BG"],
  ["Burmese", "my-MM"], ["Cantonese (Traditional)", "yue-Hant-HK"], ["Catalan", "ca-ES"],
  ["Cebuano", "ceb"], ["Central Khmer", "km-KH"], ["Croatian", "hr-HR"],
  ["Czech", "cs-CZ"], ["Danish", "da-DK"], ["Dutch", "nl-NL"],
  ["English (Great Britain)", "en-GB"], ["English (India)", "en-IN"], ["English (United States)", "en-US"],
  ["Estonian", "et-EE"], ["Farsi", "fa-IR"], ["Filipino", "fil-PH"],
  ["Finnish", "fi-FI"], ["French", "fr-FR"], ["Galician", "gl-ES"],
  ["Georgian", "ka-GE"], ["German", "de-DE"], ["Greek", "el-GR"],
  ["Gujarati", "gu-IN"], ["Hausa", "ha-NG"], ["Hebrew", "he-IL"],
  ["Hindi", "hi-IN"], ["Hungarian", "hu-HU"], ["Icelandic", "is-IS"],
  ["Indonesian", "id-ID"], ["Italian", "it-IT"], ["Japanese", "ja-JP"],
  ["Javanese", "jv-ID"], ["Kabuverdianu", "kea-CV"], ["Kannada", "kn-IN"],
  ["Kazakh", "kk-KZ"], ["Korean", "ko-KR"], ["Kyrgyz", "ky-KG"],
  ["Latvian", "lv-LV"], ["Lingala", "ln-CD"], ["Lithuanian", "lt-LT"],
  ["Macedonian", "mk-MK"], ["Malay", "ms-MY"], ["Malayalam", "ml-IN"],
  ["Maltese", "mt-MT"], ["Mandarin Chinese (Simplified)", "cmn-Hans-CN"], ["Marathi", "mr-IN"],
  ["Mongolian", "mn-MN"], ["Nepali", "ne-NP"], ["Norwegian", "nb-NO"],
  ["Oriya", "or-IN"], ["Polish", "pl-PL"], ["Portuguese (Brazil)", "pt-BR"],
  ["Portuguese (Portugal)", "pt-PT"], ["Punjabi", "pa-IN"], ["Punjabi (Gurmukhi script)", "pa-Guru-IN"],
  ["Romanian", "ro-RO"], ["Russian", "ru-RU"], ["Serbian", "sr-RS"],
  ["Sindhi (Arabic script)", "sd-Arab-IN"], ["Slovak", "sk-SK"], ["Slovenian", "sl-SI"],
  ["Spanish (Latin America)", "es-419"], ["Spanish (United States)", "es-US"], ["Swahili (Kenya)", "sw-KE"],
  ["Swedish", "sv-SE"], ["Tajik", "tg-TJ"], ["Telugu", "te-IN"],
  ["Thai", "th-TH"], ["Turkish", "tr-TR"], ["Ukrainian", "uk-UA"],
  ["Uzbek", "uz-UZ"], ["Vietnamese", "vi-VN"],
] as const;

export type GeminiExactLocale = (typeof GEMINI_LOCALES)[number][1];

export function geminiFileLimitSeconds(wordTimestamps: boolean, diarization: boolean): number {
  return wordTimestamps || diarization ? 30 * 60 : 60 * 60;
}

export function validateGeminiCompatibility(input: {
  mode: GeminiTranscriptionMode;
  wordTimestamps: boolean;
  diarization: boolean;
  route: string;
}): string | null {
  if (input.mode === "smart" && input.wordTimestamps) {
    return "SRT and VTT require Gemini Verbatim mode because word timestamps are enabled.";
  }
  if (input.mode === "smart" && input.diarization) {
    return "Gemini speaker diarization requires Verbatim mode.";
  }
  if (input.diarization && input.route !== "google") {
    return "Gemini speaker diarization is currently available only through Google Direct.";
  }
  return null;
}
