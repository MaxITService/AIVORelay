import type { TextReplacementRule } from "./textReplacementRuleView";

export const TEXT_REPLACEMENT_TRANSFER_FORMAT =
  "aivorelay-text-replacements" as const;
export const TEXT_REPLACEMENT_TRANSFER_VERSION = 1 as const;

export type TextReplacementTransferErrorCode =
  | "invalid-json"
  | "invalid-document"
  | "unsupported-format"
  | "unsupported-version"
  | "invalid-rules"
  | "invalid-rule"
  | "id-generation";

export class TextReplacementTransferError extends Error {
  readonly code: TextReplacementTransferErrorCode;
  readonly ruleIndex?: number;

  constructor(
    code: TextReplacementTransferErrorCode,
    message: string,
    ruleIndex?: number,
  ) {
    super(message);
    this.name = "TextReplacementTransferError";
    this.code = code;
    this.ruleIndex = ruleIndex;
  }
}

export interface TextReplacementTransferEnvelope {
  format: typeof TEXT_REPLACEMENT_TRANSFER_FORMAT;
  version: typeof TEXT_REPLACEMENT_TRANSFER_VERSION;
  rules: TextReplacementRule[];
}

export type TextReplacementRuleIdFactory = (
  originalId: string,
  attempt: number,
) => string;

export type TextReplacementImportMode = "replace" | "merge";

export interface TextReplacementImportOptions {
  mode: TextReplacementImportMode;
  overwriteConflicts: boolean;
  idFactory?: TextReplacementRuleIdFactory;
}

export interface TextReplacementImportResult {
  rules: TextReplacementRule[];
  /** Number of imported source rules applied, including overwrites. */
  importedCount: number;
  /** Number of imported rules appended to the result. */
  addedCount: number;
  /** Number of distinct result positions overwritten by conflict handling. */
  overwrittenConflictCount: number;
  skippedDuplicateCount: number;
  skippedConflictCount: number;
  /** Number of imported or legacy-existing IDs that were remapped. */
  remappedIdCount: number;
}

const hasOwn = (value: object, key: PropertyKey): boolean =>
  Object.prototype.hasOwnProperty.call(value, key);

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const invalidRule = (
  ruleIndex: number,
  detail: string,
): TextReplacementTransferError =>
  new TextReplacementTransferError(
    "invalid-rule",
    `Invalid replacement rule at index ${ruleIndex}: ${detail}`,
    ruleIndex,
  );

const readOptionalBoolean = (
  rule: Record<string, unknown>,
  field: "enabled" | "case_sensitive" | "is_regex",
  defaultValue: boolean,
  ruleIndex: number,
): boolean => {
  if (!hasOwn(rule, field)) {
    return defaultValue;
  }

  if (typeof rule[field] !== "boolean") {
    throw invalidRule(ruleIndex, `${field} must be a boolean`);
  }

  return rule[field] as boolean;
};

const parseRule = (value: unknown, ruleIndex: number): TextReplacementRule => {
  if (!isRecord(value)) {
    throw invalidRule(ruleIndex, "rule must be an object");
  }

  if (typeof value.id !== "string" || value.id.length === 0) {
    throw invalidRule(ruleIndex, "id must be a non-empty string");
  }
  if (typeof value.from !== "string" || value.from.length === 0) {
    throw invalidRule(ruleIndex, "from must be a non-empty string");
  }
  if (typeof value.to !== "string") {
    throw invalidRule(ruleIndex, "to must be a string");
  }

  return {
    id: value.id,
    from: value.from,
    to: value.to,
    enabled: readOptionalBoolean(value, "enabled", true, ruleIndex),
    case_sensitive: readOptionalBoolean(
      value,
      "case_sensitive",
      true,
      ruleIndex,
    ),
    is_regex: readOptionalBoolean(value, "is_regex", false, ruleIndex),
  };
};

export const serializeTextReplacementRules = (
  rules: readonly TextReplacementRule[],
): string => {
  const envelope: TextReplacementTransferEnvelope = {
    format: TEXT_REPLACEMENT_TRANSFER_FORMAT,
    version: TEXT_REPLACEMENT_TRANSFER_VERSION,
    rules: rules.map((rule) => ({
      id: rule.id,
      from: rule.from,
      to: rule.to,
      enabled: rule.enabled,
      case_sensitive: rule.case_sensitive,
      is_regex: rule.is_regex,
    })),
  };

  return `${JSON.stringify(envelope, null, 2)}\n`;
};

export const parseTextReplacementRulesJson = (
  document: string,
): TextReplacementRule[] => {
  let parsed: unknown;
  try {
    const documentWithoutBom = document.startsWith("\uFEFF")
      ? document.slice(1)
      : document;
    parsed = JSON.parse(documentWithoutBom) as unknown;
  } catch {
    throw new TextReplacementTransferError(
      "invalid-json",
      "The replacement-rules document is not valid JSON",
    );
  }

  let rawRules: unknown;
  if (Array.isArray(parsed)) {
    rawRules = parsed;
  } else if (isRecord(parsed)) {
    if (parsed.format !== TEXT_REPLACEMENT_TRANSFER_FORMAT) {
      throw new TextReplacementTransferError(
        "unsupported-format",
        "The replacement-rules document has an unsupported format",
      );
    }
    if (
      !Number.isInteger(parsed.version) ||
      parsed.version !== TEXT_REPLACEMENT_TRANSFER_VERSION
    ) {
      throw new TextReplacementTransferError(
        "unsupported-version",
        "The replacement-rules document has an unsupported version",
      );
    }
    rawRules = parsed.rules;
  } else {
    throw new TextReplacementTransferError(
      "invalid-document",
      "The replacement-rules document must be an array or versioned envelope",
    );
  }

  if (!Array.isArray(rawRules)) {
    throw new TextReplacementTransferError(
      "invalid-rules",
      "The replacement-rules document must contain a rules array",
    );
  }

  return rawRules.map(parseRule);
};

const behavioralSignature = (rule: TextReplacementRule): string =>
  JSON.stringify([
    rule.from,
    rule.to,
    rule.enabled,
    rule.case_sensitive,
    rule.is_regex,
  ]);

const conflictSignature = (rule: TextReplacementRule): string =>
  JSON.stringify([rule.from, rule.case_sensitive, rule.is_regex]);

const defaultTextReplacementRuleIdFactory: TextReplacementRuleIdFactory = (
  originalId,
  attempt,
) =>
  attempt === 0
    ? `tr_import_${originalId}`
    : `tr_import_${originalId}_${attempt}`;

const incrementCount = (counts: Map<string, number>, key: string): void => {
  counts.set(key, (counts.get(key) ?? 0) + 1);
};

const decrementCount = (counts: Map<string, number>, key: string): void => {
  const nextCount = (counts.get(key) ?? 0) - 1;
  if (nextCount <= 0) {
    counts.delete(key);
  } else {
    counts.set(key, nextCount);
  }
};

const createUniqueId = (
  originalId: string,
  usedIds: ReadonlySet<string>,
  idFactory: TextReplacementRuleIdFactory,
): string => {
  for (let attempt = 0; attempt < 10000; attempt += 1) {
    let candidate: string;
    try {
      candidate = idFactory(originalId, attempt);
    } catch {
      throw new TextReplacementTransferError(
        "id-generation",
        "Could not generate a unique replacement-rule ID",
      );
    }

    if (typeof candidate !== "string" || candidate.length === 0) {
      throw new TextReplacementTransferError(
        "id-generation",
        "The replacement-rule ID factory returned an invalid ID",
      );
    }
    if (!usedIds.has(candidate)) {
      return candidate;
    }
  }

  throw new TextReplacementTransferError(
    "id-generation",
    "Could not generate a unique replacement-rule ID",
  );
};

/**
 * Applies a validated import without mutating either input array or its rules.
 * Merge conflicts are classified after exact duplicates; when overwriting is
 * enabled, later imported rules win while existing slot IDs and positions stay
 * unchanged.
 */
export const applyTextReplacementImport = (
  existingRules: readonly TextReplacementRule[],
  importedRules: readonly TextReplacementRule[],
  options: TextReplacementImportOptions,
): TextReplacementImportResult => {
  const idFactory = options.idFactory ?? defaultTextReplacementRuleIdFactory;
  const resultRules: TextReplacementRule[] = [];
  const usedIds = new Set<string>();
  const unavailableExistingIds =
    options.mode === "merge"
      ? new Set(existingRules.map((rule) => rule.id))
      : new Set<string>();
  let remappedIdCount = 0;

  if (options.mode === "merge") {
    for (const rule of existingRules) {
      let nextRule = rule;
      if (usedIds.has(rule.id)) {
        const uniqueId = createUniqueId(
          rule.id,
          unavailableExistingIds,
          idFactory,
        );
        nextRule = { ...rule, id: uniqueId };
        unavailableExistingIds.add(uniqueId);
        remappedIdCount += 1;
      }

      resultRules.push(nextRule);
      usedIds.add(nextRule.id);
      unavailableExistingIds.add(nextRule.id);
    }
  }

  const behaviorCounts = new Map<string, number>();
  const conflictIndices = new Map<string, number[]>();

  resultRules.forEach((rule, index) => {
    incrementCount(behaviorCounts, behavioralSignature(rule));
    const key = conflictSignature(rule);
    const indices = conflictIndices.get(key) ?? [];
    indices.push(index);
    conflictIndices.set(key, indices);
  });

  let importedCount = 0;
  let addedCount = 0;
  const overwrittenIndices = new Set<number>();
  let skippedDuplicateCount = 0;
  let skippedConflictCount = 0;

  for (const rule of importedRules) {
    const signature = behavioralSignature(rule);
    if (behaviorCounts.has(signature)) {
      skippedDuplicateCount += 1;
      continue;
    }

    const key = conflictSignature(rule);
    const conflictingIndices = conflictIndices.get(key) ?? [];
    if (
      options.mode === "merge" &&
      conflictingIndices.length > 0
    ) {
      if (!options.overwriteConflicts) {
        skippedConflictCount += 1;
        continue;
      }

      for (const index of conflictingIndices) {
        const currentRule = resultRules[index];
        decrementCount(behaviorCounts, behavioralSignature(currentRule));
        resultRules[index] = {
          id: currentRule.id,
          from: rule.from,
          to: rule.to,
          enabled: rule.enabled,
          case_sensitive: rule.case_sensitive,
          is_regex: rule.is_regex,
        };
        incrementCount(behaviorCounts, signature);
        overwrittenIndices.add(index);
      }
      importedCount += 1;
      continue;
    }

    let nextRule = rule;
    if (usedIds.has(rule.id)) {
      const uniqueId = createUniqueId(rule.id, usedIds, idFactory);
      nextRule = { ...rule, id: uniqueId };
      remappedIdCount += 1;
    }

    const nextIndex = resultRules.length;
    resultRules.push(nextRule);
    usedIds.add(nextRule.id);
    incrementCount(behaviorCounts, signature);
    const indices = conflictIndices.get(key) ?? [];
    indices.push(nextIndex);
    conflictIndices.set(key, indices);
    importedCount += 1;
    addedCount += 1;
  }

  return {
    rules: resultRules,
    importedCount,
    addedCount,
    overwrittenConflictCount: overwrittenIndices.size,
    skippedDuplicateCount,
    skippedConflictCount,
    remappedIdCount,
  };
};
