import {
  GEMINI_VOCABULARY_HARD_MAX,
  GEMINI_VOCABULARY_RECOMMENDED_MAX,
} from "./geminiConfig";

export type GeminiVocabularyFormat = "empty" | "newline" | "csv" | "json" | "unknown";

export interface GeminiVocabularyIssue {
  code: string;
  message: string;
  value?: string;
  line?: number;
  position?: number;
}

export interface GeminiVocabularyParseResult {
  format: GeminiVocabularyFormat;
  parsedTerms: string[];
  normalizedTerms: string[];
  warnings: GeminiVocabularyIssue[];
  errors: GeminiVocabularyIssue[];
  safeToPersist: boolean;
}

function parseCsv(input: string): { values: string[]; error?: GeminiVocabularyIssue } {
  const values: string[] = [];
  let value = "";
  let quoted = false;
  let afterQuote = false;
  for (let index = 0; index < input.length; index += 1) {
    const char = input[index];
    if (quoted) {
      if (char === '"') {
        if (input[index + 1] === '"') {
          value += '"';
          index += 1;
        } else {
          quoted = false;
          afterQuote = true;
        }
      } else {
        value += char;
      }
      continue;
    }
    if (afterQuote && char !== "," && !/\s/.test(char)) {
      return { values, error: { code: "csv_after_quote", message: "Unexpected text after a closing quote.", position: index + 1 } };
    }
    if (char === '"') {
      if (value.trim().length > 0) {
        return { values, error: { code: "csv_quote", message: "A quote must begin at the start of a CSV term.", position: index + 1 } };
      }
      quoted = true;
      afterQuote = false;
    } else if (char === ",") {
      values.push(value);
      value = "";
      afterQuote = false;
    } else {
      value += char;
    }
  }
  if (quoted) {
    return { values, error: { code: "csv_unclosed_quote", message: "A quoted CSV term is missing its closing quote.", position: input.length } };
  }
  values.push(value);
  return { values };
}

export function parseGeminiVocabulary(input: string): GeminiVocabularyParseResult {
  const trimmedInput = input.trim();
  if (!trimmedInput) {
    return { format: "empty", parsedTerms: [], normalizedTerms: [], warnings: [], errors: [], safeToPersist: true };
  }

  let format: GeminiVocabularyFormat;
  let parsedTerms: string[] = [];
  const errors: GeminiVocabularyIssue[] = [];
  const warnings: GeminiVocabularyIssue[] = [];

  if (trimmedInput.startsWith("[") || trimmedInput.startsWith("{")) {
    format = "json";
    try {
      const parsed: unknown = JSON.parse(trimmedInput);
      if (!Array.isArray(parsed)) {
        errors.push({ code: "json_not_array", message: "JSON vocabulary must be an array of strings." });
      } else {
        parsed.forEach((term, index) => {
          if (typeof term !== "string") {
            errors.push({ code: "json_non_string", message: `JSON item ${index + 1} must be a string.`, value: String(term), position: index + 1 });
          } else {
            parsedTerms.push(term);
          }
        });
      }
    } catch (error) {
      errors.push({ code: "json_syntax", message: `Malformed JSON: ${error instanceof Error ? error.message : String(error)}` });
    }
  } else if (trimmedInput.includes("\n") && trimmedInput.includes(",")) {
    format = "unknown";
    errors.push({ code: "mixed_format", message: "Do not mix newline-separated and comma-separated formats in one vocabulary draft." });
  } else if (trimmedInput.includes("\n")) {
    format = "newline";
    parsedTerms = trimmedInput.replace(/\r\n?/g, "\n").split("\n");
  } else if (trimmedInput.includes(",") || trimmedInput.includes('"')) {
    format = "csv";
    const csv = parseCsv(trimmedInput);
    parsedTerms = csv.values;
    if (csv.error) errors.push(csv.error);
  } else {
    format = "newline";
    parsedTerms = [trimmedInput];
  }

  const normalizedTerms: string[] = [];
  parsedTerms.forEach((term, index) => {
    const normalized = term.trim();
    if (!normalized) {
      errors.push({ code: "empty_term", message: `Term ${index + 1} is empty. Remove the malformed separator or blank line.`, position: index + 1 });
      return;
    }
    const duplicateIndex = normalizedTerms.indexOf(normalized);
    if (duplicateIndex >= 0) {
      warnings.push({ code: "duplicate", message: `Duplicate term '${normalized}' was ignored; the first occurrence is retained.`, value: normalized, position: index + 1 });
      return;
    }
    normalizedTerms.push(normalized);
  });

  if (normalizedTerms.length > GEMINI_VOCABULARY_HARD_MAX) {
    errors.push({ code: "too_many_terms", message: `${normalizedTerms.length} terms exceed Gemini's ${GEMINI_VOCABULARY_HARD_MAX}-term maximum.` });
  } else if (normalizedTerms.length > GEMINI_VOCABULARY_RECOMMENDED_MAX) {
    warnings.push({ code: "recommended_limit", message: `Gemini accepts this vocabulary, but best results are typically achieved with up to ${GEMINI_VOCABULARY_RECOMMENDED_MAX} terms.` });
  }

  return { format, parsedTerms, normalizedTerms, warnings, errors, safeToPersist: errors.length === 0 };
}
