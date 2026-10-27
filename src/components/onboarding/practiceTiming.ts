import { catalog } from "@handy-computer/models";

/** The catalog's speed budget is meaningful only for a substantial take. */
export function isSlowLocalDictation(audioSeconds: number, waitSeconds: number): boolean {
  const { dictationSeconds, overallMaxWaitSeconds } = catalog.methodology;
  return audioSeconds >= 5 &&
    audioSeconds / Math.max(waitSeconds, 0.05) < dictationSeconds / overallMaxWaitSeconds;
}
