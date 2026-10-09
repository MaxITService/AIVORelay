import { expect, test } from "bun:test";
import { subscribeRecordingModelLoading, type RecordingModelLoadingState } from "./useRecordingModelLoading";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((ok) => { resolve = ok; });
  return { promise, resolve };
}

function state(session = 7, elapsed = 0): RecordingModelLoadingState {
  return {
    recording_session_id: session, provider: "local", model_id: "model-a",
    loading: true, loading_elapsed_ms: elapsed, preview_active: true,
  };
}

async function settle() {
  for (let i = 0; i < 20; i++) await Promise.resolve();
}

function harness(failedSubscription?: string) {
  const listeners = new Map<string, (payload?: RecordingModelLoadingState) => void>();
  const reads: Array<ReturnType<typeof deferred<RecordingModelLoadingState>>> = [];
  const states: RecordingModelLoadingState[] = [];
  const slow: boolean[] = [];
  const timers = new Map<number, { callback: () => void; delay: number }>();
  let timerId = 0;
  let unlistened = 0;
  const dispose = subscribeRecordingModelLoading({
    readState: () => { const read = deferred<RecordingModelLoadingState>(); reads.push(read); return read.promise; },
    subscribe: async (name, callback) => {
      if (name === failedSubscription) throw new Error("window recovering");
      listeners.set(name, callback);
      return () => { listeners.delete(name); unlistened++; };
    },
    setState: (value) => states.push(value),
    setSlow: (value) => slow.push(value),
    schedule: (callback, delay) => {
      timers.set(++timerId, { callback, delay });
      return timerId as unknown as ReturnType<typeof setTimeout>;
    },
    cancel: (id) => { timers.delete(id as unknown as number); },
  });
  return { listeners, reads, states, slow, timers, dispose, unlistened: () => unlistened };
}

test("loading event supersedes an outstanding initial snapshot", async () => {
  const h = harness();
  await settle();
  expect(h.reads).toHaveLength(1);
  const newer = state(8, 2300);
  h.listeners.get("recording-model-loading-state")!(newer);
  h.reads[0].resolve(state(7));
  await settle();
  expect(h.states).toEqual([newer]);
  expect(h.slow).toEqual([true]);
  h.dispose();
});

test("overlapping refresh snapshots accept only the newest request", async () => {
  const h = harness();
  await settle();
  h.listeners.get("show-overlay")!();
  h.listeners.get("preview-output-mode-state")!();
  expect(h.reads).toHaveLength(3);
  const newest = state(9, 2500);
  h.reads[2].resolve(newest);
  h.reads[1].resolve(state(8));
  h.reads[0].resolve(state(7));
  await settle();
  expect(h.states).toEqual([newest]);
  h.dispose();
});

test("completion clears the slow timer and disposal rejects late snapshots", async () => {
  const h = harness();
  await settle();
  h.listeners.get("recording-model-loading-state")!(state(7, 500));
  expect([...h.timers.values()].map((timer) => timer.delay)).toEqual([1500]);
  const completed = { ...state(), loading: false };
  h.listeners.get("recording-model-loading-state")!(completed);
  expect(h.timers.size).toBe(0);
  expect(h.slow.at(-1)).toBe(false);
  h.listeners.get("recording-model-loading-state")!(state(8));
  const pendingTimer = [...h.timers.values()][0];
  h.dispose();
  expect(h.timers.size).toBe(0);
  expect(h.listeners.size).toBe(0);
  const before = h.states.length;
  h.reads[0].resolve(state(9));
  pendingTimer.callback();
  await settle();
  expect(h.states.length).toBe(before);
  expect(h.slow.at(-1)).toBe(false);
});

test("failed auxiliary or primary subscription still hydrates from the command", async () => {
  for (const name of ["show-overlay", "recording-model-loading-state"]) {
    const h = harness(name);
    await settle();
    expect(h.reads).toHaveLength(1);
    h.reads[0].resolve(state(7, 2500));
    await settle();
    expect(h.states).toEqual([state(7, 2500)]);
    expect(h.slow.at(-1)).toBe(true);
    h.dispose();
  }
});

test("a subscription resolving after disposal is immediately released", async () => {
  const pending = deferred<() => void>();
  let unlistened = 0;
  let reads = 0;
  const dispose = subscribeRecordingModelLoading({
    readState: async () => { reads++; return state(); },
    subscribe: () => pending.promise,
    setState: () => { throw new Error("disposed state update"); },
    setSlow: () => { throw new Error("disposed timer update"); },
  });
  dispose();
  pending.resolve(() => { unlistened++; });
  await settle();
  expect(unlistened).toBe(1);
  expect(reads).toBe(0);
});

test("disposal during the final auxiliary subscription does not start hydration", async () => {
  const pending = deferred<() => void>();
  let unlistened = 0;
  let reads = 0;
  const dispose = subscribeRecordingModelLoading({
    readState: async () => { reads++; return state(); },
    subscribe: async (name) => name === "preview_output_mode_state"
      ? pending.promise : () => { unlistened++; },
    setState: () => { throw new Error("disposed state update"); },
    setSlow: () => { throw new Error("disposed timer update"); },
  });
  await settle();
  dispose();
  pending.resolve(() => { unlistened++; });
  await settle();
  expect(unlistened).toBe(6);
  expect(reads).toBe(0);
});
