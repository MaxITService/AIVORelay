import { expect, test } from "bun:test";
import { missingModelLanguages, suggestedModelLanguages } from "./modelLanguages";

test("suggestions normalize locales, deduplicate aliases, and ignore unknown languages", () => {
  expect(suggestedModelLanguages(["zh-TW", "zh_CN", "iw", "he", "zz-ZZ", "en-US"])).toEqual(["zh", "he", "en"]);
  expect(suggestedModelLanguages([])).toEqual([]);
});

test("model language coverage respects regional codes and language aliases", () => {
  expect(missingModelLanguages(["en-US", "zh-Hant", "iw", "jw"], ["en", "zh", "he", "jv", "ru"])).toEqual(["ru"]);
  expect(missingModelLanguages([], ["en"])).toEqual(["en"]);
});
