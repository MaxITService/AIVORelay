import { describe, expect, test } from "bun:test";
import { formatDownloadTime, remainingDownloadSeconds, sampleDownload } from "./downloadStats";

describe("model download estimates", () => {
  test("excludes resumed bytes and estimates from newly transferred data", () => {
    const first = sampleDownload(undefined, 100 * 1024 * 1024, 0);
    expect(first.speed).toBe(0);
    const next = sampleDownload(first, 102 * 1024 * 1024, 1000);
    expect(next.speed).toBe(2);
    expect(remainingDownloadSeconds(122 * 1024 * 1024, 102 * 1024 * 1024, next)).toBe(10);
  });

  test("decays and clears a stalled transfer rather than keeping a stale ETA", () => {
    const first = sampleDownload(undefined, 0, 0);
    const flowing = sampleDownload(first, 1024 * 1024, 1000);
    const slower = sampleDownload(flowing, flowing.totalDownloaded, 2000);
    expect(slower.speed).toBeLessThan(flowing.speed);
    const stalled = sampleDownload(slower, slower.totalDownloaded, 6000);
    expect(stalled.speed).toBe(0);
    expect(remainingDownloadSeconds(10 * 1024 * 1024, stalled.totalDownloaded, stalled)).toBeUndefined();
  });

  test("resets the baseline after a restarted transfer", () => {
    const flowing = sampleDownload(sampleDownload(undefined, 100, 0), 200, 1000);
    expect(sampleDownload(flowing, 0, 2000).speed).toBe(0);
  });

  test("includes a recent partial sample when the timer ticks", () => {
    const first = sampleDownload(undefined, 0, 0);
    const partial = sampleDownload(first, 1024 * 1024, 200);
    const tick = sampleDownload(partial, partial.latestDownloaded, 1000);
    expect(tick.speed).toBe(1);
    expect(tick.lastProgress).toBe(200);
  });

  test("omits estimates for unknown, completed or extremely slow transfers", () => {
    const stats = { lastUpdate: 0, lastProgress: 0, totalDownloaded: 100, latestDownloaded: 100, speed: 0.01 };
    expect(remainingDownloadSeconds(0, 100, stats)).toBeUndefined();
    expect(remainingDownloadSeconds(100, 100, { ...stats, speed: 1 })).toBeUndefined();
    expect(remainingDownloadSeconds(1000, 100, stats)).toBeUndefined();
    expect(formatDownloadTime(65)).toBe("1:05");
    expect(formatDownloadTime(3665)).toBe("1:01:05");
  });
});
