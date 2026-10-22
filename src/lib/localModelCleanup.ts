import { invoke } from "@tauri-apps/api/core";

export type LocalModelStorageEntry = {
  id: string;
  name: string;
  kind: "stt" | "tts";
  bytes: number;
  bytes_known: boolean;
  shared_cache_bytes: number;
  blocked_reason: string | null;
};

export type LocalModelStorageSnapshot = {
  entries: LocalModelStorageEntry[];
  total_bytes: number | null;
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

// Rows sharing files each count them as kept; together they free them once.
export const estimateLocalModelCleanup = (entries: Pick<LocalModelStorageEntry, "kind" | "id">[]) =>
  invoke<number>("estimate_local_model_cleanup", {
    sttModelIds: entries.filter(entry => entry.kind === "stt").map(entry => entry.id),
    ttsModelIds: entries.filter(entry => entry.kind === "tts").map(entry => entry.id),
  });

export const cleanupLocalModel = (entry: LocalModelStorageEntry, selectedSttModelIds = [entry.id]) =>
  invoke<void>("cleanup_local_model", { kind: entry.kind, modelId: entry.id, selectedSttModelIds });

// Keep removal sequential: STT unloads share one loading lane, and TTS workers
// share one foreground lane. One failure must not hide the remaining results.
export const cleanSelectedLocalModels = async (
  entries: LocalModelStorageEntry[],
  remove?: (entry: LocalModelStorageEntry) => Promise<void>,
  onProgress: (key: string, outcome?: ModelCleanupOutcome) => void = () => {},
): Promise<ModelCleanupOutcome[]> => {
  const outcomes: ModelCleanupOutcome[] = [];
  const selectedSttModelIds = entries.filter(entry => entry.kind === "stt" && !entry.blocked_reason).map(entry => entry.id);
  const seen = new Set<string>();
  for (const entry of entries) {
    const key = modelStorageKey(entry);
    if (seen.has(key) || entry.blocked_reason) continue;
    seen.add(key);
    onProgress(key);
    let error: string | null = null;
    try {
      if (remove) await remove(entry);
      else await cleanupLocalModel(entry, selectedSttModelIds);
    } catch (failure) {
      error = String(failure);
    }
    const outcome = { key, error };
    outcomes.push(outcome);
    onProgress(key, outcome);
  }
  return outcomes;
};

// Both estimates cover the same attempted models, so a failed removal and
// unrelated disk changes don't count as freed.
export const cleanupFreedBytes = (before: number | null, remaining: number | null) =>
  before === null || remaining === null ? null : Math.max(0, before - remaining);

export const formatStorageBytes = (bytes: number): string => {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  const exponent = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: exponent > 0 ? 1 : 0 }).format(bytes / 1024 ** exponent)} ${units[exponent]}`;
};
