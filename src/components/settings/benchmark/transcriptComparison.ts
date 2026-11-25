/// <reference lib="es2022.intl" />

const MAX_COMPARISON_CHARACTERS = 100_000;
const MAX_COMPARISON_WORDS = 4_000;
const MAX_ALIGNMENT_CELLS = 1_000_000;

const segmenter =
  typeof Intl.Segmenter === "function"
    ? new Intl.Segmenter(undefined, { granularity: "word" })
    : null;

interface Word {
  start: number;
  end: number;
  normalized: string;
}

type Edit = "substitutions" | "omissions" | "additions";

export interface TranscriptPart {
  text: string;
  edit?: Edit;
}

function getWords(text: string): Word[] {
  const words: Word[] = [];
  const addWord = (value: string, start: number) => {
    words.push({
      start,
      end: start + value.length,
      normalized: value.normalize("NFC").toLowerCase().replace(/\p{P}/gu, ""),
    });
  };

  if (segmenter) {
    for (const segment of segmenter.segment(text)) {
      if (segment.isWordLike) addWord(segment.segment, segment.index);
      if (words.length > MAX_COMPARISON_WORDS) break;
    }
  } else {
    for (const match of text.matchAll(/[\p{L}\p{N}][\p{L}\p{M}\p{N}]*/gu)) {
      addWord(match[0], match.index);
      if (words.length > MAX_COMPARISON_WORDS) break;
    }
  }
  return words;
}

function highlightWords(
  text: string,
  words: Word[],
  edits: Map<number, Edit>,
): TranscriptPart[] {
  const parts: TranscriptPart[] = [];
  let previousEnd = 0;

  for (let i = 0; i < words.length; i += 1) {
    const edit = edits.get(i);
    if (!edit) continue;
    const start = words[i].start;
    // Include spaces and punctuation between consecutive words with the same edit.
    while (edits.get(i + 1) === edit) i += 1;
    const end = words[i].end;
    if (start > previousEnd) parts.push({ text: text.slice(previousEnd, start) });
    parts.push({ text: text.slice(start, end), edit });
    previousEnd = end;
  }
  if (previousEnd < text.length) parts.push({ text: text.slice(previousEnd) });
  return parts;
}

export function compareTranscripts(reference: string, recognized: string) {
  if (reference.length + recognized.length > MAX_COMPARISON_CHARACTERS) return null;
  const expected = getWords(reference);
  const actual = getWords(recognized);
  const columns = actual.length + 1;
  const cells = (expected.length + 1) * columns;
  if (expected.length + actual.length > MAX_COMPARISON_WORDS || cells > MAX_ALIGNMENT_CELLS) {
    return null;
  }

  // Each cell is the minimum edit count for its pair of word prefixes.
  const costs = new Uint32Array(cells);
  for (let j = 1; j < columns; j += 1) costs[j] = j;
  for (let i = 1; i <= expected.length; i += 1) {
    const row = i * columns;
    costs[row] = i;
    for (let j = 1; j < columns; j += 1) {
      const index = row + j;
      const diagonal = costs[index - columns - 1];
      costs[index] =
        expected[i - 1].normalized === actual[j - 1].normalized
          ? diagonal
          : 1 + Math.min(diagonal, costs[index - columns], costs[index - 1]);
    }
  }

  const referenceEdits = new Map<number, Edit>();
  const recognizedEdits = new Map<number, Edit>();
  const counts: Record<Edit, number> = { substitutions: 0, omissions: 0, additions: 0 };
  let i = expected.length;
  let j = actual.length;
  while (i > 0 || j > 0) {
    const index = i * columns + j;
    if (i > 0 && j > 0 && expected[i - 1].normalized === actual[j - 1].normalized) {
      i -= 1;
      j -= 1;
    } else if (i > 0 && j > 0 && costs[index] === costs[index - columns - 1] + 1) {
      referenceEdits.set(--i, "substitutions");
      recognizedEdits.set(--j, "substitutions");
      counts.substitutions += 1;
    } else if (i > 0 && costs[index] === costs[index - columns] + 1) {
      referenceEdits.set(--i, "omissions");
      counts.omissions += 1;
    } else {
      recognizedEdits.set(--j, "additions");
      counts.additions += 1;
    }
  }

  return {
    reference: highlightWords(reference, expected, referenceEdits),
    recognized: highlightWords(recognized, actual, recognizedEdits),
    counts,
    wordErrorRate: expected.length === 0 ? null : costs[cells - 1] / expected.length,
  };
}
