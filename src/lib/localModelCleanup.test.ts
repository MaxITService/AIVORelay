import { describe, expect, test } from "bun:test";
import {
  cleanSelectedLocalModels, cleanupFreedBytes, estimateLocalModelCleanup, formatStorageBytes,
  modelStorageKey, type LocalModelStorageEntry,
} from "./localModelCleanup";

const entry = (id: string, kind: "stt" | "tts" = "stt", bytes = 100): LocalModelStorageEntry => ({
  id, kind, bytes, bytes_known: true, name: id, shared_cache_bytes: 0, blocked_reason: null,
});
type TauriCall = { command: string; args: Record<string, unknown> };
const withTauriCalls = async (run: () => Promise<unknown>): Promise<TauriCall[]> => {
  const originalWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
  const calls: TauriCall[] = [];
  Object.defineProperty(globalThis, "window", { configurable: true, value: {
    __TAURI_INTERNALS__: { invoke: async (command: string, args: Record<string, unknown>) => { calls.push({ command, args }); } },
  } });
  try {
    await run();
    return calls;
  } finally {
    if (originalWindow) Object.defineProperty(globalThis, "window", originalWindow);
    else Reflect.deleteProperty(globalThis, "window");
  }
};

describe("local model cleanup", () => {
  test("removes sequentially, preserves model type and continues after a busy model", async () => {
    let running = 0;
    const calls: string[] = [];
    const progress: string[] = [];
    const results = await cleanSelectedLocalModels([entry("same"), entry("busy"), entry("same", "tts")], async model => {
      expect(++running).toBe(1);
      calls.push(modelStorageKey(model));
      await Promise.resolve();
      running--;
      if (model.id === "busy") throw new Error("Recording is active");
    }, (key, outcome) => { progress.push(`${key}:${outcome ? "done" : "start"}`); });
    expect(calls).toEqual(["stt:same", "stt:busy", "tts:same"]);
    expect(results.map(result => !!result.error)).toEqual([false, true, false]);
    expect(progress).toEqual(["stt:same:start", "stt:same:done", "stt:busy:start", "stt:busy:done", "tts:same:start", "tts:same:done"]);
  });

  test("does not remove blocked entries or duplicate selections", async () => {
    const calls: string[] = [];
    const results = await cleanSelectedLocalModels([
      entry("a"), { ...entry("installing"), blocked_reason: "Installing" }, entry("a"),
    ], async model => { calls.push(model.id); });
    expect(calls).toEqual(["a"]);
    expect(results.length).toBe(1);
  });

  test("forwards the complete selected STT scope to each cleanup command", async () => {
    const calls = await withTauriCalls(() => cleanSelectedLocalModels([entry("a"), entry("alias"), entry("qwen", "tts"),
      { ...entry("busy"), blocked_reason: "Downloading" }]));
    expect(calls.map(call => call.args.modelId)).toEqual(["a", "alias", "qwen"]);
    expect(calls.every(call => call.command === "cleanup_local_model"
      && JSON.stringify(call.args.selectedSttModelIds) === JSON.stringify(["a", "alias"]))).toBe(true);
  });

  test("estimates the whole selection in one backend call, split by model type", async () => {
    const calls = await withTauriCalls(() => estimateLocalModelCleanup([entry("a"), entry("qwen", "tts"), entry("alias")]));
    expect(calls).toEqual([{ command: "estimate_local_model_cleanup", args: { sttModelIds: ["a", "alias"], ttsModelIds: ["qwen"] } }]);
  });

  test("reports measured partial removal even when the operation failed", () => {
    expect(cleanupFreedBytes(1000, 300)).toBe(700);
    expect(cleanupFreedBytes(0, 0)).toBe(0);
  });

  test("does not count growth or unknown sizes as removed", () => {
    expect(cleanupFreedBytes(100, 200)).toBe(0);
    expect(cleanupFreedBytes(null, 0)).toBeNull();
    expect(cleanupFreedBytes(1000, null)).toBeNull();
  });

  test("uses binary units and handles empty sizes", () => {
    expect(formatStorageBytes(0)).toBe("0 B");
    expect(formatStorageBytes(Number.NaN)).toBe("0 B");
    expect(formatStorageBytes(1024 ** 3)).toBe("1 GiB");
  });
});
