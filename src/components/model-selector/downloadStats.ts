export interface DownloadStats {
  lastUpdate: number;
  lastProgress: number;
  totalDownloaded: number;
  latestDownloaded: number;
  speed: number;
}

const MIB = 1024 * 1024;
const STALL_MS = 5_000;

/** Samples only transferred bytes, excluding the initial resume offset. */
export function sampleDownload(stats: DownloadStats | undefined, downloaded: number, now: number): DownloadStats {
  if (!stats || downloaded < stats.latestDownloaded || now < stats.lastUpdate) {
    return { lastUpdate: now, lastProgress: now, totalDownloaded: downloaded, latestDownloaded: downloaded, speed: 0 };
  }

  const elapsed = (now - stats.lastUpdate) / 1000;
  const transferred = downloaded - stats.totalDownloaded;
  const lastProgress = downloaded > stats.latestDownloaded ? now : stats.lastProgress;
  if (elapsed < 0.5) return { ...stats, lastProgress, latestDownloaded: downloaded };

  const instantaneous = transferred / MIB / elapsed;
  const speed = now - lastProgress >= STALL_MS ? 0 :
    stats.speed > 0 ? stats.speed * 0.8 + instantaneous * 0.2 : instantaneous;
  return { lastUpdate: now, lastProgress, totalDownloaded: downloaded, latestDownloaded: downloaded, speed };
}

/** Returns no estimate for unknown size, a stalled transfer, or an implausible rate. */
export function remainingDownloadSeconds(total: number, downloaded: number, stats?: DownloadStats): number | undefined {
  if (!stats || stats.speed < 0.05 || total <= 0 || downloaded >= total) return undefined;
  const seconds = (total - downloaded) / (stats.speed * MIB);
  return Number.isFinite(seconds) && seconds > 0 && seconds <= 86_400 ? Math.ceil(seconds) : undefined;
}

export function formatDownloadTime(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const remainder = Math.floor(seconds % 60);
  return hours > 0
    ? `${hours}:${String(minutes).padStart(2, "0")}:${String(remainder).padStart(2, "0")}`
    : `${minutes}:${String(remainder).padStart(2, "0")}`;
}
