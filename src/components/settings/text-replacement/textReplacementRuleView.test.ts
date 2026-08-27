import assert from "node:assert";

import {
  getVisibleTextReplacementRules,
  type TextReplacementRule,
  type TextReplacementSearchScope,
  type TextReplacementSortOrder,
} from "./textReplacementRuleView";

const makeRule = (
  id: string,
  from: string,
  to: string,
): TextReplacementRule => ({
  id,
  from,
  to,
  enabled: true,
  case_sensitive: false,
  is_regex: false,
});

const rules: readonly TextReplacementRule[] = [
  makeRule("rule-0", "beta", "zeta"),
  makeRule("rule-1", "Alpha", "two"),
  makeRule("rule-2", "alpha", "one"),
  makeRule("rule-3", "beta", "alpha"),
  makeRule("rule-4", "gamma", "middle value"),
  makeRule("rule-5", "beta", "alpha"),
];

const originalOrder = rules.slice();
const originalValues = rules.map((rule) => ({ ...rule }));

const ids = (visibleRules: TextReplacementRule[]): string[] =>
  visibleRules.map((rule) => rule.id);

const assertView = (
  query: string,
  scope: TextReplacementSearchScope,
  sortOrder: TextReplacementSortOrder,
  expectedIds: string[],
) => {
  const visibleRules = getVisibleTextReplacementRules(
    rules,
    query,
    scope,
    sortOrder,
  );

  assert.notStrictEqual(visibleRules, rules);
  assert.deepStrictEqual(ids(visibleRules), expectedIds);
  assert.deepStrictEqual(rules, originalOrder);
  assert.deepStrictEqual(rules, originalValues);
  for (const rule of visibleRules) {
    assert.ok(rules.includes(rule));
  }
};

assertView("ALPHA", "all", "added", [
  "rule-1",
  "rule-2",
  "rule-3",
  "rule-5",
]);
assertView("ZETA", "all", "added", ["rule-0"]);
assertView("ALPHA", "replacement", "added", ["rule-3", "rule-5"]);
assertView("ALPHA", "replacement", "replacement-asc", [
  "rule-3",
  "rule-5",
]);
assertView(" ", "replacement", "added", ["rule-4"]);
assertView("", "all", "added", [
  "rule-0",
  "rule-1",
  "rule-2",
  "rule-3",
  "rule-4",
  "rule-5",
]);

assertView("", "all", "find-asc", [
  "rule-1",
  "rule-2",
  "rule-0",
  "rule-3",
  "rule-5",
  "rule-4",
]);
assertView("", "all", "find-desc", [
  "rule-4",
  "rule-0",
  "rule-3",
  "rule-5",
  "rule-1",
  "rule-2",
]);
assertView("", "all", "replacement-asc", [
  "rule-3",
  "rule-5",
  "rule-4",
  "rule-2",
  "rule-1",
  "rule-0",
]);
assertView("", "all", "replacement-desc", [
  "rule-0",
  "rule-1",
  "rule-2",
  "rule-4",
  "rule-3",
  "rule-5",
]);
