import { expect, test } from "bun:test";
import { languageIdentity } from "@handy-computer/models";

// Mirrors `language_aliases_match_the_frontend_identity` in
// src-tauri/src/managers/model.rs: the spoken-language picker hides what this
// calls a duplicate, and saving drops what the backend calls one.
test("language identities group the same codes as the backend", () => {
  for (const [left, right] of [["no", "nb"], ["tl", "fil"], ["he", "iw"], ["jv", "jw"], ["pt", "pt-BR"], ["zh", "zh-Hant"]]) {
    expect(languageIdentity(left)).toBe(languageIdentity(right));
  }
  const distinct = ["zh", "yue", "pt", "en", "no", "he", "tl", "jv"];
  expect(new Set(distinct.map(languageIdentity)).size).toBe(distinct.length);
});
