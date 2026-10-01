import { expect, test } from "bun:test";
import {
  getVisibleTextReplacementRules,
  type TextReplacementRule,
} from "./textReplacementRuleView";

const rule = (
  id: string,
  from: string,
  to: string,
  enabled = true,
): TextReplacementRule => ({
  id,
  from,
  to,
  enabled,
  case_sensitive: false,
  is_regex: false,
});

test("numeric sorting places numbered rules in natural order for both columns", () => {
  const rules = [
    rule("ten", "term 10", "value 10"),
    rule("two", "term 2", "value 2"),
    rule("one", "term 1", "value 1"),
  ];
  const snapshot = rules.map(entry => ({ ...entry }));

  expect(getVisibleTextReplacementRules(rules, "", "all", "find-asc").map(entry => entry.id))
    .toEqual(["one", "two", "ten"]);
  expect(getVisibleTextReplacementRules(rules, "", "all", "replacement-desc").map(entry => entry.id))
    .toEqual(["ten", "two", "one"]);
  expect(rules).toEqual(snapshot);
});

test("disabled rules remain searchable so they can be found and re-enabled", () => {
  const disabled = rule("disabled", "AivoRelay", "Aivo Relay", false);
  const rules = [rule("other", "Gemini", "Google Gemini"), disabled];

  expect(getVisibleTextReplacementRules(rules, "AIVORELAY", "all", "added"))
    .toEqual([disabled]);
  expect(getVisibleTextReplacementRules(rules, "AIVO RELAY", "replacement", "added"))
    .toEqual([disabled]);
  expect(disabled.enabled).toBe(false);
});

test("search treats regex metacharacters as literal text", () => {
  const literal = rule("literal", "a.b[0]", "$1");
  const rules = [rule("other", "axb0", "replacement"), literal];

  expect(getVisibleTextReplacementRules(rules, "a.b[0]", "all", "added"))
    .toEqual([literal]);
  expect(getVisibleTextReplacementRules(rules, "$1", "replacement", "added"))
    .toEqual([literal]);
});

test("removal rules with empty replacements sort first and retain their relative order", () => {
  const rules = [
    rule("normal", "hello", "world"),
    rule("remove-first", "um", ""),
    rule("remove-second", "uh", ""),
  ];

  expect(getVisibleTextReplacementRules(rules, "", "replacement", "replacement-asc").map(entry => entry.id))
    .toEqual(["remove-first", "remove-second", "normal"]);
  expect(getVisibleTextReplacementRules(rules, "", "replacement", "replacement-desc").map(entry => entry.id))
    .toEqual(["normal", "remove-first", "remove-second"]);
});
