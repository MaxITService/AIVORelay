export interface TextReplacementRule {
  id: string;
  from: string;
  to: string;
  enabled: boolean;
  case_sensitive: boolean;
  is_regex: boolean;
}

export type TextReplacementSearchScope = "all" | "replacement";

export type TextReplacementSortOrder =
  | "alphabetical-asc"
  | "alphabetical-desc"
  | "newest"
  | "oldest";

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
  direction: 1 | -1,
): number => {
  const fromComparison = alphabeticalCollator.compare(
    left.rule.from,
    right.rule.from,
  );
  if (fromComparison !== 0) {
    return fromComparison * direction;
  }

  const toComparison = alphabeticalCollator.compare(
    left.rule.to,
    right.rule.to,
  );
  if (toComparison !== 0) {
    return toComparison * direction;
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
    case "alphabetical-asc":
      indexedVisibleRules.sort((left, right) =>
        compareIndexedRules(left, right, 1),
      );
      break;
    case "alphabetical-desc":
      indexedVisibleRules.sort((left, right) =>
        compareIndexedRules(left, right, -1),
      );
      break;
    case "newest":
      indexedVisibleRules.reverse();
      break;
    case "oldest":
      break;
  }

  return indexedVisibleRules.map(({ rule }) => rule);
};
