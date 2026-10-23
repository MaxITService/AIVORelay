/**
 * Shared helpers for the frontend-driven batch queues (TTS batch and file
 * transcription). They are kept free of Tauri imports so they can be unit
 * tested directly.
 */

/**
 * Key used to detect duplicate input paths. Windows ignores letter case and
 * accepts both slash directions; elsewhere paths are compared exactly, because
 * a case-sensitive file system can hold `Song.txt` and `song.txt` side by side.
 */
export const pathKey = (path: string, windows: boolean): string =>
  windows ? outputPathKey(path) : path;

/**
 * Case- and separator-insensitive key for output paths on every platform, so
 * outputs never overwrite each other on a case-insensitive volume (the macOS
 * default). Elsewhere the only cost is an unneeded `-N` suffix.
 */
export const outputPathKey = (path: string): string =>
  path.replace(/\//g, "\\").toLowerCase();

const splitExtension = (path: string): [string, string] => {
  const separator = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  const dot = path.lastIndexOf(".");
  return dot > separator ? [path.slice(0, dot), path.slice(dot)] : [path, ""];
};

/**
 * Returns `path` with a `-N` suffix before the extension so that its key is not
 * in `taken`. Keeps the original path when it is available.
 */
export const uniqueOutputPath = (
  path: string,
  taken: ReadonlySet<string>,
): string => {
  if (!taken.has(outputPathKey(path))) return path;
  const [stem, extension] = splitExtension(path);
  for (let index = 2; ; index += 1) {
    const candidate = `${stem}-${index}${extension}`;
    if (!taken.has(outputPathKey(candidate))) {
      return candidate;
    }
  }
};

/** Returns true when the message describes a user-requested cancellation. */
export const isCancellationMessage = (value: unknown): boolean => {
  const normalized = String(value ?? "").toLowerCase();
  return normalized.includes("cancelled") || normalized.includes("canceled");
};

export const newQueueId = (): string => {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `queue-${Date.now()}-${Math.random().toString(36).slice(2)}`;
};
