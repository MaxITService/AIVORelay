import { describe, expect, it } from "bun:test";
import {
  findNormalizedMatchRange,
  normalizeSearchText,
} from "./settingsSearchText";

describe("settings search text normalization", () => {
  it("treats common separators as spaces", () => {
    expect(normalizeSearchText("speech-only_mode/path")).toBe(
      "speech only mode path",
    );
    expect(normalizeSearchText("speech — only")).toBe("speech only");
  });

  it("keeps matching highlights aligned with the original text", () => {
    expect(
      findNormalizedMatchRange("Speech-only mode", "speech only"),
    ).toEqual([0, 11]);
    expect(findNormalizedMatchRange("Low___memory", "low memory")).toEqual([
      0, 12,
    ]);
  });
});
