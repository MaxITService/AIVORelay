import { invoke } from "@tauri-apps/api/core";

export type LocalModelStorageEntry = {
  id: string;
  name: string;
  kind: "stt" | "tts";
  bytes: number;
  shared_cache_bytes: number;
  blocked_reason: string | null;
};

export type LocalModelStorageSnapshot = {
  entries: LocalModelStorageEntry[];
  total_bytes: number;
  disk_free_bytes: number | null;
};

export type ModelCleanupOutcome = {
  key: string;
  error: string | null;
};

export const modelStorageKey = (entry: Pick<LocalModelStorageEntry, "kind" | "id">) =>
  `${entry.kind}:${entry.id}`;

export const getLocalModelStorage = () =>
  invoke<LocalModelStorageSnapshot>("get_local_model_storage");

export const cleanupLocalModel = (entry: LocalModelStorageEntry) =>
  invoke<void>("cleanup_local_model", { kind: entry.kind, modelId: entry.id });

// Keep removal sequential: STT unloads share one loading lane, and TTS workers
// share one foreground lane. One failure must not hide the remaining results.
export const cleanSelectedLocalModels = async (
  entries: LocalModelStorageEntry[],
  remove = cleanupLocalModel,
  onProgress: (key: string, outcome?: ModelCleanupOutcome) => void = () => {},
): Promise<ModelCleanupOutcome[]> => {
  const outcomes: ModelCleanupOutcome[] = [];
  const seen = new Set<string>();
  for (const entry of entries) {
    const key = modelStorageKey(entry);
    if (seen.has(key) || entry.blocked_reason) continue;
    seen.add(key);
    onProgress(key);
    let error: string | null = null;
    try {
      await remove(entry);
    } catch (failure) {
      error = String(failure);
    }
    const outcome = { key, error };
    outcomes.push(outcome);
    onProgress(key, outcome);
  }
  return outcomes;
};

export const removedModelBytes = (
  before: LocalModelStorageSnapshot,
  after: LocalModelStorageSnapshot,
  attemptedKeys: Set<string>,
) => before.entries.reduce((total, entry) => {
  const key = modelStorageKey(entry);
  if (!attemptedKeys.has(key)) return total;
  const remaining = after.entries.find((candidate) => modelStorageKey(candidate) === key);
  if (remaining?.blocked_reason) return total;
  return total + Math.max(0, entry.bytes - (remaining?.bytes ?? 0));
}, 0);

export const formatStorageBytes = (bytes: number): string => {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  const exponent = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: exponent > 0 ? 1 : 0 }).format(bytes / 1024 ** exponent)} ${units[exponent]}`;
};
