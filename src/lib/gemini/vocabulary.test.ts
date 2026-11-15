import { describe, expect, it } from "bun:test";
import { parseGeminiVocabulary, validateGeminiVocabularyFile } from "./vocabulary";

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

  it("accepts Windows newline files with a UTF-8 BOM", () => {
    const result = parseGeminiVocabulary("\uFEFFGemini\r\nKubernetes\r\nBigQuery\r\n");
    expect(result.format).toBe("newline");
    expect(result.normalizedTerms).toEqual(["Gemini", "Kubernetes", "BigQuery"]);
    expect(result.errors).toEqual([]);
    expect(result.safeToPersist).toBe(true);
  });

  it("retains commas inside quoted CSV terms", () => {
    const result = parseGeminiVocabulary('"Acme, Inc.", Gemini');
    expect(result.normalizedTerms).toEqual(["Acme, Inc.", "Gemini"]);
    expect(result.safeToPersist).toBe(true);
  });

  it("reports the location of unexpected text after a closing CSV quote", () => {
    const result = parseGeminiVocabulary('Gemini,"BigQuery"x');
    expect(result.errors).toEqual([{
      code: "csv_after_quote",
      message: "Unexpected text after a closing quote.",
      position: 18,
    }]);
    expect(result.safeToPersist).toBe(false);
  });

  it("rejects a quote embedded in an unquoted CSV term", () => {
    const result = parseGeminiVocabulary('Gem"ini,BigQuery');
    expect(result.errors).toEqual([{
      code: "csv_quote",
      message: "A quote must begin at the start of a CSV term.",
      position: 4,
    }]);
    expect(result.safeToPersist).toBe(false);
  });

  it("rejects JSON objects instead of silently importing their properties", () => {
    const result = parseGeminiVocabulary('{"terms":["Gemini"]}');
    expect(result.format).toBe("json");
    expect(result.normalizedTerms).toEqual([]);
    expect(result.errors.map(issue => issue.code)).toEqual(["json_not_array"]);
    expect(result.safeToPersist).toBe(false);
  });

  it("deduplicates trimmed terms while preserving distinct capitalization", () => {
    const result = parseGeminiVocabulary('[" Gemini ", "Gemini", "gemini"]');
    expect(result.normalizedTerms).toEqual(["Gemini", "gemini"]);
    expect(result.warnings).toEqual([{
      code: "duplicate",
      message: "Duplicate term 'Gemini' was ignored; the first occurrence is retained.",
      value: "Gemini",
      position: 2,
    }]);
    expect(result.safeToPersist).toBe(true);
  });

  it("counts unique terms for the hard limit rather than duplicate input rows", () => {
    const terms = Array.from({ length: 1000 }, (_, index) => `term-${index}`);
    const result = parseGeminiVocabulary([...terms, "term-0"].join("\n"));
    expect(result.normalizedTerms).toEqual(terms);
    expect(result.errors).toEqual([]);
    expect(result.warnings.find(issue => issue.code === "duplicate")?.position).toBe(1001);
    expect(result.safeToPersist).toBe(true);
  });
});

describe("validateGeminiVocabularyFile", () => {
  it("rejects empty files even though an empty vocabulary draft is safe to persist", () => {
    expect(validateGeminiVocabularyFile("\uFEFF \r\n\t")).toBe("The file contains no terms.");
    expect(validateGeminiVocabularyFile("[]")).toBe("The file contains no terms.");
  });

  it("rejects malformed imports even when they contain usable terms", () => {
    expect(validateGeminiVocabularyFile("Gemini,,BigQuery"))
      .toBe("Term 2 is empty. Remove the malformed separator or blank line.");
  });

  it("allows imports with duplicate and recommended-limit warnings", () => {
    const terms = Array.from({ length: 101 }, (_, index) => `term-${index}`);
    expect(validateGeminiVocabularyFile([...terms, "term-0"].join("\n"))).toBeNull();
  });
});
