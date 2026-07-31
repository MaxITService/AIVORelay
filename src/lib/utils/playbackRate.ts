export const PLAYBACK_RATES = [0.75, 1, 1.25, 1.5, 1.75, 2, 2.5, 3] as const;

export type PlaybackRate = (typeof PLAYBACK_RATES)[number];

export const DEFAULT_PLAYBACK_RATE: PlaybackRate = 1;

export function nextPlaybackRate(current: PlaybackRate): PlaybackRate {
  const currentIndex = PLAYBACK_RATES.indexOf(current);
  return PLAYBACK_RATES[(currentIndex + 1) % PLAYBACK_RATES.length];
}

export function applyPlaybackRate(audio: HTMLAudioElement, rate: number) {
  const normalized = Number.isFinite(rate)
    ? Math.min(16, Math.max(0.0625, rate))
    : 1;
  audio.defaultPlaybackRate = normalized;
  audio.playbackRate = normalized;
  audio.preservesPitch = true;
}

export function formatPlaybackRate(rate: PlaybackRate): string {
  return `${rate}×`;
}
