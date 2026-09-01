import { expect, test } from "bun:test";

import {
  applyTextReplacementImport,
  parseTextReplacementRulesJson,
  TextReplacementTransferError,
} from "./textReplacementRuleTransfer";
import {
  getVisibleTextReplacementRules,
  type TextReplacementRule,
} from "./textReplacementRuleView";

const makeRule = (
  id: string,
  from: string,
  to: string,
  overrides: Partial<TextReplacementRule> = {},
): TextReplacementRule => ({
  id,
  from,
  to,
  enabled: true,
  case_sensitive: true,
  is_regex: false,
  ...overrides,
});

test("validates top-level document structure and captures precise ruleIndex on invalid rule items", () => {
  // Primitive or null JSON documents must trigger 'invalid-document' error code
  expect(() => parseTextReplacementRulesJson('"plain text string"')).toThrow(
    TextReplacementTransferError,
  );
  expect(() => parseTextReplacementRulesJson("42")).toThrow(
    TextReplacementTransferError,
  );
  expect(() => parseTextReplacementRulesJson("null")).toThrow(
    TextReplacementTransferError,
  );

  try {
    parseTextReplacementRulesJson("true");
    expect.unreachable("Expected parseTextReplacementRulesJson to throw for boolean JSON");
  } catch (error) {
    expect(error).toBeInstanceOf(TextReplacementTransferError);
    if (error instanceof TextReplacementTransferError) {
      expect(error.code).toBe("invalid-document");
    }
  }

  // Schema failure in a rule inside a batch must attach the exact 0-based ruleIndex
  const batchWithInvalidItem = JSON.stringify([
    { id: "rule-0", from: "alpha", to: "beta" },
    { id: "rule-1", from: "gamma", to: "delta" },
    { id: "rule-2", from: "epsilon", to: 12345 },
  ]);

  try {
    parseTextReplacementRulesJson(batchWithInvalidItem);
    expect.unreachable("Expected parseTextReplacementRulesJson to throw for invalid item type");
  } catch (error) {
    expect(error).toBeInstanceOf(TextReplacementTransferError);
    if (error instanceof TextReplacementTransferError) {
      expect(error.code).toBe("invalid-rule");
      expect(error.ruleIndex).toBe(2);
      expect(error.message).toContain("index 2");
    }
  }
});

test("handles idFactory generation errors and tracks remapping attempt progression", () => {
  const existingRules: TextReplacementRule[] = [
    makeRule("target-id", "existing-find", "existing-replace"),
    makeRule("target-id_0", "other-find", "other-replace"),
  ];
  const importedRules: TextReplacementRule[] = [
    makeRule("target-id", "new-find", "new-replace"),
  ];

  // Factory throwing an exception must map to 'id-generation' error code
  try {
    applyTextReplacementImport(existingRules, importedRules, {
      mode: "merge",
      overwriteConflicts: false,
      idFactory: () => {
        throw new Error("ID generator exploded");
      },
    });
    expect.unreachable("Expected applyTextReplacementImport to throw on factory exception");
  } catch (error) {
    expect(error).toBeInstanceOf(TextReplacementTransferError);
    if (error instanceof TextReplacementTransferError) {
      expect(error.code).toBe("id-generation");
    }
  }

  // Factory returning an empty string must map to 'id-generation' error code
  try {
    applyTextReplacementImport(existingRules, importedRules, {
      mode: "merge",
      overwriteConflicts: false,
      idFactory: () => "",
    });
    expect.unreachable("Expected applyTextReplacementImport to throw on empty string ID");
  } catch (error) {
    expect(error).toBeInstanceOf(TextReplacementTransferError);
    if (error instanceof TextReplacementTransferError) {
      expect(error.code).toBe("id-generation");
    }
  }

  // Factory receives increasing attempt numbers until finding an unoccupied candidate
  const recordedAttempts: Array<{ originalId: string; attempt: number }> = [];

  const result = applyTextReplacementImport(existingRules, importedRules, {
    mode: "merge",
    overwriteConflicts: false,
    idFactory: (originalId, attempt) => {
      recordedAttempts.push({ originalId, attempt });
      return `${originalId}_${attempt}`;
    },
  });

  expect(recordedAttempts).toEqual([
    { originalId: "target-id", attempt: 0 },
    { originalId: "target-id", attempt: 1 },
  ]);
  expect(result.rules.map((rule) => rule.id)).toEqual([
    "target-id",
    "target-id_0",
    "target-id_1",
  ]);
  expect(result.remappedIdCount).toBe(1);
  expect(result.addedCount).toBe(1);
});

test("distinguishes enabled-state metadata conflicts from exact duplicate rules during merge import", () => {
  const existingRule = makeRule("slot-id", "target-pattern", "target-value", {
    enabled: true,
  });
  const disabledConflictRule = makeRule("incoming-id-1", "target-pattern", "target-value", {
    enabled: false,
  });
  const exactDuplicateRule = makeRule("incoming-id-2", "target-pattern", "target-value", {
    enabled: true,
  });

  // When overwriteConflicts is false, differing 'enabled' is treated as a conflict and skipped
  const skipResult = applyTextReplacementImport(
    [existingRule],
    [disabledConflictRule],
    {
      mode: "merge",
      overwriteConflicts: false,
    },
  );
  expect(skipResult.skippedConflictCount).toBe(1);
  expect(skipResult.skippedDuplicateCount).toBe(0);
  expect(skipResult.importedCount).toBe(0);
  expect(skipResult.rules[0].enabled).toBe(true);

  // When overwriteConflicts is true, differing 'enabled' overwrites existing slot in-place
  const overwriteResult = applyTextReplacementImport(
    [existingRule],
    [disabledConflictRule],
    {
      mode: "merge",
      overwriteConflicts: true,
    },
  );
  expect(overwriteResult.overwrittenConflictCount).toBe(1);
  expect(overwriteResult.skippedConflictCount).toBe(0);
  expect(overwriteResult.importedCount).toBe(1);
  expect(overwriteResult.rules[0].id).toBe("slot-id");
  expect(overwriteResult.rules[0].enabled).toBe(false);

  // Exact duplicate with identical 'enabled' state is classified under skippedDuplicateCount
  const duplicateResult = applyTextReplacementImport(
    [existingRule],
    [exactDuplicateRule],
    {
      mode: "merge",
      overwriteConflicts: false,
    },
  );
  expect(duplicateResult.skippedDuplicateCount).toBe(1);
  expect(duplicateResult.skippedConflictCount).toBe(0);
  expect(duplicateResult.importedCount).toBe(0);
  expect(duplicateResult.rules[0].enabled).toBe(true);
});

test("applies natural numeric collation and preserves stable original index ordering on tie-breaks", () => {
  const numericRules: TextReplacementRule[] = [
    makeRule("r-2", "item-2", "val-2"),
    makeRule("r-10", "item-10", "val-10"),
    makeRule("r-1", "item-1", "val-1"),
    makeRule("r-20", "item-20", "val-20"),
  ];

  // Natural numeric sorting in find-asc: item-1 < item-2 < item-10 < item-20
  const ascResult = getVisibleTextReplacementRules(
    numericRules,
    "",
    "all",
    "find-asc",
  );
  expect(ascResult.map((rule) => rule.id)).toEqual([
    "r-1",
    "r-2",
    "r-10",
    "r-20",
  ]);

  // Natural numeric sorting in find-desc: item-20 > item-10 > item-2 > item-1
  const descResult = getVisibleTextReplacementRules(
    numericRules,
    "",
    "all",
    "find-desc",
  );
  expect(descResult.map((rule) => rule.id)).toEqual([
    "r-20",
    "r-10",
    "r-2",
    "r-1",
  ]);

  // Base sensitivity tie-breaking contract:
  // Strings with equal base collation value maintain stable original index order in both asc and desc
  const tieRules: TextReplacementRule[] = [
    makeRule("base-first", "resume", "first"),
    makeRule("base-second", "résumé", "second"),
  ];
  const tieAsc = getVisibleTextReplacementRules(tieRules, "", "all", "find-asc");
  expect(tieAsc.map((rule) => rule.id)).toEqual(["base-first", "base-second"]);

  const tieDesc = getVisibleTextReplacementRules(
    tieRules,
    "",
    "all",
    "find-desc",
  );
  expect(tieDesc.map((rule) => rule.id)).toEqual(["base-first", "base-second"]);
});
