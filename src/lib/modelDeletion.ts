import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import i18n from "@/i18n";
import { sessionToast as toast } from "@/lib/sessionToast";

export interface ModelDeletionJob {
  model_id: string;
  job_id: string;
  status: "pending" | "deleting";
  deadline_unix_ms: number;
  remaining_ms: number;
}

export interface ModelDeletionSnapshot {
  revision: number;
  jobs: ModelDeletionJob[];
  change: {
    model_id: string;
    job_id: string;
    status: "pending" | "cancelled" | "deleting" | "completed" | "failed";
    error: string | null;
  } | null;
}

export interface ModelDeletionViewJob extends ModelDeletionJob {
  displayDeadline: number;
}

interface ModelDeletionState {
  revision: number;
  jobs: ReadonlyMap<string, ModelDeletionViewJob>;
  busyModels: ReadonlySet<string>;
  change: ModelDeletionSnapshot["change"];
}

let state: ModelDeletionState = { revision: -1, jobs: new Map(), busyModels: new Set(), change: null };
const subscribers = new Set<() => void>();
const operations = new Map<string, Promise<void>>();
let tracking: Promise<void> | undefined;
let listening: Promise<unknown> | undefined;
let reconciliation: Promise<void> | undefined;
let focusListening = false;
const expiredRefreshJobs = new Set<string>();

export const getModelDeletionState = (): ModelDeletionState => state;
export const subscribeModelDeletionState = (subscriber: () => void): (() => void) => {
  subscribers.add(subscriber);
  return () => { subscribers.delete(subscriber); };
};

const publish = () => subscribers.forEach(subscriber => subscriber());
const applySnapshot = (snapshot: ModelDeletionSnapshot) => {
  if (snapshot.revision < state.revision) return;
  const newer = snapshot.revision > state.revision;
  const now = performance.now();
  const jobs = new Map<string, ModelDeletionViewJob>();
  for (const job of snapshot.jobs) {
    const previous = state.jobs.get(job.model_id);
    const deadline = now + Math.max(0, job.remaining_ms);
    jobs.set(job.model_id, {
      ...job,
      displayDeadline: previous?.job_id === job.job_id ? Math.min(previous.displayDeadline, deadline) : deadline,
    });
  }
  state = { ...state, revision: snapshot.revision, jobs, change: snapshot.change };
  publish();
  if (newer && snapshot.change?.status === "failed") {
    toast.error(i18n.t("modelSelector.deletion.failed"), { description: snapshot.change.error ?? undefined });
  }
};

export const reconcileModelDeletions = (): Promise<void> => {
  reconciliation ??= invoke<ModelDeletionSnapshot>("get_model_deletion_snapshot")
    .then(applySnapshot).finally(() => { reconciliation = undefined; });
  return reconciliation;
};

const refreshVisibleState = () => {
  if (document.visibilityState === "hidden") return;
  void reconcileModelDeletions().catch(error => {
    toast.error(i18n.t("modelSelector.deletion.syncFailed"), { description: String(error) });
  });
};

export const reconcileExpiredModelDeletions = (): void => {
  const expired = Array.from(state.jobs.values()).filter(job => job.status === "pending"
    && job.displayDeadline <= performance.now() && !expiredRefreshJobs.has(job.job_id));
  if (expired.length === 0) return;
  expired.forEach(job => expiredRefreshJobs.add(job.job_id));
  refreshVisibleState();
};

export const ensureModelDeletionTracking = (): Promise<void> => {
  if (!tracking) {
    tracking = (async () => {
      listening ??= listen<ModelDeletionSnapshot>("model-deletion-state", event => applySnapshot(event.payload)).catch(error => {
        listening = undefined;
        throw error;
      });
      await listening;
      // One subscription and recovery path for this window, including when
      // every model dropdown is closed or the settings page has changed.
      if (!focusListening) {
        focusListening = true;
        window.addEventListener("focus", refreshVisibleState);
        document.addEventListener("visibilitychange", refreshVisibleState);
      }
      await reconcileModelDeletions();
    })().catch(error => {
      tracking = undefined;
      throw error;
    });
  }
  return tracking;
};

const runOperation = (modelId: string, action: () => Promise<void>): Promise<void> => {
  const existing = operations.get(modelId);
  if (existing) return existing;
  const operation = action().finally(() => {
    operations.delete(modelId);
    state = { ...state, busyModels: new Set(operations.keys()) };
    publish();
  });
  operations.set(modelId, operation);
  state = { ...state, busyModels: new Set(operations.keys()) };
  publish();
  return operation;
};

export const requestModelDeletion = (modelId: string): Promise<void> => runOperation(modelId, async () => {
  await ensureModelDeletionTracking();
  const job = state.jobs.get(modelId);
  if (job?.status === "pending") return;
  if (job) throw new Error(i18n.t("modelSelector.deletion.tooLate"));
  applySnapshot(await invoke<ModelDeletionSnapshot>("schedule_model_deletion", { modelId }));
});

const cancelDeletion = async (modelId: string, jobId?: string, allowMissing = false): Promise<void> => {
  await operations.get(modelId);
  return runOperation(modelId, async () => {
    await ensureModelDeletionTracking();
    const job = state.jobs.get(modelId);
    const expectedJobId = jobId ?? job?.job_id;
    if (!expectedJobId) return;
    const result = await invoke<{
      outcome: "cancelled" | "deleting" | "not_found";
      snapshot: ModelDeletionSnapshot;
    }>("cancel_model_deletion", { modelId, jobId: expectedJobId });
    applySnapshot(result.snapshot);
    const change = result.snapshot.change;
    const alreadyCancelled = result.outcome === "not_found" && change?.status === "cancelled"
      && change.model_id === modelId && change.job_id === expectedJobId;
    const missingBeforeSelection = allowMissing && result.outcome === "not_found"
      && !result.snapshot.jobs.some(item => item.model_id === modelId);
    if (result.outcome !== "cancelled" && !alreadyCancelled && !missingBeforeSelection) {
      throw new Error(i18n.t("modelSelector.deletion.tooLate"));
    }
  });
};

export const cancelModelDeletion = (modelId: string, jobId?: string): Promise<void> => cancelDeletion(modelId, jobId);

export const cancelModelDeletionBeforeSelection = async (modelId: string): Promise<void> => {
  await ensureModelDeletionTracking();
  await operations.get(modelId);
  const job = state.jobs.get(modelId);
  if (job) await cancelDeletion(modelId, job.job_id, true);
};

export const modelDeletionSecondsRemaining = (job?: ModelDeletionViewJob, now = performance.now()): number =>
  job?.status === "pending" ? Math.max(0, Math.ceil((job.displayDeadline - now) / 1000)) : 0;
