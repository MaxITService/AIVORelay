import { expect, test } from "bun:test";
import type { HistoryEntry } from "@/bindings";
import {
  historyDisplayText, matchesHistorySearch, matchesSearch, rawTranscriptToggle, splitByQuery,
} from "./search";

const entry = (overrides: Partial<HistoryEntry> = {}): HistoryEntry => ({
  id: 1, saved: false, file_name: "recording.wav", timestamp: 100,
  title: "Recording", transcription_text: "raw words", post_processed_text: null,
  post_process_prompt: null, post_process_requested: false, action_type: "transcribe",
  original_selection: null, ai_response: null, ...overrides,
});

test("search and highlights agree for Unicode case folding and preserve original text", () => {
  for (const [text, query] of [
    ["Встреча в ПЯТНИЦУ", "пятницу"], ["ΟΣ", "ος"], ["Kelvin", "kelvin"],
    ["😀 İstanbul I", "i"], ["ſame", "same"], ["ẞ", "ß"],
  ]) {
    const segments = splitByQuery(text, query);
    expect(matchesSearch(text, query)).toBe(true);
    expect(segments.some((segment) => segment.match)).toBe(true);
    expect(segments.map((segment) => segment.text).join("")).toBe(text);
  }
  expect(splitByQuery("😀 İstanbul I", "i")).toEqual([
    { text: "😀 İstanbul ", match: false }, { text: "I", match: true },
  ]);
});

test("highlights all occurrences including substrings inside words", () => {
  expect(splitByQuery("Milk, milky milk", "milk")).toEqual([
    { text: "Milk", match: true }, { text: ", ", match: false },
    { text: "milk", match: true }, { text: "y ", match: false },
    { text: "milk", match: true },
  ]);
});

test("SQL and regex syntax are searched literally", () => {
  for (const query of ["%", "_", "$5", "(approx.)", "[x]", "a+b", "\\"]) {
    expect(matchesSearch(`before ${query} after`, query)).toBe(true);
    expect(splitByQuery(`before ${query} after`, query)).toEqual([
      { text: "before ", match: false }, { text: query, match: true },
      { text: " after", match: false },
    ]);
  }
  expect(matchesSearch("anything", "%")).toBe(false);
});

test("blank and missing queries do not change the text", () => {
  expect(matchesSearch("anything", "  ")).toBe(true);
  expect(splitByQuery("unchanged", "  ")).toEqual([{ text: "unchanged", match: false }]);
  expect(splitByQuery("unchanged", "missing")).toEqual([{ text: "unchanged", match: false }]);
});

test("dictation search follows the displayed processed text including empty overrides", () => {
  const processed = entry({ post_processed_text: "Polished words" });
  expect(historyDisplayText(processed)).toBe("Polished words");
  expect(matchesHistorySearch(processed, "polished")).toBe(true);
  expect(matchesHistorySearch(processed, "raw")).toBe(false);
  expect(matchesHistorySearch(entry(), "raw")).toBe(true);
  expect(matchesHistorySearch(entry({ post_processed_text: "" }), "raw")).toBe(false);
});

test("the raw transcript toggle tells unchanged processing apart from none", () => {
  expect(rawTranscriptToggle(entry())).toBe("none");
  expect(rawTranscriptToggle(entry({ post_processed_text: "raw words" }))).toBe("unchanged");
  expect(rawTranscriptToggle(entry({ post_processed_text: "Polished words" }))).toBe("available");
  expect(rawTranscriptToggle(entry({ post_processed_text: "" }))).toBe("available");
  expect(rawTranscriptToggle(entry({ action_type: "ai_replace", post_processed_text: "raw words" })))
    .toBe("none");
});

test("AI Replace searches instructions, full selection, and response", () => {
  const ai = entry({ action_type: "ai_replace", transcription_text: "Fix grammar",
    original_selection: `${"prefix ".repeat(30)}Original ending`, ai_response: "AI result",
    post_processed_text: "hidden", post_process_prompt: "secret prompt" });
  for (const query of ["grammar", "original ending", "ai result"]) {
    expect(matchesHistorySearch(ai, query)).toBe(true);
  }
  for (const query of ["hidden", "secret", "recording"]) {
    expect(matchesHistorySearch(ai, query)).toBe(false);
  }
  expect(historyDisplayText(ai)).toBe("AI result");
  expect(matchesHistorySearch(entry({ action_type: "ai_replace", ai_response: null }), "raw")).toBe(true);
});
