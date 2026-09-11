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
): T[] {
  const entries = new Map<number, T>();
  for (const entry of firstPage ? page : [...page, ...previous]) {
    entries.set(entry.id, entry);
  }
  for (const change of changes) {
    switch (change.action) {
      case "added":
        entries.set(change.entry.id, change.entry);
        break;
      case "updated":
        if (entries.has(change.entry.id)) entries.set(change.entry.id, change.entry);
        break;
      case "deleted":
        entries.delete(change.id);
        break;
      case "cleared":
        entries.clear();
        break;
      case "toggled": {
        const entry = entries.get(change.id);
        if (entry) entries.set(change.id, { ...entry, saved: change.saved });
        break;
      }
    }
  }
  return [...entries.values()].sort((a, b) => b.id - a.id);
}
