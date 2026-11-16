import { describe, expect, it } from "bun:test";
import { LANGUAGES } from "./languages";
import {
  ELEVENLABS_ADDITIONAL_LANGUAGE_OPTIONS,
  ELEVENLABS_SUPPORTED_LANGUAGE_CODES,
  filterLanguageOptionsForElevenLabs,
  isLanguageSupportedByElevenLabs,
  isSupportedElevenLabsLanguage,
  normalizeLanguageForElevenLabs,
} from "./elevenlabsLanguages";

describe("elevenlabsLanguages", () => {
  it("keeps the authoritative ElevenLabs Scribe v3 / v3 Live catalog complete", () => {
    expect(ELEVENLABS_SUPPORTED_LANGUAGE_CODES.size).toBe(101);

    for (const code of [
      "af",
      "ast",
      "ceb",
      "en",
      "ff",
      "fil",
      "he",
      "id",
      "ig",
      "jv",
      "kea",
      "ku",
      "ky",
      "lg",
      "luo",
      "no",
      "nso",
      "ny",
      "or",
      "ru",
      "umb",
      "wo",
      "xh",
      "yue",
      "zh",
      "zu",
    ]) {
      expect(ELEVENLABS_SUPPORTED_LANGUAGE_CODES.has(code)).toBe(true);
    }

    // Aliases and documented unsupported codes must not be treated as canonical catalog entries.
    for (const code of [
      "ba",
      "bo",
      "br",
      "eu",
      "fo",
      "haw",
      "ht",
      "iw",
      "jw",
      "la",
      "mg",
      "nb",
      "nn",
      "sa",
      "sq",
      "su",
      "tk",
      "tl",
      "tt",
      "xx",
      "yi",
    ]) {
      expect(ELEVENLABS_SUPPORTED_LANGUAGE_CODES.has(code)).toBe(false);
    }
  });

  it("normalizes UI aliases, BCP-47 tags, and ISO 639-3 codes to canonical codes", () => {
    expect(normalizeLanguageForElevenLabs("en-US")).toBe("en");
    expect(normalizeLanguageForElevenLabs("pt_BR")).toBe("pt");
    expect(normalizeLanguageForElevenLabs("zh-Hans")).toBe("zh");
    expect(normalizeLanguageForElevenLabs("zh_hant")).toBe("zh");
    expect(normalizeLanguageForElevenLabs("zh-yue")).toBe("yue");
    expect(normalizeLanguageForElevenLabs("yue-Hant")).toBe("yue");
    expect(normalizeLanguageForElevenLabs("nb")).toBe("no");
    expect(normalizeLanguageForElevenLabs("nn-NO")).toBe("no");
    expect(normalizeLanguageForElevenLabs("no")).toBe("no");
    expect(normalizeLanguageForElevenLabs("tl")).toBe("fil");
    expect(normalizeLanguageForElevenLabs("fil-PH")).toBe("fil");
    expect(normalizeLanguageForElevenLabs("jw")).toBe("jv");
    expect(normalizeLanguageForElevenLabs("jv-ID")).toBe("jv");
    expect(normalizeLanguageForElevenLabs("iw")).toBe("he");
    expect(normalizeLanguageForElevenLabs("he-IL")).toBe("he");
    expect(normalizeLanguageForElevenLabs("in")).toBe("id");
    expect(normalizeLanguageForElevenLabs("eng")).toBe("en");
    expect(normalizeLanguageForElevenLabs("deu")).toBe("de");
    expect(normalizeLanguageForElevenLabs("zho")).toBe("zh");
    expect(normalizeLanguageForElevenLabs("cmn")).toBe("zh");
    expect(normalizeLanguageForElevenLabs("auto")).toBeNull();
    expect(normalizeLanguageForElevenLabs("os_input")).toBeNull();
    expect(normalizeLanguageForElevenLabs("   ")).toBeNull();
    expect(normalizeLanguageForElevenLabs("-Latn")).toBeNull();
  });

  it("validates auto, os_input, UI aliases, additional codes, and unsupported languages", () => {
    for (const supported of [
      "auto",
      "os_input",
      "en",
      "en-US",
      "nb",
      "nn",
      "no",
      "tl",
      "fil",
      "jw",
      "jv",
      "iw",
      "he",
      "zh-Hans",
      "zh-Hant",
      "yue",
      "ast",
      "ceb",
      "kea",
      "luo",
      "nso",
      "umb",
      "eng",
    ]) {
      expect(isLanguageSupportedByElevenLabs(supported)).toBe(true);
      expect(isSupportedElevenLabsLanguage(supported)).toBe(true);
    }

    for (const unsupported of [
      "la",
      "eu",
      "sq",
      "ht",
      "haw",
      "fo",
      "br",
      "ba",
      "bo",
      "mg",
      "sa",
      "su",
      "tk",
      "tt",
      "yi",
      "xx-YY",
      "",
      "   ",
    ]) {
      expect(isLanguageSupportedByElevenLabs(unsupported)).toBe(false);
    }
  });

  it("filters shared LANGUAGES consistently without dropping auto/os_input or keeping unsupported codes", () => {
    const filtered = filterLanguageOptionsForElevenLabs(LANGUAGES);
    const values = filtered.map((language) => language.value);

    expect(values).toContain("auto");
    expect(values).toContain("os_input");
    expect(values).toContain("en");
    expect(values).toContain("nb");
    expect(values).toContain("nn");
    expect(values).toContain("no");
    expect(values).toContain("tl");
    expect(values).toContain("fil");
    expect(values).toContain("jw");
    expect(values).toContain("he");
    expect(values).toContain("zh");
    expect(values).toContain("yue");

    for (const unsupported of ["la", "eu", "sq", "ht", "haw", "yi", "tk"]) {
      expect(values).not.toContain(unsupported);
    }

    // Additional options only cover canonical codes missing from LANGUAGES.
    for (const extra of ELEVENLABS_ADDITIONAL_LANGUAGE_OPTIONS) {
      expect(ELEVENLABS_SUPPORTED_LANGUAGE_CODES.has(extra.value)).toBe(true);
      expect(LANGUAGES.some((item) => item.value === extra.value)).toBe(false);
    }
  });
});
