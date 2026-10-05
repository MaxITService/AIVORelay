import { useCallback } from "react";
import { create } from "zustand";
import { persist } from "zustand/middleware";
import { SORT_DIRECTIONS, type SortDirection } from "@/lib/displayNameSorting";

interface ListSortPreferences {
  orders: Record<string, string>;
  setOrder: (listKey: string, order: string) => void;
}

const useListSortPreferences = create<ListSortPreferences>()(
  persist(
    (set) => ({
      orders: {},
      setOrder: (listKey, order) => set((state) => ({
        orders: { ...state.orders, [listKey]: order },
      })),
    }),
    {
      name: "aivorelay-list-sorting-v1",
      partialize: (state) => ({ orders: state.orders }),
    },
  ),
);

export const useListSortPreference = <T extends string>(
  listKey: string,
  defaultOrder: T,
  choices: readonly T[],
) => {
  const storedOrder = useListSortPreferences((state) => state.orders[listKey]);
  const setOrder = useListSortPreferences((state) => state.setOrder);
  const order = choices.includes(storedOrder as T) ? storedOrder as T : defaultOrder;
  const onChange = useCallback(
    (nextOrder: T) => setOrder(listKey, nextOrder),
    [listKey, setOrder],
  );
  return [order, onChange] as const;
};

export const useListSortDirection = (listKey: string) =>
  useListSortPreference<SortDirection>(listKey, "asc", SORT_DIRECTIONS);
