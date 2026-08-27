export interface TextReplacementRule {
  id: string;
  from: string;
  to: string;
  enabled: boolean;
  case_sensitive: boolean;
  is_regex: boolean;
}

export type TextReplacementSearchScope = "all" | "replacement";

export type TextReplacementColumnSortDirection = "off" | "asc" | "desc";

export type TextReplacementSortOrder =
  | "added"
  | "find-asc"
  | "find-desc"
  | "replacement-asc"
  | "replacement-desc";

const alphabeticalCollator = new Intl.Collator(undefined, {
  numeric: true,
  sensitivity: "base",
});

interface IndexedRule {
  rule: TextReplacementRule;
  index: number;
}

const compareIndexedRules = (
  left: IndexedRule,
  right: IndexedRule,
  field: "from" | "to",
  direction: 1 | -1,
): number => {
  const comparison = alphabeticalCollator.compare(
    left.rule[field],
    right.rule[field],
  );
  if (comparison !== 0) {
    return comparison * direction;
  }

  return left.index - right.index;
};

export const getVisibleTextReplacementRules = (
  rules: readonly TextReplacementRule[],
  query: string,
  scope: TextReplacementSearchScope,
  sortOrder: TextReplacementSortOrder,
): TextReplacementRule[] => {
  const normalizedQuery = query.toLowerCase();
  const indexedVisibleRules: IndexedRule[] = rules
    .map((rule, index) => ({ rule, index }))
    .filter(({ rule }) => {
      if (normalizedQuery.length === 0) {
        return true;
      }

      const replacementMatches = rule.to
        .toLowerCase()
        .includes(normalizedQuery);
      if (scope === "replacement") {
        return replacementMatches;
      }

      return (
        replacementMatches ||
        rule.from.toLowerCase().includes(normalizedQuery)
      );
    });

  switch (sortOrder) {
    case "find-asc":
      indexedVisibleRules.sort((left, right) =>
        compareIndexedRules(left, right, "from", 1),
      );
      break;
    case "find-desc":
      indexedVisibleRules.sort((left, right) =>
        compareIndexedRules(left, right, "from", -1),
      );
      break;
    case "replacement-asc":
      indexedVisibleRules.sort((left, right) =>
        compareIndexedRules(left, right, "to", 1),
      );
      break;
    case "replacement-desc":
      indexedVisibleRules.sort((left, right) =>
        compareIndexedRules(left, right, "to", -1),
      );
      break;
    case "added":
      break;
  }

  return indexedVisibleRules.map(({ rule }) => rule);
};
