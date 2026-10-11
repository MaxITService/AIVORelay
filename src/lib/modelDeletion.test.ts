import { afterAll, beforeAll, beforeEach, expect, mock, spyOn, test } from "bun:test";
import { sessionToast } from "@/lib/sessionToast";

type Job = {
  model_id: string;
  job_id: string;
  status: "pending" | "deleting";
  deadline_unix_ms: number;
  remaining_ms: number;
};
type Change = {
  model_id: string;
  job_id: string;
  status: "pending" | "cancelled" | "deleting" | "completed" | "failed";
  error: string | null;
} | null;
type Snapshot = { revision: number; jobs: Job[]; change: Change };

// The module keeps process-wide state, so every test uses its own model id and
// the revision counter only ever grows, like the backend's.
let revision = 0;
const snapshot = (jobs: Job[], change: Change = null): Snapshot => ({ revision: ++revision, jobs, change });
const job = (modelId: string, status: Job["status"] = "pending", remainingMs = 5000): Job => ({
  model_id: modelId, job_id: `job-${modelId}`, status, deadline_unix_ms: 0, remaining_ms: remainingMs,
});

const calls: Array<{ command: string; args: Record<string, unknown> | undefined }> = [];
let handlers: Record<string, (args?: Record<string, unknown>) => unknown> = {};
let eventHandler: ((event: { payload: Snapshot }) => void) | undefined;
const callbacks = new Map<number, (event: { payload: Snapshot }) => void>();
let callbackId = 0;

// No `mock.module`: it is process-wide in bun and would leak into other test
// files. The Tauri API reads `window.__TAURI_INTERNALS__`, so stub that instead
// (the same approach as settingsStore.test.ts) and restore it afterwards.
const originalWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
const originalDocument = Object.getOwnPropertyDescriptor(globalThis, "document");

const tauriInternals = {
  transformCallback: (callback: (event: { payload: Snapshot }) => void) => {
    callbacks.set(++callbackId, callback);
    return callbackId;
  },
  invoke: async (command: string, args?: Record<string, unknown>) => {
    if (command === "plugin:event|listen") {
      eventHandler = callbacks.get(args?.handler as number);
      return 1;
    }
    calls.push({ command, args });
    const handler = handlers[command];
    if (!handler) throw new Error(`Unexpected command ${command}`);
    return handler(args);
  },
};

// `@/i18n` relies on Vite's import.meta.glob, so it cannot be loaded by bun;
// no other test imports it, which makes this module mock harmless to leak.
mock.module("@/i18n", () => ({ default: { t: (key: string) => key } }));

let api: typeof import("./modelDeletion");
let toastError: ReturnType<typeof spyOn>;

beforeAll(async () => {
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    writable: true,
    value: { __TAURI_INTERNALS__: tauriInternals, addEventListener: () => {} },
  });
  Object.defineProperty(globalThis, "document", {
    configurable: true,
    writable: true,
    value: { visibilityState: "visible", addEventListener: () => {} },
  });
  api = await import("./modelDeletion");
});

afterAll(() => {
  toastError?.mockRestore();
  for (const [name, original] of [["window", originalWindow], ["document", originalDocument]] as const) {
    if (original) Object.defineProperty(globalThis, name, original);
    else delete (globalThis as Record<string, unknown>)[name];
  }
});

const count = (command: string) => calls.filter(call => call.command === command).length;

beforeEach(() => {
  calls.length = 0;
  toastError?.mockRestore();
  toastError = spyOn(sessionToast, "error").mockImplementation((() => "") as never);
  handlers = { get_model_deletion_snapshot: () => snapshot([]) };
});

test("requestModelDeletion schedules once and a second request while pending is a no-op", async () => {
  handlers.schedule_model_deletion = ({ modelId } = {}) => snapshot([job(String(modelId))]);
  await api.requestModelDeletion("m-schedule");
  expect(api.getModelDeletionState().jobs.get("m-schedule")?.status).toBe("pending");

  await api.requestModelDeletion("m-schedule");
  expect(count("schedule_model_deletion")).toBe(1);
  expect(calls.find(call => call.command === "schedule_model_deletion")?.args).toEqual({ modelId: "m-schedule" });
});

test("concurrent requests for the same model share one backend call", async () => {
  handlers.schedule_model_deletion = ({ modelId } = {}) => snapshot([job(String(modelId))]);
  await Promise.all([api.requestModelDeletion("m-concurrent"), api.requestModelDeletion("m-concurrent")]);
  expect(count("schedule_model_deletion")).toBe(1);
});

test("cancelModelDeletion sends the scheduled job id and clears the job", async () => {
  handlers.schedule_model_deletion = ({ modelId } = {}) => snapshot([job(String(modelId))]);
  handlers.cancel_model_deletion = () => ({
    outcome: "cancelled",
    snapshot: snapshot([], { model_id: "m-cancel", job_id: "job-m-cancel", status: "cancelled", error: null }),
  });
  await api.requestModelDeletion("m-cancel");
  await api.cancelModelDeletion("m-cancel");

  expect(calls.find(call => call.command === "cancel_model_deletion")?.args)
    .toEqual({ modelId: "m-cancel", jobId: "job-m-cancel" });
  expect(api.getModelDeletionState().jobs.has("m-cancel")).toBe(false);
});

test("cancelling after deletion has started fails with the too-late message", async () => {
  handlers.schedule_model_deletion = ({ modelId } = {}) => snapshot([job(String(modelId))]);
  handlers.cancel_model_deletion = () => ({
    outcome: "deleting",
    snapshot: snapshot([job("m-late", "deleting")]),
  });
  await api.requestModelDeletion("m-late");
  await expect(api.cancelModelDeletion("m-late")).rejects.toThrow("modelSelector.deletion.tooLate");

  // A new delete request must not restart a job that is already deleting.
  await expect(api.requestModelDeletion("m-late")).rejects.toThrow("modelSelector.deletion.tooLate");
  expect(count("schedule_model_deletion")).toBe(1);
});

test("cancelModelDeletionBeforeSelection does nothing when no deletion is scheduled", async () => {
  await api.cancelModelDeletionBeforeSelection("m-none");
  expect(count("cancel_model_deletion")).toBe(0);
});

test("cancelModelDeletionBeforeSelection tolerates a job that vanished before the cancel arrived", async () => {
  handlers.schedule_model_deletion = ({ modelId } = {}) => snapshot([job(String(modelId))]);
  handlers.cancel_model_deletion = () => ({ outcome: "not_found", snapshot: snapshot([]) });
  await api.requestModelDeletion("m-vanished");
  await api.cancelModelDeletionBeforeSelection("m-vanished");
  expect(count("cancel_model_deletion")).toBe(1);
});

test("an outdated snapshot event cannot resurrect a cancelled job", async () => {
  await api.ensureModelDeletionTracking();
  expect(eventHandler).toBeDefined();
  eventHandler!({ payload: snapshot([]) });
  eventHandler!({ payload: { revision: 0, jobs: [job("m-stale")], change: null } });
  expect(api.getModelDeletionState().jobs.has("m-stale")).toBe(false);
});

test("a failed deletion shows one error toast per new revision", async () => {
  await api.ensureModelDeletionTracking();
  const failed = snapshot([], { model_id: "m-fail", job_id: "j", status: "failed", error: "disk busy" });
  eventHandler!({ payload: failed });
  eventHandler!({ payload: failed });
  expect(toastError.mock.calls).toEqual([["modelSelector.deletion.failed", { description: "disk busy" }]]);
});

test("modelDeletionSecondsRemaining counts only pending jobs and never goes negative", () => {
  const view = (status: Job["status"], displayDeadline: number) => ({ ...job("m", status), displayDeadline });
  expect(api.modelDeletionSecondsRemaining(view("pending", 10_500), 10_000)).toBe(1);
  expect(api.modelDeletionSecondsRemaining(view("pending", 14_001), 10_000)).toBe(5);
  expect(api.modelDeletionSecondsRemaining(view("pending", 9_000), 10_000)).toBe(0);
  expect(api.modelDeletionSecondsRemaining(view("deleting", 20_000), 10_000)).toBe(0);
  expect(api.modelDeletionSecondsRemaining(undefined, 10_000)).toBe(0);
});
