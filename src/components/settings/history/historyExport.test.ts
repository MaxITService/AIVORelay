import { expect, test } from "bun:test";
import type { HistoryEntry } from "@/bindings";
import {
  buildHistoryExport,
  fetchEntireHistory,
  historyExportFileName,
} from "./historyExport";

const entry = (id: number, overrides: Partial<HistoryEntry> = {}): HistoryEntry => ({
  id,
  file_name: `aivorelay-${id}.wav`,
  timestamp: 1_700_000_000 + id,
  saved: false,
  title: "",
  transcription_text: `raw ${id}`,
  post_processed_text: null,
  post_process_prompt: null,
  post_process_requested: false,
  action_type: "transcribe",
  original_selection: null,
  ai_response: null,
  language: "en",
  language_source: null,
  ...overrides,
});

test("export keeps only final text and drops failed entries", () => {
  const now = new Date("2026-11-23T10:00:00Z");
  const result = buildHistoryExport([
    entry(4, { post_processed_text: "processed 4" }),
    entry(3, { action_type: "ai_replace", ai_response: "answer 3", saved: true }),
    entry(2, { transcription_text: "   " }),
    entry(1),
  ], now);

  expect(result.exported_at).toBe("2026-11-23T10:00:00.000Z");
  expect(result.count).toBe(3);
  expect(result.entries.map((e) => [e.id, e.text, e.type, e.saved])).toEqual([
    [4, "processed 4", "transcribe", false],
    [3, "answer 3", "ai_replace", true],
    [1, "raw 1", "transcribe", false],
  ]);
  expect(result.entries[2].date).toBe(new Date((1_700_000_000 + 1) * 1000).toISOString());
});

test("fetchEntireHistory follows cursors until the last page", async () => {
  const pages: Record<string, { entries: HistoryEntry[]; has_more: boolean }> = {
    null: { entries: [entry(5), entry(4)], has_more: true },
    4: { entries: [entry(3), entry(2)], has_more: true },
    2: { entries: [entry(1)], has_more: false },
  };
  const cursors: (number | null)[] = [];
  const all = await fetchEntireHistory(async (cursor) => {
    cursors.push(cursor);
    return pages[String(cursor)];
  });
  expect(cursors).toEqual([null, 4, 2]);
  expect(all.map((e) => e.id)).toEqual([5, 4, 3, 2, 1]);
});

test("file name uses the local date", () => {
  expect(historyExportFileName(new Date(2026, 0, 5))).toBe("aivorelay-history-2026-01-05.json");
});
