import { useMemo } from "react";
import {
  getGroupedDisplayNameSortOrder,
  type DisplayNameSortEntry,
  type SortDirection,
} from "@/lib/displayNameSorting";

interface DisplayNameSortOptions<T> {
  isPinned?: (item: T) => boolean;
  getGroup?: (item: T) => string;
}

export const useSortedDisplayNames = <T>(
  items: readonly T[],
  getName: (item: T) => string,
  direction: SortDirection,
  { isPinned, getGroup }: DisplayNameSortOptions<T> = {},
) => {
  // Inline callbacks can change identity without changing the sort criteria.
  const criteriaKey = JSON.stringify(items.map((item) => ({
    name: getName(item),
    pinned: isPinned?.(item) ?? false,
    group: getGroup?.(item) ?? null,
  })));
  const order = useMemo(
    () => getGroupedDisplayNameSortOrder(
      JSON.parse(criteriaKey) as DisplayNameSortEntry[],
      direction,
    ),
    [criteriaKey, direction],
  );
  const sortedItems = useMemo(
    () => order.map((index) => items[index]),
    [items, order],
  );
  return { items: sortedItems };
};
