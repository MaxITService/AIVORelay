export type HistoryChange<T> =
  | { action: "added" | "updated"; entry: T }
  | { action: "deleted"; id: number }
  | { action: "cleared" }
  | { action: "toggled"; id: number; saved: boolean };

export function reconcileHistoryPage<T extends { id: number; saved: boolean }>(
  previous: T[],
  page: T[],
  firstPage: boolean,
  changes: HistoryChange<T>[],
  matches?: (entry: T) => boolean,
): T[] {
  const entries = new Map<number, T>();
  const deletedIds = new Set<number>();
  let cleared = false;
  for (const entry of firstPage ? page : [...page, ...previous]) {
    entries.set(entry.id, entry);
  }
  for (const change of changes) {
    switch (change.action) {
      case "added":
        deletedIds.delete(change.entry.id);
        entries.set(change.entry.id, change.entry);
        break;
      case "updated":
        if (
          entries.has(change.entry.id) ||
          (matches && !cleared && !deletedIds.has(change.entry.id) && matches(change.entry))
        ) {
          entries.set(change.entry.id, change.entry);
        }
        break;
      case "deleted":
        deletedIds.add(change.id);
        entries.delete(change.id);
        break;
      case "cleared":
        cleared = true;
        entries.clear();
        break;
      case "toggled": {
        const entry = entries.get(change.id);
        if (entry) entries.set(change.id, { ...entry, saved: change.saved });
        break;
      }
    }
  }
  return [...entries.values()]
    .filter((entry) => !matches || matches(entry))
    .sort((a, b) => b.id - a.id);
}
