import { describe, expect, test } from "bun:test";
import {
  cleanSelectedLocalModels, formatStorageBytes, modelStorageKey, removedModelBytes,
  type LocalModelStorageEntry, type LocalModelStorageSnapshot,
} from "./localModelCleanup";

const entry = (id: string, kind: "stt" | "tts" = "stt", bytes = 100): LocalModelStorageEntry => ({
  id, kind, bytes, name: id, shared_cache_bytes: 0, blocked_reason: null,
});
const snapshot = (entries: LocalModelStorageEntry[]): LocalModelStorageSnapshot => ({
  entries, total_bytes: entries.reduce((sum, model) => sum + model.bytes, 0), disk_free_bytes: null,
});

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

  test("reports measured partial removal even when the operation failed", () => {
    const before = snapshot([entry("partial", "stt", 1000), entry("unselected", "tts", 500)]);
    const after = snapshot([entry("partial", "stt", 300)]);
    expect(removedModelBytes(before, after, new Set(["stt:partial"]))).toBe(700);
  });

  test("does not count shared blobs or unrelated concurrent disk changes as removed", () => {
    const before = snapshot([{ ...entry("shared", "stt", 0), shared_cache_bytes: 1000000 }, entry("other", "tts", 200)]);
    expect(removedModelBytes(before, snapshot([]), new Set(["stt:shared"]))).toBe(0);
    expect(removedModelBytes(snapshot([entry("growing")]), snapshot([entry("growing", "stt", 200)]), new Set(["stt:growing"]))).toBe(0);
    expect(removedModelBytes(snapshot([entry("unreadable")]), snapshot([{ ...entry("unreadable", "stt", 0), blocked_reason: "Permission denied" }]), new Set(["stt:unreadable"]))).toBe(0);
  });

  test("uses binary units and handles empty sizes", () => {
    expect(formatStorageBytes(0)).toBe("0 B");
    expect(formatStorageBytes(Number.NaN)).toBe("0 B");
    expect(formatStorageBytes(1024 ** 3)).toBe("1 GiB");
  });
});
