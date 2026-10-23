import { describe, expect, test } from "bun:test";
import { isCancellationMessage, outputPathKey, pathKey, uniqueOutputPath } from "./batchQueueUtils";

describe("batch queue paths", () => {
  test("Windows path keys ignore slash direction and letter case", () => {
    expect(pathKey("C:/Audio/VOICE.WAV", true)).toBe(pathKey("c:\\audio\\voice.wav", true));
    expect(pathKey("//Server/Share/файл.wav", true)).toBe(pathKey("\\\\server\\share\\ФАЙЛ.WAV", true));
    expect(pathKey("C:/Audio/one.wav", true)).not.toBe(pathKey("C:/Audio/two.wav", true));
  });

  test("macOS and Linux path keys keep letter case and backslashes", () => {
    expect(pathKey("/audio/Song.wav", false)).not.toBe(pathKey("/audio/song.wav", false));
    expect(pathKey("/audio/a\\b.wav", false)).not.toBe(pathKey("/audio/a/b.wav", false));
    expect(pathKey("/audio/song.wav", false)).toBe(pathKey("/audio/song.wav", false));
  });

  test("available outputs keep their original name", () => {
    expect(uniqueOutputPath("C:/out/read.mp3", new Set())).toBe("C:/out/read.mp3");
  });

  test("output suffixes avoid occupied names regardless of case or separators", () => {
    const taken = new Set(["c:\\out\\read.mp3", "C:/out/read-2.MP3", "C:/out/read-3.mp3"].map(outputPathKey));
    expect(uniqueOutputPath("C:/out/read.mp3", taken)).toBe("C:/out/read-4.mp3");
    expect(taken.size).toBe(3);
  });

  test("extensionless outputs and dots in parent directories are preserved", () => {
    expect(uniqueOutputPath("C:/out.v1/read", new Set([outputPathKey("C:/out.v1/read")]))).toBe("C:/out.v1/read-2");
    expect(uniqueOutputPath("C:\\out\\read.part.wav", new Set([outputPathKey("C:\\out\\read.part.wav")]))).toBe("C:\\out\\read.part-2.wav");
  });

  test("a crowded output folder never falls back to an occupied path", () => {
    const taken = new Set([outputPathKey("out.wav")]);
    for (let index = 2; index <= 10_000; index += 1) taken.add(outputPathKey(`out-${index}.wav`));
    expect(uniqueOutputPath("out.wav", taken)).toBe("out-10001.wav");
  });
});

describe("batch cancellation messages", () => {
  test("recognizes both spellings, mixed case, and Error objects", () => {
    expect(isCancellationMessage("File transcription was cancelled")).toBe(true);
    expect(isCancellationMessage(new Error("CANCELED by user"))).toBe(true);
  });

  test("ordinary failures and missing messages are not cancellations", () => {
    for (const error of [null, undefined, "", "Quota exceeded", new Error("Disk full")]) {
      expect(isCancellationMessage(error)).toBe(false);
    }
  });
});
