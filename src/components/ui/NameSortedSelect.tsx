import React from "react";
import { useSortedDisplayNames } from "@/hooks/useSortedDisplayNames";
import { useListSortDirection } from "@/hooks/useListSortPreference";
import { Select, type SelectProps } from "./Select";
import { NameSortControl } from "./NameSortControl";
import type { SortDirection } from "./SortControl";

type NameSortedSelectProps = SelectProps & {
  sortKey: string;
  pinnedValues?: readonly string[];
  /** Always sort this way and hide the sort control. */
  fixedDirection?: SortDirection;
};

export const NameSortedSelect: React.FC<NameSortedSelectProps> = ({
  sortKey,
  options,
  className = "",
  disabled = false,
  isLoading = false,
  pinnedValues = [],
  fixedDirection,
  ...selectProps
}) => {
  const [storedDirection, setDirection] = useListSortDirection(sortKey);
  const direction = fixedDirection ?? storedDirection;
  const { items } = useSortedDisplayNames(
    options,
    (option) => option.label,
    direction,
    {
      isPinned: (option) => pinnedValues.includes(option.value),
      getGroup: (option) => option.group?.trim() || "Other",
    },
  );

  return (
    <div className={`min-w-0 ${className}`}>
      <div className="flex min-w-0 items-center gap-2">
        <Select
          {...selectProps}
          options={items}
          disabled={disabled}
          isLoading={isLoading}
          className="min-w-0 flex-1"
        />
        {!fixedDirection && (
          <NameSortControl direction={direction} onChange={setDirection} />
        )}
      </div>
    </div>
  );
};
