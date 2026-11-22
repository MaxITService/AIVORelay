import type { HistoryEntry } from "@/bindings";
import { historyDisplayText } from "./search";

export const HISTORY_EXPORT_FORMAT = "aivorelay-history";
export const HISTORY_EXPORT_VERSION = 1;

/** One finished result: the text the user actually got, plus when and how. */
export type HistoryExportEntry = {
  id: number;
  /** ISO 8601, UTC. */
  date: string;
  type: string;
  text: string;
  language: string | null;
  saved: boolean;
};

export type HistoryExport = {
  format: typeof HISTORY_EXPORT_FORMAT;
  version: typeof HISTORY_EXPORT_VERSION;
  exported_at: string;
  count: number;
  entries: HistoryExportEntry[];
};

/**
 * Keeps only final results: post-processed text when it exists, the AI
 * response for AI Replace, otherwise the raw transcript. Entries without any
 * text (failed transcriptions) are left out.
 */
export const buildHistoryExport = (
  entries: readonly HistoryEntry[],
  now: Date = new Date(),
): HistoryExport => {
  const exported = entries
    .map((entry): HistoryExportEntry => ({
      id: entry.id,
      date: new Date(entry.timestamp * 1000).toISOString(),
      type: entry.action_type,
      text: historyDisplayText(entry).trim(),
      language: entry.language,
      saved: entry.saved,
    }))
    .filter((entry) => entry.text.length > 0);

  return {
    format: HISTORY_EXPORT_FORMAT,
    version: HISTORY_EXPORT_VERSION,
    exported_at: now.toISOString(),
    count: exported.length,
    entries: exported,
  };
};

export const serializeHistoryExport = (data: HistoryExport): string =>
  `${JSON.stringify(data, null, 2)}\n`;

export const historyExportFileName = (now: Date = new Date()): string => {
  const pad = (value: number) => String(value).padStart(2, "0");
  return `aivorelay-history-${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}.json`;
};

type HistoryPage = { entries: HistoryEntry[]; has_more: boolean };

/** Walks every history page, newest first. The backend caps a page at 100. */
export const fetchEntireHistory = async (
  fetchPage: (cursor: number | null) => Promise<HistoryPage>,
): Promise<HistoryEntry[]> => {
  const all: HistoryEntry[] = [];
  let cursor: number | null = null;
  for (;;) {
    const page = await fetchPage(cursor);
    const entries = page.entries ?? [];
    all.push(...entries);
    const last = entries[entries.length - 1];
    if (!page.has_more || !last) return all;
    cursor = last.id;
  }
};
