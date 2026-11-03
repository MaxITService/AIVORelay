export const LANGUAGE_DETECTOR_ID = "language-detector";

export type DetectorDownloadEvent =
  | { kind: "progress"; modelId: string; percentage: number }
  | { kind: "complete" | "cancelled" | "failed"; modelId: string };

/** The detector's download percentage after `event`, or null when none is running. */
export function detectorProgress(current: number | null, event: DetectorDownloadEvent): number | null {
  if (event.modelId !== LANGUAGE_DETECTOR_ID) return current;
  return event.kind === "progress" ? event.percentage : null;
}

/** What the detector row shows; `progress` also covers downloads a model started. */
export function detectorDownloadView(downloaded: boolean, busy: boolean, progress: number | null) {
  return {
    showDownload: !downloaded,
    downloadDisabled: busy || progress !== null,
    showProgress: progress !== null,
  };
}
