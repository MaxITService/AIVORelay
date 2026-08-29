import { describe, expect, it } from "bun:test";
import { parseGeminiVocabulary } from "./vocabulary";

describe("parseGeminiVocabulary", () => {
  it.each([
    ["newline", "Gemini\nKubernetes\nBigQuery"],
    ["csv", "Gemini, Kubernetes, BigQuery"],
    ["csv", '"Gemini", "Kubernetes", "BigQuery"'],
    ["json", '["Gemini", "Kubernetes", "BigQuery"]'],
  ])("accepts %s input", (format, input) => {
    const result = parseGeminiVocabulary(input);
    expect(result.format).toBe(format);
    expect(result.normalizedTerms).toEqual(["Gemini", "Kubernetes", "BigQuery"]);
    expect(result.safeToPersist).toBe(true);
  });

  it("preserves Unicode, internal spaces, punctuation, capitalization, and escaped CSV quotes", () => {
    expect(parseGeminiVocabulary('"Aivo Relay", "ЖФК", "BigQuery!", "say ""hello"""').normalizedTerms)
      .toEqual(["Aivo Relay", "ЖФК", "BigQuery!", 'say "hello"']);
  });

  it("ignores surrounding whitespace while retaining empty internal newline entries as errors", () => {
    expect(parseGeminiVocabulary("  Gemini\nBigQuery\n  ").normalizedTerms)
      .toEqual(["Gemini", "BigQuery"]);
    expect(parseGeminiVocabulary("Gemini\n\nBigQuery").safeToPersist).toBe(false);
  });

  it.each([
    ["malformed CSV", '"Gemini, Kubernetes'],
    ["malformed JSON", '["Gemini",]'],
    ["mixed formats", "Gemini, Kubernetes\nBigQuery"],
    ["non-string JSON", '["Gemini", 3, null]'],
    ["empty CSV item", "Gemini,,BigQuery"],
  ])("blocks %s", (_name, input) => expect(parseGeminiVocabulary(input).safeToPersist).toBe(false));

  it("retains the first duplicate deterministically and reports it", () => {
    const result = parseGeminiVocabulary("Gemini\nGemini");
    expect(result.normalizedTerms).toEqual(["Gemini"]);
    expect(result.warnings.some(issue => issue.code === "duplicate")).toBe(true);
  });

  it.each([[100, true], [101, true], [1000, true], [1001, false]])(
    "handles the %i-term boundary",
    (count, safe) => expect(parseGeminiVocabulary(Array.from({ length: count }, (_, i) => `term-${i}`).join("\n")).safeToPersist).toBe(safe),
  );

  it("shows only a recommendation from 101 through 1,000 terms", () => {
    const hundred = parseGeminiVocabulary(Array.from({ length: 100 }, (_, i) => `term-${i}`).join("\n"));
    const hundredOne = parseGeminiVocabulary(Array.from({ length: 101 }, (_, i) => `term-${i}`).join("\n"));
    const thousand = parseGeminiVocabulary(Array.from({ length: 1000 }, (_, i) => `term-${i}`).join("\n"));
    expect(hundred.warnings.some(issue => issue.code === "recommended_limit")).toBe(false);
    expect(hundredOne.warnings.some(issue => issue.code === "recommended_limit")).toBe(true);
    expect(thousand.safeToPersist).toBe(true);
  });

  it("keeps malformed drafts unsafe even when some terms parsed successfully", () => {
    const result = parseGeminiVocabulary("Gemini,,BigQuery");
    expect(result.normalizedTerms).toEqual(["Gemini", "BigQuery"]);
    expect(result.safeToPersist).toBe(false);
  });
});
