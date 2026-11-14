import { expect, it } from "bun:test";
import {
  isLanguageSupportedByReson8,
  normalizeLanguageForReson8,
  RESON8_RUMBLE_2_SUPPORTED_LANGUAGE_CODES,
} from "./reson8Languages";

it("keeps the authoritative Rumble-2 catalog complete", () => {
  expect(RESON8_RUMBLE_2_SUPPORTED_LANGUAGE_CODES.size).toBe(56);
  for (const code of ["af", "ceb", "fy", "nb", "fil", "jv", "yue", "zh"]) {
    expect(RESON8_RUMBLE_2_SUPPORTED_LANGUAGE_CODES.has(code)).toBe(true);
  }
  for (const code of ["he", "iw", "la", "haw", "no", "tl", "jw"]) {
    expect(RESON8_RUMBLE_2_SUPPORTED_LANGUAGE_CODES.has(code)).toBe(false);
  }
});

it("maps recognition identities to concrete Rumble-2 codes", () => {
  expect(normalizeLanguageForReson8("no")).toBe("nb");
  expect(normalizeLanguageForReson8("nb-NO")).toBe("nb");
  expect(normalizeLanguageForReson8("tl")).toBe("fil");
  expect(normalizeLanguageForReson8("fil-PH")).toBe("fil");
  expect(normalizeLanguageForReson8("jw")).toBe("jv");
  expect(normalizeLanguageForReson8("jv-ID")).toBe("jv");
  expect(normalizeLanguageForReson8("zh-Hans")).toBe("zh");
  expect(normalizeLanguageForReson8("ZH_hant")).toBe("zh");
  expect(normalizeLanguageForReson8(" EN-us ")).toBe("en");
  expect(normalizeLanguageForReson8("   ")).toBeNull();
});

it("validates auto, OS input, aliases, regions, and unsupported languages", () => {
  for (const value of [
    "auto",
    "os_input",
    "en",
    "en-US",
    "nb-NO",
    "no",
    "fil",
    "tl",
    "jv",
    "jw",
    "zh-Hans",
    "zh-Hant",
    "ceb",
    "fy",
  ]) {
    expect(isLanguageSupportedByReson8(value)).toBe(true);
  }

  for (const value of ["he", "iw-IL", "la", "haw", "xx-YY", "", "   "]) {
    expect(isLanguageSupportedByReson8(value)).toBe(false);
  }
});
