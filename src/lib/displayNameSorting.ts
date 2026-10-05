export const SORT_DIRECTIONS = ["off", "asc", "desc"] as const;

export type SortDirection = (typeof SORT_DIRECTIONS)[number];

const displayNameCollator = new Intl.Collator(undefined, {
  numeric: true,
  sensitivity: "base",
});

export const compareNaturalStrings = displayNameCollator.compare;

/** Locale-aware numeric name sorting shared by all platforms. */
export const getDisplayNameSortOrder = (
  names: readonly string[],
  direction: SortDirection,
): number[] => {
  const originalOrder = names.map((_, index) => index);
  if (direction === "off" || names.length < 2) return originalOrder;

  const snapshot = [...names];
  const multiplier = direction === "desc" ? -1 : 1;
  return originalOrder.sort((left, right) => {
    const comparison = compareNaturalStrings(
      snapshot[left],
      snapshot[right],
    );
    return comparison * multiplier || left - right;
  });
};

export interface DisplayNameSortEntry {
  name: string;
  pinned: boolean;
  group: string | null;
}

/** Pinned entries stay first in caller order; names sort inside each group. */
export const getGroupedDisplayNameSortOrder = (
  entries: readonly DisplayNameSortEntry[],
  direction: SortDirection,
): number[] => {
  if (direction === "off") return entries.map((_, index) => index);

  const pinnedIndices: number[] = [];
  const sortableIndices: number[] = [];
  entries.forEach((entry, index) => {
    (entry.pinned ? pinnedIndices : sortableIndices).push(index);
  });
  const sortedIndices = getDisplayNameSortOrder(
    sortableIndices.map((index) => entries[index].name),
    direction,
  ).map((index) => sortableIndices[index]);

  // Keep the caller's group order while sorting names within each group.
  const groups = new Map<string | null, number[]>();
  for (const index of sortableIndices) {
    const group = entries[index].group;
    if (!groups.has(group)) groups.set(group, []);
  }
  for (const index of sortedIndices) groups.get(entries[index].group)!.push(index);
  return [...pinnedIndices, ...[...groups.values()].flat()];
};
