import { describe, expect, it } from "bun:test";
import {
  GEMINI_LIVE_PROVIDER_LIMIT_SECONDS,
  GEMINI_LIVE_SAFE_FINALIZE_SECONDS,
  geminiFileLimitSeconds,
  validateGeminiCompatibility,
} from "./geminiConfig";
import { parseGeminiVocabulary } from "./vocabulary";

describe("Gemini vocabulary diagnostics and configuration boundaries", () => {
  it("reports structured diagnostic errors for JSON object roots and syntax errors", () => {
    const objectResult = parseGeminiVocabulary('{"terms": ["Alpha", "Beta"]}');
    expect(objectResult.format).toBe("json");
    expect(objectResult.safeToPersist).toBe(false);
    expect(objectResult.errors.some(issue => issue.code === "json_not_array")).toBe(true);

    const syntaxResult = parseGeminiVocabulary('["Alpha", "Beta"');
    expect(syntaxResult.format).toBe("json");
    expect(syntaxResult.safeToPersist).toBe(false);
    expect(syntaxResult.errors.some(issue => issue.code === "json_syntax")).toBe(true);
  });

  it("identifies CSV quote syntax violations with specific error codes and 1-based positions", () => {
    const midQuoteResult = parseGeminiVocabulary('Alpha"Beta, Gamma');
    expect(midQuoteResult.format).toBe("csv");
    expect(midQuoteResult.safeToPersist).toBe(false);
    expect(midQuoteResult.errors).toContainEqual({
      code: "csv_quote",
      message: "A quote must begin at the start of a CSV term.",
      position: 6,
    });

    const afterQuoteResult = parseGeminiVocabulary('"Alpha"Beta, Gamma');
    expect(afterQuoteResult.format).toBe("csv");
    expect(afterQuoteResult.safeToPersist).toBe(false);
    expect(afterQuoteResult.errors).toContainEqual({
      code: "csv_after_quote",
      message: "Unexpected text after a closing quote.",
      position: 8,
    });

    const unclosedQuoteResult = parseGeminiVocabulary('"UnclosedAlpha, Beta');
    expect(unclosedQuoteResult.format).toBe("csv");
    expect(unclosedQuoteResult.safeToPersist).toBe(false);
    expect(unclosedQuoteResult.errors.some(issue => issue.code === "csv_unclosed_quote")).toBe(true);
  });

  it("enforces live session duration limits and safe finalization margin invariants", () => {
    expect(GEMINI_LIVE_PROVIDER_LIMIT_SECONDS).toBe(600);
    expect(GEMINI_LIVE_SAFE_FINALIZE_SECONDS).toBe(590);
    expect(GEMINI_LIVE_PROVIDER_LIMIT_SECONDS - GEMINI_LIVE_SAFE_FINALIZE_SECONDS).toBe(10);
    expect(GEMINI_LIVE_SAFE_FINALIZE_SECONDS).toBeGreaterThan(0);
    expect(GEMINI_LIVE_PROVIDER_LIMIT_SECONDS).toBeLessThan(geminiFileLimitSeconds(true, true));
    expect(GEMINI_LIVE_PROVIDER_LIMIT_SECONDS).toBeLessThan(geminiFileLimitSeconds(false, false));
  });

  it("validates compatibility rules across non-Google routes and enforces error precedence", () => {
    expect(validateGeminiCompatibility({
      mode: "smart",
      wordTimestamps: false,
      diarization: false,
      route: "custom",
    })).toBeNull();

    expect(validateGeminiCompatibility({
      mode: "smart",
      wordTimestamps: false,
      diarization: false,
      route: "vercel",
    })).toBeNull();

    expect(validateGeminiCompatibility({
      mode: "verbatim",
      wordTimestamps: true,
      diarization: false,
      route: "custom",
    })).toBeNull();

    expect(validateGeminiCompatibility({
      mode: "verbatim",
      wordTimestamps: true,
      diarization: false,
      route: "vercel",
    })).toBeNull();

    const multiConflict = validateGeminiCompatibility({
      mode: "smart",
      wordTimestamps: true,
      diarization: true,
      route: "vercel",
    });
    expect(multiConflict).toBe("SRT and VTT require Gemini Verbatim mode because word timestamps are enabled.");
  });
});
