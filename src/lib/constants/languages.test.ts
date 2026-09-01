import { expect, it } from "bun:test";
import {
  LANGUAGES,
  MODEL_CAPABILITY_LANGUAGES,
  recognitionLanguage,
  supportsLanguageCode,
} from "./languages";
import {
  isLanguageSupportedBySoniox,
  parseAndNormalizeSonioxLanguageHints,
} from "./sonioxLanguages";

it("collapses regional subtags, script suffixes, and canonical aliases to stable recognition codes", () => {
  expect(recognitionLanguage("en")).toBe("en");
  expect(recognitionLanguage("en-US")).toBe("en");
  expect(recognitionLanguage("en_GB")).toBe("en");
  expect(recognitionLanguage("nb")).toBe("no");
  expect(recognitionLanguage("nb-NO")).toBe("no");
  expect(recognitionLanguage("nb_NO")).toBe("no");
  expect(recognitionLanguage("fil")).toBe("tl");
  expect(recognitionLanguage("fil-PH")).toBe("tl");
  expect(recognitionLanguage("fil_PH")).toBe("tl");
  expect(recognitionLanguage("zh-Hans")).toBe("zh");
  expect(recognitionLanguage("zh-Hant")).toBe("zh");
  expect(recognitionLanguage("zh-Hans-CN")).toBe("zh");
});

it("matches language support across dialect aliases and regional variants while filtering pseudo-languages", () => {
  expect(supportsLanguageCode(["en", "fr"], "en")).toBe(true);
  expect(supportsLanguageCode(["en-US"], "en-GB")).toBe(true);
  expect(supportsLanguageCode(["no"], "nb-NO")).toBe(true);
  expect(supportsLanguageCode(["nb-NO"], "no")).toBe(true);
  expect(supportsLanguageCode(["tl"], "fil-PH")).toBe(true);
  expect(supportsLanguageCode(["fil-PH"], "tl")).toBe(true);
  expect(supportsLanguageCode(["en", "fr"], "de")).toBe(false);
  expect(supportsLanguageCode([], "en")).toBe(false);

  const capabilityValues = MODEL_CAPABILITY_LANGUAGES.map((lang) => lang.value);
  expect(capabilityValues).not.toContain("auto");
  expect(capabilityValues).not.toContain("os_input");
  expect(capabilityValues).toContain("en");
  expect(capabilityValues).toContain("nb");
  expect(capabilityValues.length).toBe(LANGUAGES.length - 2);
});

it("validates Soniox language support for pseudo-languages and supported locale variants while rejecting unsupported codes", () => {
  expect(isLanguageSupportedBySoniox("auto")).toBe(true);
  expect(isLanguageSupportedBySoniox("os_input")).toBe(true);

  expect(isLanguageSupportedBySoniox("en")).toBe(true);
  expect(isLanguageSupportedBySoniox("en-US")).toBe(true);
  expect(isLanguageSupportedBySoniox("es_MX")).toBe(true);
  expect(isLanguageSupportedBySoniox("zh-Hans")).toBe(true);
  expect(isLanguageSupportedBySoniox("zh_Hant")).toBe(true);

  expect(isLanguageSupportedBySoniox("la")).toBe(false);
  expect(isLanguageSupportedBySoniox("haw")).toBe(false);
  expect(isLanguageSupportedBySoniox("invalid_lang")).toBe(false);
  expect(isLanguageSupportedBySoniox("")).toBe(false);
  expect(isLanguageSupportedBySoniox("   ")).toBe(false);
});

it("parses comma-delimited language hints, deduplicates normalized codes, and isolates rejected entries", () => {
  expect(parseAndNormalizeSonioxLanguageHints("")).toEqual({
    normalized: [],
    rejected: [],
  });
  expect(parseAndNormalizeSonioxLanguageHints("  ,  ,  ")).toEqual({
    normalized: [],
    rejected: [],
  });

  const duplicatesResult = parseAndNormalizeSonioxLanguageHints(
    "en-US, EN, en_GB, zh-Hans, ZH_HANT, de",
  );
  expect(duplicatesResult).toEqual({
    normalized: ["en", "zh", "de"],
    rejected: [],
  });

  const mixedResult = parseAndNormalizeSonioxLanguageHints(
    "  en_US , invalid-lang , es , la , zh-Hans , unknown_123  ",
  );
  expect(mixedResult).toEqual({
    normalized: ["en", "es", "zh"],
    rejected: ["invalid-lang", "la", "unknown_123"],
  });
});
