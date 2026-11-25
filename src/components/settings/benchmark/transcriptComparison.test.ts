import { describe, expect, test } from "bun:test";
import { compareTranscripts } from "./transcriptComparison";

describe("compareTranscripts", () => {
  test("ignores case and punctuation while preserving original text", () => {
    const result = compareTranscripts("Встреча в пять.", "встреча в пять!");
    expect(result?.wordErrorRate).toBe(0);
    expect(result?.reference).toEqual([{ text: "Встреча в пять." }]);
    expect(result?.recognized).toEqual([{ text: "встреча в пять!" }]);
  });

  test("counts substitutions, omissions, and additions", () => {
    const result = compareTranscripts(
      "keep omitted anchor old end",
      "keep anchor new end extra",
    );
    expect(result?.counts).toEqual({ substitutions: 1, omissions: 1, additions: 1 });
    expect(result?.wordErrorRate).toBe(0.6);
  });

  test("groups a completely different transcript into readable highlights", () => {
    const result = compareTranscripts("встреча в пять", "green trees bloom");
    expect(result?.wordErrorRate).toBe(1);
    expect(result?.reference).toEqual([{ text: "встреча в пять", edit: "substitutions" }]);
    expect(result?.recognized).toEqual([{ text: "green trees bloom", edit: "substitutions" }]);
  });

  test("counts every reference word as omitted if no speech is recognized", () => {
    const result = compareTranscripts("Встреча в пять", "");
    expect(result?.counts.omissions).toBe(3);
    expect(result?.wordErrorRate).toBe(1);
    expect(result?.recognized).toEqual([]);
  });

  test("skips alignment when the word matrix would exceed the work limit", () => {
    expect(compareTranscripts("alpha ".repeat(1_000), "beta ".repeat(1_000))).toBeNull();
  });
});
