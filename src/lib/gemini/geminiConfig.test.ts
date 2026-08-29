import { describe, expect, it } from "bun:test";
import {
  GEMINI_LOCALES,
  geminiFileLimitSeconds,
  validateGeminiCompatibility,
} from "./geminiConfig";

describe("Gemini configuration", () => {
  it("exposes unique exact locale values", () => {
    const values = GEMINI_LOCALES.map(([, value]) => value);
    expect(new Set(values).size).toBe(values.length);
    expect(values).toContain("en-US");
    expect(values).toContain("en-GB");
    expect(values).toContain("es-419");
    expect(values).toContain("cmn-Hans-CN");
    expect(values).toContain("pa-Guru-IN");
    expect(values).not.toContain("en" as never);
  });

  it("uses the documented 60- and 30-minute file limits", () => {
    expect(geminiFileLimitSeconds(false, false)).toBe(60 * 60);
    expect(geminiFileLimitSeconds(true, false)).toBe(30 * 60);
    expect(geminiFileLimitSeconds(false, true)).toBe(30 * 60);
    expect(geminiFileLimitSeconds(true, true)).toBe(30 * 60);
  });

  it("rejects Smart timestamps and diarization", () => {
    expect(validateGeminiCompatibility({
      mode: "smart",
      wordTimestamps: true,
      diarization: false,
      route: "google",
    })).toContain("Verbatim");
    expect(validateGeminiCompatibility({
      mode: "smart",
      wordTimestamps: false,
      diarization: true,
      route: "google",
    })).toContain("Verbatim");
  });

  it("allows Google Verbatim diarization and gates Vercel diarization", () => {
    expect(validateGeminiCompatibility({
      mode: "verbatim",
      wordTimestamps: true,
      diarization: true,
      route: "google",
    })).toBeNull();
    expect(validateGeminiCompatibility({
      mode: "verbatim",
      wordTimestamps: false,
      diarization: true,
      route: "vercel",
    })).toContain("Google Direct");
  });
});
