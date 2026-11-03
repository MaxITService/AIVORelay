import { expect, test } from "bun:test";
import { detectorDownloadView, detectorProgress, LANGUAGE_DETECTOR_ID } from "./languageDetectorDownload";

const detector = LANGUAGE_DETECTOR_ID;

test("detector progress follows only the detector's own events", () => {
  expect(detectorProgress(null, { kind: "progress", modelId: detector, percentage: 40 })).toBe(40);
  expect(detectorProgress(40, { kind: "progress", modelId: "whisper-small", percentage: 90 })).toBe(40);
  expect(detectorProgress(40, { kind: "failed", modelId: "whisper-small" })).toBe(40);
  expect(detectorProgress(40, { kind: "complete", modelId: detector })).toBeNull();
});

test("a cancelled or failed detector download stops showing progress", () => {
  expect(detectorProgress(40, { kind: "cancelled", modelId: detector })).toBeNull();
  expect(detectorProgress(40, { kind: "failed", modelId: detector })).toBeNull();
});

test("a detector download started by a model shows progress and blocks a second download", () => {
  // Selecting a model that needs the detector downloads it without this page.
  expect(detectorDownloadView(false, false, 40)).toEqual({ showDownload: true, downloadDisabled: true, showProgress: true });
});

test("the detector row is idle without a download and hidden once downloaded", () => {
  expect(detectorDownloadView(false, false, null)).toEqual({ showDownload: true, downloadDisabled: false, showProgress: false });
  expect(detectorDownloadView(false, true, 0)).toEqual({ showDownload: true, downloadDisabled: true, showProgress: true });
  expect(detectorDownloadView(true, false, null)).toEqual({ showDownload: false, downloadDisabled: false, showProgress: false });
});
