import assert from "node:assert";

import {
  applyTextReplacementImport,
  parseTextReplacementRulesJson,
  serializeTextReplacementRules,
  TextReplacementTransferError,
} from "./textReplacementRuleTransfer";
import type { TextReplacementRule } from "./textReplacementRuleView";

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

const simpleRule = makeRule("first", "Hello", "Hi");
const secondRule = makeRule("second", "World", "Earth", {
  enabled: false,
  case_sensitive: false,
  is_regex: true,
});

const expectedSimpleJson = `{
  "format": "aivorelay-text-replacements",
  "version": 1,
  "rules": [
    {
      "id": "first",
      "from": "Hello",
      "to": "Hi",
      "enabled": true,
      "case_sensitive": true,
      "is_regex": false
    },
    {
      "id": "second",
      "from": "World",
      "to": "Earth",
      "enabled": false,
      "case_sensitive": false,
      "is_regex": true
    }
  ]
}
`;

const serialized = serializeTextReplacementRules([simpleRule, secondRule]);
assert.strictEqual(serialized, expectedSimpleJson);
assert.deepStrictEqual(parseTextReplacementRulesJson(serialized), [
  simpleRule,
  secondRule,
]);
assert.deepStrictEqual(
  parseTextReplacementRulesJson(JSON.stringify([simpleRule, secondRule])),
  [simpleRule, secondRule],
);
assert.deepStrictEqual(
  parseTextReplacementRulesJson(`\uFEFF${serialized}`),
  [simpleRule, secondRule],
);
assert.deepStrictEqual(
  parseTextReplacementRulesJson(`\uFEFF${JSON.stringify([simpleRule])}`),
  [simpleRule],
);

const actualLf = "\n";
const actualCr = "\r";
const actualCrLf = "\r\n";
const actualTab = "\t";
const literalBackslashN = "\\n";
const literalBackslashR = "\\r";
const literalBackslashRBackslashN = "\\r\\n";
const literalBackslashT = "\\t";
const literalBackslash = "\\";
const doubledBackslashes = "\\\\";
const realZeroWidthJoiner = "\u200D";
const specialFrom = [
  "actual-lf:",
  actualLf,
  "actual-cr:",
  actualCr,
  "actual-crlf:",
  actualCrLf,
  "actual-tab:",
  actualTab,
  "literal-backslash-n:",
  literalBackslashN,
  "literal-backslash-r:",
  literalBackslashR,
  "literal-backslash-r-backslash-n:",
  literalBackslashRBackslashN,
  "literal-backslash-t:",
  literalBackslashT,
  "literal-backslash:",
  literalBackslash,
  "doubled-backslashes:",
  doubledBackslashes,
  "quotes-and-json:{\"key\": [true, null]}",
  "Cyrillic: Привет",
  "emoji: 🧪",
  "zwj: 👩",
  realZeroWidthJoiner,
  "💻",
  "regex:(foo)\\d+(bar)\\s+\\b(?<name>baz)\\b",
].join("");
const specialTo = [
  "$1/${1}/${name}/$$",
  "actual-lf:",
  actualLf,
  "actual-cr:",
  actualCr,
  "actual-crlf:",
  actualCrLf,
  "actual-tab:",
  actualTab,
  "literal-backslash-n:",
  literalBackslashN,
  "literal-backslash-r:",
  literalBackslashR,
  "literal-backslash-r-backslash-n:",
  literalBackslashRBackslashN,
  "literal-backslash-t:",
  literalBackslashT,
  "literal-backslash:",
  literalBackslash,
  "doubled-backslashes:",
  doubledBackslashes,
  "quotes: \"quoted\"",
].join("");
const specialRule = makeRule(
  "special",
  specialFrom,
  specialTo,
  { is_regex: true },
);
const specialRoundTrip = parseTextReplacementRulesJson(
  serializeTextReplacementRules([specialRule]),
)[0];
assert.deepStrictEqual(specialRoundTrip, specialRule);
assert.strictEqual(specialRoundTrip.from, specialFrom);
assert.strictEqual(specialRoundTrip.to, specialTo);
assert.ok(specialRoundTrip.from.includes(`actual-lf:${actualLf}`));
assert.ok(specialRoundTrip.from.includes(`actual-cr:${actualCr}`));
assert.ok(specialRoundTrip.from.includes(`actual-crlf:${actualCrLf}`));
assert.ok(specialRoundTrip.from.includes(`actual-tab:${actualTab}`));
assert.ok(specialRoundTrip.from.includes(`literal-backslash-n:${literalBackslashN}`));
assert.ok(specialRoundTrip.from.includes(`literal-backslash-r:${literalBackslashR}`));
assert.ok(
  specialRoundTrip.from.includes(
    `literal-backslash-r-backslash-n:${literalBackslashRBackslashN}`,
  ),
);
assert.ok(specialRoundTrip.from.includes(`literal-backslash-t:${literalBackslashT}`));
assert.ok(specialRoundTrip.from.includes(`literal-backslash:${literalBackslash}`));
assert.ok(specialRoundTrip.from.includes(`doubled-backslashes:${doubledBackslashes}`));
assert.ok(specialRoundTrip.from.includes("quotes-and-json:{\"key\": [true, null]}"));
assert.ok(specialRoundTrip.from.includes("Cyrillic: Привет"));
assert.ok(specialRoundTrip.from.includes("emoji: 🧪"));
assert.ok(specialRoundTrip.from.includes(`zwj: 👩${realZeroWidthJoiner}💻`));
assert.ok(specialRoundTrip.from.includes("regex:(foo)\\d+(bar)\\s+\\b(?<name>baz)\\b"));
assert.ok(specialRoundTrip.to.includes("$1/${1}/${name}/$$"));

const emptyReplacementRule = parseTextReplacementRulesJson(
  serializeTextReplacementRules([makeRule("empty-replacement", "remove", "")]),
)[0];
assert.strictEqual(emptyReplacementRule.to, "");

const defaults = parseTextReplacementRulesJson(
  JSON.stringify([
    {
      id: "defaults",
      from: "  literal whitespace  ",
      to: "",
    },
  ]),
);
assert.deepStrictEqual(defaults, [
  makeRule("defaults", "  literal whitespace  ", ""),
]);

const assertTransferError = (
  document: string,
  code: TextReplacementTransferError["code"],
) => {
  assert.throws(
    () => parseTextReplacementRulesJson(document),
    (error: unknown) =>
      error instanceof TextReplacementTransferError && error.code === code,
  );
};

assertTransferError("{", "invalid-json");
assertTransferError(
  JSON.stringify({ format: "other", version: 1, rules: [] }),
  "unsupported-format",
);
assertTransferError(
  JSON.stringify({ format: "aivorelay-text-replacements", version: 2, rules: [] }),
  "unsupported-version",
);
assertTransferError(JSON.stringify({ format: "aivorelay-text-replacements", version: 1 }), "invalid-rules");
assertTransferError(JSON.stringify([null]), "invalid-rule");
assertTransferError(JSON.stringify([{ id: "", from: "x", to: "y" }]), "invalid-rule");
assertTransferError(JSON.stringify([{ id: "id", from: "", to: "y" }]), "invalid-rule");
assertTransferError(JSON.stringify([{ id: "id", from: "x", to: 3 }]), "invalid-rule");
assertTransferError(JSON.stringify([{ id: "id", from: "x", to: "y", enabled: "yes" }]), "invalid-rule");
assertTransferError(JSON.stringify([{ id: "id", from: "x", to: "y", case_sensitive: 1 }]), "invalid-rule");
assertTransferError(JSON.stringify([{ id: "id", from: "x", to: "y", is_regex: null }]), "invalid-rule");

const existingRule = makeRule("existing", "one", "two");
const duplicateRule = makeRule("different-id", "one", "two");
const conflictRule = makeRule("conflict-id", "one", "changed", {
  case_sensitive: true,
});
const collidingRule = makeRule("existing", "three", "four");
const uniqueRule = makeRule("unique", "five", "six");
const duplicateImportedRule = makeRule("another-id", "three", "four");
const laterConflictRule = makeRule("later-conflict", "five", "changed");
const existingSnapshot = { ...existingRule };
const importedRules = [
  duplicateRule,
  conflictRule,
  collidingRule,
  uniqueRule,
  duplicateImportedRule,
  laterConflictRule,
];
const importedSnapshot = importedRules.map((rule) => ({ ...rule }));
const attempts: Array<[string, number]> = [];
const mergeResult = applyTextReplacementImport(
  [existingRule],
  importedRules,
  {
    mode: "merge",
    overwriteConflicts: false,
    idFactory: (originalId, attempt) => {
      attempts.push([originalId, attempt]);
      return attempt === 0 ? "existing" : `generated-${attempt}`;
    },
  },
);
assert.deepStrictEqual(
  mergeResult.rules.map((rule) => rule.id),
  ["existing", "generated-1", "unique"],
);
assert.strictEqual(mergeResult.importedCount, 2);
assert.strictEqual(mergeResult.addedCount, 2);
assert.strictEqual(mergeResult.overwrittenConflictCount, 0);
assert.strictEqual(mergeResult.skippedDuplicateCount, 2);
assert.strictEqual(mergeResult.skippedConflictCount, 2);
assert.strictEqual(mergeResult.remappedIdCount, 1);
assert.deepStrictEqual(attempts, [["existing", 0], ["existing", 1]]);
assert.strictEqual(mergeResult.rules[0], existingRule);
assert.strictEqual(mergeResult.rules[2], uniqueRule);
assert.notStrictEqual(mergeResult.rules[1], collidingRule);
assert.deepStrictEqual(existingRule, existingSnapshot);
assert.deepStrictEqual(importedRules, importedSnapshot);
assert.deepStrictEqual(importedRules.map((rule) => rule.id), [
  "different-id",
  "conflict-id",
  "existing",
  "unique",
  "another-id",
  "later-conflict",
]);

const replaceDuplicateIdRules = [
  makeRule("same-id", "replace-a", "A"),
  makeRule("same-id", "replace-b", "B"),
  makeRule("same-id", "replace-b", "B"),
];
const replaceSnapshot = replaceDuplicateIdRules.map((rule) => ({ ...rule }));
const replaceResult = applyTextReplacementImport(
  [makeRule("same-id", "old", "rule")],
  replaceDuplicateIdRules,
  {
    mode: "replace",
    overwriteConflicts: true,
    idFactory: (_originalId, attempt) => `replacement-${attempt}`,
  },
);
assert.deepStrictEqual(
  replaceResult.rules.map((rule) => rule.id),
  ["same-id", "replacement-0"],
);
assert.deepStrictEqual(
  replaceResult.rules.map((rule) => rule.from),
  ["replace-a", "replace-b"],
);
assert.strictEqual(replaceResult.importedCount, 2);
assert.strictEqual(replaceResult.addedCount, 2);
assert.strictEqual(replaceResult.overwrittenConflictCount, 0);
assert.strictEqual(replaceResult.skippedDuplicateCount, 1);
assert.strictEqual(replaceResult.skippedConflictCount, 0);
assert.strictEqual(replaceResult.remappedIdCount, 1);
assert.deepStrictEqual(replaceDuplicateIdRules, replaceSnapshot);

const replaceEmptyResult = applyTextReplacementImport(
  [],
  [makeRule("fresh", "fresh-from", "fresh-to")],
  {
    mode: "replace",
    overwriteConflicts: false,
  },
);
assert.deepStrictEqual(
  replaceEmptyResult.rules.map((rule) => rule.id),
  ["fresh"],
);
assert.strictEqual(replaceEmptyResult.importedCount, 1);
assert.strictEqual(replaceEmptyResult.addedCount, 1);

const overwriteExisting = makeRule("slot-id", "same-key", "old");
const overwriteOther = makeRule("other-id", "other-key", "keep");
const overwriteFirst = makeRule("import-one", "same-key", "first");
const overwriteLast = makeRule("import-two", "same-key", "last");
const overwriteDuplicate = makeRule("import-three", "same-key", "last");
const overwriteAppend = makeRule("other-id", "append-key", "append");
const overwriteExistingSnapshot = { ...overwriteExisting };
const overwriteOtherSnapshot = { ...overwriteOther };
const overwriteImports = [
  overwriteFirst,
  overwriteLast,
  overwriteDuplicate,
  overwriteAppend,
];
const overwriteImportsSnapshot = overwriteImports.map((rule) => ({ ...rule }));
const overwriteResult = applyTextReplacementImport(
  [overwriteExisting, overwriteOther],
  overwriteImports,
  {
    mode: "merge",
    overwriteConflicts: true,
    idFactory: (_originalId, attempt) =>
      attempt === 0 ? "other-id" : `remapped-${attempt}`,
  },
);
assert.deepStrictEqual(
  overwriteResult.rules.map((rule) => rule.id),
  ["slot-id", "other-id", "remapped-1"],
);
assert.deepStrictEqual(
  overwriteResult.rules.map((rule) => rule.to),
  ["last", "keep", "append"],
);
assert.strictEqual(overwriteResult.importedCount, 3);
assert.strictEqual(overwriteResult.addedCount, 1);
assert.strictEqual(overwriteResult.overwrittenConflictCount, 1);
assert.strictEqual(overwriteResult.skippedDuplicateCount, 1);
assert.strictEqual(overwriteResult.skippedConflictCount, 0);
assert.strictEqual(overwriteResult.remappedIdCount, 1);
assert.strictEqual(overwriteResult.rules[0].id, overwriteExisting.id);
assert.deepStrictEqual(overwriteExisting, overwriteExistingSnapshot);
assert.deepStrictEqual(overwriteOther, overwriteOtherSnapshot);
assert.deepStrictEqual(overwriteImports, overwriteImportsSnapshot);

const legacyFirst = makeRule("legacy-id", "legacy-one", "one");
const legacyDuplicate = makeRule("legacy-id", "legacy-two", "two");
const legacyImported = makeRule("legacy-import", "new-rule", "new");
const legacyExistingSnapshot = [
  { ...legacyFirst },
  { ...legacyDuplicate },
];
const legacyImportedSnapshot = { ...legacyImported };
const legacyRepairResult = applyTextReplacementImport(
  [legacyFirst, legacyDuplicate],
  [legacyImported],
  {
    mode: "merge",
    overwriteConflicts: false,
    idFactory: (_originalId, attempt) =>
      attempt === 0 ? "legacy-id" : `legacy-repaired-${attempt}`,
  },
);
assert.deepStrictEqual(
  legacyRepairResult.rules.map((rule) => rule.id),
  ["legacy-id", "legacy-repaired-1", "legacy-import"],
);
assert.deepStrictEqual(
  legacyRepairResult.rules.map((rule) => [rule.from, rule.to]),
  [
    ["legacy-one", "one"],
    ["legacy-two", "two"],
    ["new-rule", "new"],
  ],
);
assert.strictEqual(legacyRepairResult.importedCount, 1);
assert.strictEqual(legacyRepairResult.addedCount, 1);
assert.strictEqual(legacyRepairResult.remappedIdCount, 1);
assert.strictEqual(legacyRepairResult.rules[0], legacyFirst);
assert.notStrictEqual(legacyRepairResult.rules[1], legacyDuplicate);
assert.strictEqual(legacyRepairResult.rules[2], legacyImported);
assert.deepStrictEqual([legacyFirst, legacyDuplicate], legacyExistingSnapshot);
assert.deepStrictEqual(legacyImported, legacyImportedSnapshot);
assert.strictEqual(
  new Set(legacyRepairResult.rules.map((rule) => rule.id)).size,
  legacyRepairResult.rules.length,
);

const reservedFirst = makeRule("x", "reserved-one", "one");
const reservedDuplicate = makeRule("x", "reserved-two", "two");
const reservedLater = makeRule("tr_import_x", "reserved-three", "three");
const reservedImported = makeRule("reserved-import", "reserved-four", "four");
const reservedExistingSnapshot = [
  { ...reservedFirst },
  { ...reservedDuplicate },
  { ...reservedLater },
];
const reservedImportedSnapshot = { ...reservedImported };
const reservedResult = applyTextReplacementImport(
  [reservedFirst, reservedDuplicate, reservedLater],
  [reservedImported],
  {
    mode: "merge",
    overwriteConflicts: false,
  },
);
assert.deepStrictEqual(
  reservedResult.rules.map((rule) => rule.id),
  ["x", "tr_import_x_1", "tr_import_x", "reserved-import"],
);
assert.strictEqual(reservedResult.remappedIdCount, 1);
assert.strictEqual(reservedResult.rules[0], reservedFirst);
assert.notStrictEqual(reservedResult.rules[1], reservedDuplicate);
assert.strictEqual(reservedResult.rules[2], reservedLater);
assert.strictEqual(reservedResult.rules[3], reservedImported);
assert.deepStrictEqual(
  [reservedFirst, reservedDuplicate, reservedLater],
  reservedExistingSnapshot,
);
assert.deepStrictEqual(reservedImported, reservedImportedSnapshot);
assert.strictEqual(
  new Set(reservedResult.rules.map((rule) => rule.id)).size,
  reservedResult.rules.length,
);

const noOpLegacyFirst = makeRule("no-op-id", "no-op", "value");
const noOpLegacyDuplicate = makeRule("no-op-id", "other", "other-value");
const noOpImportedDuplicate = makeRule("import-no-op", "no-op", "value");
const noOpExistingSnapshot = [
  { ...noOpLegacyFirst },
  { ...noOpLegacyDuplicate },
];
const noOpImportedSnapshot = { ...noOpImportedDuplicate };
const noOpResult = applyTextReplacementImport(
  [noOpLegacyFirst, noOpLegacyDuplicate],
  [noOpImportedDuplicate],
  {
    mode: "merge",
    overwriteConflicts: false,
  },
);
assert.strictEqual(noOpResult.importedCount, 0);
assert.strictEqual(noOpResult.addedCount, 0);
assert.strictEqual(noOpResult.skippedDuplicateCount, 1);
assert.strictEqual(noOpResult.remappedIdCount, 1);
assert.deepStrictEqual(
  noOpResult.rules.map((rule) => rule.id),
  ["no-op-id", "tr_import_no-op-id"],
);
assert.deepStrictEqual(noOpResult.rules, [
  { ...noOpLegacyFirst },
  { ...noOpLegacyDuplicate, id: "tr_import_no-op-id" },
]);
assert.strictEqual(noOpResult.rules[0], noOpLegacyFirst);
assert.notStrictEqual(noOpResult.rules[1], noOpLegacyDuplicate);
assert.deepStrictEqual([noOpLegacyFirst, noOpLegacyDuplicate], noOpExistingSnapshot);
assert.deepStrictEqual(noOpImportedDuplicate, noOpImportedSnapshot);
assert.strictEqual(
  new Set(noOpResult.rules.map((rule) => rule.id)).size,
  noOpResult.rules.length,
);

const appendedBase = makeRule("append-base", "base", "base");
const appendedFirst = makeRule("append-first", "new-key", "first");
const appendedLast = makeRule("append-last", "new-key", "last");
const appendedImportsSnapshot = [
  { ...appendedFirst },
  { ...appendedLast },
];
const appendedResult = applyTextReplacementImport(
  [appendedBase],
  [appendedFirst, appendedLast],
  {
    mode: "merge",
    overwriteConflicts: true,
  },
);
assert.deepStrictEqual(
  appendedResult.rules.map((rule) => rule.id),
  ["append-base", "append-first"],
);
assert.deepStrictEqual(
  appendedResult.rules.map((rule) => [rule.from, rule.to]),
  [
    ["base", "base"],
    ["new-key", "last"],
  ],
);
assert.strictEqual(appendedResult.importedCount, 2);
assert.strictEqual(appendedResult.addedCount, 1);
assert.strictEqual(appendedResult.overwrittenConflictCount, 1);
assert.strictEqual(appendedResult.remappedIdCount, 0);
assert.strictEqual(appendedResult.rules[0], appendedBase);
assert.deepStrictEqual(
  [appendedFirst, appendedLast],
  appendedImportsSnapshot,
);

const duplicateConflictFirst = makeRule(
  "duplicate-conflict-id",
  "duplicate-key",
  "old-first",
);
const duplicateConflictSecond = makeRule(
  "duplicate-conflict-id",
  "duplicate-key",
  "old-second",
);
const duplicateConflictImports = [
  makeRule("import-first", "duplicate-key", "new-first"),
  makeRule("import-last", "duplicate-key", "new-last"),
];
const duplicateConflictExistingSnapshot = [
  { ...duplicateConflictFirst },
  { ...duplicateConflictSecond },
];
const duplicateConflictImportsSnapshot = duplicateConflictImports.map((rule) => ({
  ...rule,
}));
const duplicateConflictResult = applyTextReplacementImport(
  [duplicateConflictFirst, duplicateConflictSecond],
  duplicateConflictImports,
  {
    mode: "merge",
    overwriteConflicts: true,
    idFactory: (_originalId, attempt) => `repaired-conflict-${attempt}`,
  },
);
assert.deepStrictEqual(
  duplicateConflictResult.rules.map((rule) => rule.id),
  ["duplicate-conflict-id", "repaired-conflict-0"],
);
assert.deepStrictEqual(
  duplicateConflictResult.rules.map((rule) => [rule.from, rule.to]),
  [
    ["duplicate-key", "new-last"],
    ["duplicate-key", "new-last"],
  ],
);
assert.strictEqual(duplicateConflictResult.importedCount, 2);
assert.strictEqual(duplicateConflictResult.addedCount, 0);
assert.strictEqual(duplicateConflictResult.overwrittenConflictCount, 2);
assert.strictEqual(duplicateConflictResult.skippedDuplicateCount, 0);
assert.strictEqual(duplicateConflictResult.skippedConflictCount, 0);
assert.strictEqual(duplicateConflictResult.remappedIdCount, 1);
assert.deepStrictEqual(
  [duplicateConflictFirst, duplicateConflictSecond],
  duplicateConflictExistingSnapshot,
);
assert.deepStrictEqual(duplicateConflictImports, duplicateConflictImportsSnapshot);
assert.strictEqual(
  new Set(duplicateConflictResult.rules.map((rule) => rule.id)).size,
  duplicateConflictResult.rules.length,
);
