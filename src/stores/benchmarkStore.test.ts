import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import type { AppSettings } from "@/bindings";
import {
  BOOST_SWEEP_DB_LEVELS,
  VAD_SWEEP_THRESHOLDS,
  useBenchmarkStore,
  type BenchmarkClipInfo,
  type BenchmarkRunRequest,
  type BenchmarkRunResult,
} from "./benchmarkStore";
import { STT_MODEL_IDS, type SttModelSelection } from "@/lib/sttModelSelection";

const originalWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
const initialState = useBenchmarkStore.getState();

let calls: Array<{ command: string; args: any }>;
let backend: (command: string, args: any) => Promise<any>;

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((ok, fail) => {
    resolve = ok;
    reject = fail;
  });
  return { promise, resolve, reject };
}

async function until(condition: () => boolean) {
  for (let turn = 0; turn < 100; turn += 1) {
    if (condition()) return;
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
  }
  throw new Error("Store did not reach the expected state");
}

function makeClip(
  id: string,
  name: string,
  recordedAtMs = 1_700_000_000_000,
): BenchmarkClipInfo {
  return {
    id,
    name,
    durationSeconds: 2.5,
    sampleCount: 40_000,
    deviceName: "USB Mic",
    recordedAtMs,
  };
}

function makeResult(
  id: string,
  clipId: string,
  overrides: Partial<BenchmarkRunResult> = {},
): BenchmarkRunResult {
  return {
    id,
    clipId,
    clipName: "Clip #1",
    timestampMs: 1_700_000_001_000,
    label: "Current",
    profileName: "Default",
    providerLabel: "Local",
    modelLabel: "whisper-large-v3-turbo",
    isLive: false,
    text: "Hello world",
    rawText: null,
    llmPostProcessed: false,
    error: null,
    durationMs: 250,
    clipDurationSeconds: 2.5,
    processedDurationSeconds: 2.5,
    settingsText: "Clip: Clip #1",
    ...overrides,
  };
}

function makeSettings(patch: Partial<AppSettings> = {}): AppSettings {
  return {
    transcription_provider: "local",
    selected_model: "whisper-large-v3-turbo",
    active_profile_id: "default",
    transcription_profiles: [],
    soniox_live_enabled: false,
    soniox_model: STT_MODEL_IDS.sonioxLive,
    deepgram_live_enabled: false,
    deepgram_model: STT_MODEL_IDS.deepgramLive,
    remote_stt: {
      base_url: "",
      model_id: "",
      provider_preset: "",
      debug_capture: false,
      debug_mode: "normal",
    },
    openai_realtime_whisper_flatten_enabled: false,
    ...patch,
  } as unknown as AppSettings;
}

beforeEach(() => {
  calls = [];
  backend = async () => undefined;
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: {
      __TAURI_INTERNALS__: {
        invoke: async (command: string, args: any) => {
          calls.push({ command, args });
          return backend(command, args);
        },
      },
    },
  });
  useBenchmarkStore.setState(initialState, true);
});

afterEach(() => {
  useBenchmarkStore.setState(initialState, true);
  if (originalWindow) {
    Object.defineProperty(globalThis, "window", originalWindow);
  } else {
    Reflect.deleteProperty(globalThis, "window");
  }
});

describe("benchmarkStore constants", () => {
  test("BOOST_SWEEP_DB_LEVELS has all 18 integer levels 0..=17 and VAD_SWEEP_THRESHOLDS has all 9 levels 0.1..=0.9", () => {
    expect(BOOST_SWEEP_DB_LEVELS).toHaveLength(18);
    expect([...BOOST_SWEEP_DB_LEVELS]).toEqual([
      0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17,
    ]);
    expect(VAD_SWEEP_THRESHOLDS).toHaveLength(9);
    expect([...VAD_SWEEP_THRESHOLDS]).toEqual([
      0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9,
    ]);
  });
});

describe("benchmarkStore state sync and clip management", () => {
  test("syncState loads clips and results, auto-selects newest clip, preserves existing selection, and switches filter when clips are empty", async () => {
    const clipNewest = makeClip("clip-2", "Clip #2", 2000);
    const clipOlder = makeClip("clip-1", "Clip #1", 1000);
    const res1 = makeResult("res-1", "clip-1");

    backend = async (command) => {
      if (command === "benchmark_get_state") {
        return { clips: [clipNewest, clipOlder], results: [res1] };
      }
    };

    await useBenchmarkStore.getState().syncState();
    expect(useBenchmarkStore.getState().clips).toEqual([clipNewest, clipOlder]);
    expect(useBenchmarkStore.getState().results).toEqual([res1]);
    expect(useBenchmarkStore.getState().selectedClipId).toBe("clip-2");
    expect(useBenchmarkStore.getState().resultsFilter).toBe("selected");

    // Select the older clip and re-sync: selection should be preserved.
    useBenchmarkStore.getState().selectClip("clip-1");
    expect(useBenchmarkStore.getState().selectedClipId).toBe("clip-1");
    await useBenchmarkStore.getState().syncState();
    expect(useBenchmarkStore.getState().selectedClipId).toBe("clip-1");

    // When all clips are gone but orphaned results remain, filter switches to "all".
    backend = async (command) => {
      if (command === "benchmark_get_state") {
        return { clips: [], results: [res1] };
      }
    };
    await useBenchmarkStore.getState().syncState();
    expect(useBenchmarkStore.getState().clips).toEqual([]);
    expect(useBenchmarkStore.getState().selectedClipId).toBeNull();
    expect(useBenchmarkStore.getState().results).toEqual([res1]);
    expect(useBenchmarkStore.getState().resultsFilter).toBe("all");
  });

  test("importFromHistory prepends and selects the imported clip, and propagates errors without touching clips", async () => {
    const existing = makeClip("clip-1", "Clip #1");
    const imported = makeClip("clip-2", "Clip #2 · History");
    useBenchmarkStore.setState({ clips: [existing], selectedClipId: "clip-1", lastError: "old" });

    backend = async (command) => {
      if (command === "benchmark_import_history_entry") return imported;
    };
    const clip = await useBenchmarkStore.getState().importFromHistory(42);
    expect(clip).toEqual(imported);
    expect(calls).toEqual([{ command: "benchmark_import_history_entry", args: { historyId: 42 } }]);
    expect(useBenchmarkStore.getState().clips).toEqual([imported, existing]);
    expect(useBenchmarkStore.getState().selectedClipId).toBe("clip-2");
    expect(useBenchmarkStore.getState().lastError).toBeNull();

    backend = async () => {
      throw new Error("History entry not found.");
    };
    await expect(useBenchmarkStore.getState().importFromHistory(7)).rejects.toThrow("History entry not found.");
    expect(useBenchmarkStore.getState().clips).toEqual([imported, existing]);
    expect(useBenchmarkStore.getState().selectedClipId).toBe("clip-2");
  });

  test("deleteClip removes the clip, updates selectedClipId, preserves results, and switches filter to all when last clip is deleted", async () => {
    const clip2 = makeClip("clip-2", "Clip #2");
    const clip1 = makeClip("clip-1", "Clip #1");
    const res2 = makeResult("res-2", "clip-2");
    const res1 = makeResult("res-1", "clip-1");

    useBenchmarkStore.setState({
      clips: [clip2, clip1],
      selectedClipId: "clip-2",
      results: [res2, res1],
      resultsFilter: "selected",
    });

    await useBenchmarkStore.getState().deleteClip("clip-2");
    expect(calls).toEqual([
      { command: "benchmark_delete_clip", args: { clipId: "clip-2" } },
    ]);
    expect(useBenchmarkStore.getState().clips).toEqual([clip1]);
    expect(useBenchmarkStore.getState().selectedClipId).toBe("clip-1");
    expect(useBenchmarkStore.getState().results).toEqual([res2, res1]);
    expect(useBenchmarkStore.getState().resultsFilter).toBe("selected");

    await useBenchmarkStore.getState().deleteClip("clip-1");
    expect(useBenchmarkStore.getState().clips).toEqual([]);
    expect(useBenchmarkStore.getState().selectedClipId).toBeNull();
    expect(useBenchmarkStore.getState().results).toEqual([res2, res1]);
    expect(useBenchmarkStore.getState().resultsFilter).toBe("all");
  });
});

describe("benchmarkStore single run and sweeps", () => {
  test("runCurrent passes clipId, target profile or custom model override, and prepends result with LLM fields", async () => {
    const clip = makeClip("clip-1", "Clip #1");
    const existingResult = makeResult("res-old", "clip-1", { label: "Older" });
    const profileResult = makeResult("res-profile", "clip-1", {
      label: "Profile Run",
      text: "Polished output",
      rawText: "raw output",
      llmPostProcessed: true,
    });
    const customResult = makeResult("res-custom", "clip-1", {
      label: "Custom Run",
      text: "Custom model output",
    });

    const receivedRequests: BenchmarkRunRequest[] = [];
    backend = async (command, args) => {
      if (command === "benchmark_run_single") {
        receivedRequests.push(args.request);
        return receivedRequests.length === 1 ? profileResult : customResult;
      }
    };

    useBenchmarkStore.setState({
      clips: [clip],
      selectedClipId: "clip-1",
      results: [existingResult],
      targetMode: "profile",
      selectedProfileId: "profile_work",
    });

    await useBenchmarkStore.getState().runCurrent("Profile Run");
    expect(receivedRequests[0]).toEqual({
      clipId: "clip-1",
      profileId: "profile_work",
      modelSelectionOverride: null,
      boostDbOverride: null,
      noiseCancellationOverride: null,
      filterSilenceOverride: null,
      vadThresholdOverride: null,
      vadBackendOverride: null,
      label: "Profile Run",
    });
    expect(useBenchmarkStore.getState().results.map((r) => r.id)).toEqual([
      "res-profile",
      "res-old",
    ]);
    expect(useBenchmarkStore.getState().results[0].llmPostProcessed).toBe(true);
    expect(useBenchmarkStore.getState().results[0].rawText).toBe("raw output");

    const customSelection: SttModelSelection = {
      provider: "remote_openai_compatible",
      provider_preset: "google",
      model_id: STT_MODEL_IDS.googleGeminiBatch,
    };
    useBenchmarkStore.getState().setTargetMode("custom");
    useBenchmarkStore.getState().setCustomSelection(customSelection);

    await useBenchmarkStore.getState().runCurrent("Custom Run");
    expect(receivedRequests[1]).toEqual({
      clipId: "clip-1",
      profileId: null,
      modelSelectionOverride: customSelection,
      boostDbOverride: null,
      noiseCancellationOverride: null,
      filterSilenceOverride: null,
      vadThresholdOverride: null,
      vadBackendOverride: null,
      label: "Custom Run",
    });
    expect(useBenchmarkStore.getState().results.map((r) => r.id)).toEqual([
      "res-custom",
      "res-profile",
      "res-old",
    ]);
    expect(useBenchmarkStore.getState().isRunning).toBe(false);
    expect(useBenchmarkStore.getState().progress).toBeNull();
  });

  test("runBoostSweep runs all 18 levels (0..17 dB) sequentially via benchmark_run_single", async () => {
    const clip = makeClip("clip-1", "Clip #1");
    const receivedRequests: BenchmarkRunRequest[] = [];
    let activeCalls = 0;
    let maxConcurrentCalls = 0;

    backend = async (command, args) => {
      if (command !== "benchmark_run_single") return;
      activeCalls += 1;
      maxConcurrentCalls = Math.max(maxConcurrentCalls, activeCalls);
      const req = args.request as BenchmarkRunRequest;
      receivedRequests.push(req);
      await Promise.resolve();
      activeCalls -= 1;
      return makeResult(`res-boost-${req.boostDbOverride}`, req.clipId, {
        label: req.label ?? "",
      });
    };

    useBenchmarkStore.setState({
      clips: [clip],
      selectedClipId: "clip-1",
      results: [],
    });

    await useBenchmarkStore
      .getState()
      .runBoostSweep((db) => (db === 0 ? "Boost Off (0 dB)" : `Boost +${db} dB`));

    expect(maxConcurrentCalls).toBe(1);
    expect(receivedRequests).toHaveLength(18);
    expect(receivedRequests.map((r) => r.boostDbOverride)).toEqual([
      0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17,
    ]);
    expect(receivedRequests[0].label).toBe("Boost Off (0 dB)");
    expect(receivedRequests[17].label).toBe("Boost +17 dB");
    expect(useBenchmarkStore.getState().results).toHaveLength(18);
    expect(useBenchmarkStore.getState().results[0].id).toBe("res-boost-17");
    expect(useBenchmarkStore.getState().results[17].id).toBe("res-boost-0");
    expect(useBenchmarkStore.getState().isRunning).toBe(false);
    expect(useBenchmarkStore.getState().lastError).toBeNull();
  });

  test("runNoiseSweep runs 11 steps for non-live providers and 2 steps for live providers", async () => {
    const clip = makeClip("clip-1", "Clip #1");
    const labels = {
      rnnoiseOff: "RNNoise Off",
      rnnoiseOn: "RNNoise On",
      vadLevel: (threshold: number) => `VAD ${threshold.toFixed(1)}`,
    };

    let receivedRequests: BenchmarkRunRequest[] = [];
    backend = async (command, args) => {
      if (command !== "benchmark_run_single") return;
      const req = args.request as BenchmarkRunRequest;
      receivedRequests.push(req);
      return makeResult(`res-noise-${receivedRequests.length}`, req.clipId, {
        label: req.label ?? "",
      });
    };

    // 1. Non-live (local) provider -> 11 steps (RNNoise Off, RNNoise On, VAD 0.1..0.9)
    useBenchmarkStore.setState({
      clips: [clip],
      selectedClipId: "clip-1",
      results: [],
      targetMode: "active",
    });

    await useBenchmarkStore
      .getState()
      .runNoiseSweep(makeSettings({ transcription_provider: "local" }), labels);

    expect(receivedRequests).toHaveLength(11);
    expect(receivedRequests[0]).toMatchObject({
      noiseCancellationOverride: false,
      filterSilenceOverride: false,
      vadThresholdOverride: null,
      label: "RNNoise Off",
    });
    expect(receivedRequests[1]).toMatchObject({
      noiseCancellationOverride: true,
      filterSilenceOverride: false,
      vadThresholdOverride: null,
      label: "RNNoise On",
    });
    expect(
      receivedRequests.slice(2).map((r) => ({
        filterSilenceOverride: r.filterSilenceOverride,
        vadThresholdOverride: r.vadThresholdOverride,
        label: r.label,
      })),
    ).toEqual(
      VAD_SWEEP_THRESHOLDS.map((t) => ({
        filterSilenceOverride: true,
        vadThresholdOverride: t,
        label: `VAD ${t.toFixed(1)}`,
      })),
    );

    // 2. Live provider (Soniox live enabled) -> 2 steps (RNNoise Off, RNNoise On)
    receivedRequests = [];
    useBenchmarkStore.setState({ results: [] });

    await useBenchmarkStore.getState().runNoiseSweep(
      makeSettings({
        transcription_provider: "remote_soniox",
        soniox_live_enabled: true,
        soniox_model: STT_MODEL_IDS.sonioxLive,
      }),
      labels,
    );

    expect(receivedRequests).toHaveLength(2);
    expect(receivedRequests[0]).toMatchObject({
      noiseCancellationOverride: false,
      filterSilenceOverride: false,
      vadThresholdOverride: null,
      label: "RNNoise Off",
    });
    expect(receivedRequests[1]).toMatchObject({
      noiseCancellationOverride: true,
      filterSilenceOverride: false,
      vadThresholdOverride: null,
      label: "RNNoise On",
    });
  });

  test("cancelRun aborts an in-progress sweep cleanly without recording a spurious lastError", async () => {
    const clip = makeClip("clip-1", "Clip #1");
    const secondStep = deferred<BenchmarkRunResult>();
    const stepDbs: Array<number | null> = [];

    backend = async (command, args) => {
      if (command === "benchmark_run_single") {
        const req = args.request as BenchmarkRunRequest;
        stepDbs.push(req.boostDbOverride);
        if (stepDbs.length === 1) {
          return makeResult("res-0", req.clipId, { label: req.label ?? "" });
        }
        return secondStep.promise;
      }
      if (command === "benchmark_cancel_run") {
        return undefined;
      }
    };

    useBenchmarkStore.setState({
      clips: [clip],
      selectedClipId: "clip-1",
      results: [],
    });

    const sweepPromise = useBenchmarkStore
      .getState()
      .runBoostSweep((db) => `Boost +${db} dB`);

    await until(() => stepDbs.length === 2);
    expect(useBenchmarkStore.getState().isRunning).toBe(true);

    await useBenchmarkStore.getState().cancelRun();
    expect(useBenchmarkStore.getState().cancelling).toBe(true);
    expect(calls.some((c) => c.command === "benchmark_cancel_run")).toBe(true);

    secondStep.reject("Benchmark run cancelled");
    await sweepPromise;

    expect(stepDbs).toEqual([0, 1]);
    expect(useBenchmarkStore.getState().results.map((r) => r.id)).toEqual([
      "res-0",
    ]);
    expect(useBenchmarkStore.getState().isRunning).toBe(false);
    expect(useBenchmarkStore.getState().cancelling).toBe(false);
    expect(useBenchmarkStore.getState().progress).toBeNull();
    expect(useBenchmarkStore.getState().lastError).toBeNull();
  });
});

describe("benchmarkStore result deletion", () => {
  test("removeResult deletes one result and clearResults supports per-clip and all", async () => {
    const r1 = makeResult("r1", "clip-1");
    const r2 = makeResult("r2", "clip-1");
    const r3 = makeResult("r3", "clip-2");

    useBenchmarkStore.setState({
      results: [r1, r2, r3],
    });

    await useBenchmarkStore.getState().removeResult("r1");
    expect(calls[0]).toEqual({
      command: "benchmark_delete_result",
      args: { resultId: "r1" },
    });
    expect(useBenchmarkStore.getState().results.map((r) => r.id)).toEqual([
      "r2",
      "r3",
    ]);

    await useBenchmarkStore.getState().clearResults("clip-1");
    expect(calls[1]).toEqual({
      command: "benchmark_clear_results",
      args: { clipId: "clip-1" },
    });
    expect(useBenchmarkStore.getState().results.map((r) => r.id)).toEqual([
      "r3",
    ]);

    await useBenchmarkStore.getState().clearResults(null);
    expect(calls[2]).toEqual({
      command: "benchmark_clear_results",
      args: { clipId: null },
    });
    expect(useBenchmarkStore.getState().results).toEqual([]);
  });
});
