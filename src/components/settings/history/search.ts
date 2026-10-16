import type { HistoryEntry } from "@/bindings";

export interface TextSegment {
  text: string;
  match: boolean;
}

// Like Rust's Unicode regex, use simple case folding for both search and highlights.
export const matchesSearch = (text: string, query: string): boolean =>
  new RegExp(escapeRegExp(query.trim()), "iu").test(text);

const escapeRegExp = (value: string) =>
  value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

export const historyDisplayText = (entry: HistoryEntry): string =>
  entry.action_type === "ai_replace"
    ? (entry.ai_response ?? entry.transcription_text)
    : (entry.post_processed_text ?? entry.transcription_text);

/** "unchanged": processing ran but kept the raw text, so there is nothing to switch to. */
export type RawTranscriptToggle = "none" | "unchanged" | "available";

export const rawTranscriptToggle = (entry: HistoryEntry): RawTranscriptToggle =>
  entry.action_type === "ai_replace" || entry.post_processed_text == null
    ? "none"
    : entry.post_processed_text === entry.transcription_text
      ? "unchanged"
      : "available";

export const matchesHistorySearch = (entry: HistoryEntry, query: string): boolean =>
  entry.action_type === "ai_replace"
    ? [
        entry.transcription_text,
        entry.original_selection ?? "",
        entry.ai_response ?? "",
      ].some((text) => matchesSearch(text, query))
    : matchesSearch(historyDisplayText(entry), query);

/**
 * Splits `text` into plain and matching segments so search hits can be
 * highlighted. Uses a Unicode case-insensitive regex rather than lowercasing,
 * because lowercasing can change string length and shift match offsets.
 */
export const splitByQuery = (text: string, query: string): TextSegment[] => {
  const needle = query.trim();
  if (!needle) return [{ text, match: false }];

  const segments: TextSegment[] = [];
  let lastIndex = 0;
  for (const match of text.matchAll(new RegExp(escapeRegExp(needle), "giu"))) {
    const start = match.index;
    if (start > lastIndex) {
      segments.push({ text: text.slice(lastIndex, start), match: false });
    }
    segments.push({ text: match[0], match: true });
    lastIndex = start + match[0].length;
  }
  if (lastIndex < text.length) {
    segments.push({ text: text.slice(lastIndex), match: false });
  }
  return segments;
};
