import { describe, expect, it } from "bun:test";
import { isSlowLocalDictation } from "./practiceTiming";

describe("local dictation timing", () => {
  it("only judges a substantial take against the catalog speed budget", () => {
    expect(isSlowLocalDictation(4.9, 10)).toBe(false);
    expect(isSlowLocalDictation(30, 5)).toBe(false);
    expect(isSlowLocalDictation(30, 6)).toBe(true);
    expect(isSlowLocalDictation(10, 0)).toBe(false);
  });
});
