import assert from "node:assert";
import { expect, test } from "bun:test";
import {
  applyTextReplacementImport,
  TextReplacementTransferError,
} from "./textReplacementRuleTransfer";
import type { TextReplacementRule } from "./textReplacementRuleView";

const rule = (
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

test("an ID factory failure leaves all existing and imported rules untouched", () => {
  const existing = [rule("same-id", "existing", "keep")];
  const imported = [rule("new-id", "first", "new"), rule("same-id", "second", "new")];
  const before = JSON.stringify({ existing, imported });

  assert.throws(() => applyTextReplacementImport(existing, imported, {
    mode: "merge",
    overwriteConflicts: true,
    idFactory: () => { throw new Error("ID source unavailable"); },
  }), error => error instanceof TextReplacementTransferError && error.code === "id-generation");
  expect(JSON.stringify({ existing, imported })).toBe(before);
});

test("an empty generated ID aborts import instead of creating an unaddressable rule", () => {
  const existing = [rule("same-id", "existing", "keep")];
  const imported = [rule("same-id", "new", "value")];

  assert.throws(() => applyTextReplacementImport(existing, imported, {
    mode: "merge",
    overwriteConflicts: false,
    idFactory: () => "",
  }), error => error instanceof TextReplacementTransferError && error.code === "id-generation");
  expect(existing).toEqual([rule("same-id", "existing", "keep")]);
  expect(imported).toEqual([rule("same-id", "new", "value")]);
});

test("repeated ID collisions stop at the retry limit without altering rules", () => {
  const existing = [rule("same-id", "existing", "keep")];
  const imported = [rule("same-id", "new", "value")];
  let attempts = 0;

  assert.throws(() => applyTextReplacementImport(existing, imported, {
    mode: "merge",
    overwriteConflicts: false,
    idFactory: () => {
      attempts += 1;
      return "same-id";
    },
  }), error => error instanceof TextReplacementTransferError && error.code === "id-generation");
  expect(attempts).toBe(10000);
  expect(existing).toEqual([rule("same-id", "existing", "keep")]);
  expect(imported).toEqual([rule("same-id", "new", "value")]);
});

test("conflict overwrites preserve distinct regex and case-sensitive rule variants", () => {
  const literal = rule("literal", "Word", "old");
  const insensitive = rule("insensitive", "Word", "keep-insensitive", { case_sensitive: false });
  const regex = rule("regex", "Word", "keep-regex", { is_regex: true });

  const result = applyTextReplacementImport([literal, insensitive, regex], [
    rule("incoming", "Word", "updated"),
  ], { mode: "merge", overwriteConflicts: true });

  expect(result.rules).toEqual([rule("literal", "Word", "updated"), insensitive, regex]);
  expect(result.importedCount).toBe(1);
  expect(result.overwrittenConflictCount).toBe(1);
  expect(result.addedCount).toBe(0);
  expect(literal).toEqual(rule("literal", "Word", "old"));
});

test("importing a disabled rule updates its enabled state and repeated imports are idempotent", () => {
  const enabled = rule("existing", "Word", "Replacement");
  const disabled = rule("incoming", "Word", "Replacement", { enabled: false });
  const result = applyTextReplacementImport([enabled], [disabled], {
    mode: "merge", overwriteConflicts: true,
  });

  expect(result.rules).toEqual([rule("existing", "Word", "Replacement", { enabled: false })]);
  expect(result.overwrittenConflictCount).toBe(1);
  expect(result.skippedDuplicateCount).toBe(0);
  expect(enabled.enabled).toBe(true);

  const repeated = applyTextReplacementImport(result.rules, [disabled], {
    mode: "merge", overwriteConflicts: true,
  });
  expect(repeated.rules).toEqual(result.rules);
  expect(repeated.importedCount).toBe(0);
  expect(repeated.overwrittenConflictCount).toBe(0);
  expect(repeated.skippedDuplicateCount).toBe(1);
});
