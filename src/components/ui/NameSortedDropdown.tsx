import React from "react";
import { useSortedDisplayNames } from "@/hooks/useSortedDisplayNames";
import { useListSortDirection } from "@/hooks/useListSortPreference";
import { Dropdown, type DropdownProps } from "./Dropdown";
import { NameSortControl } from "./NameSortControl";
import type { SortDirection } from "./SortControl";

interface NameSortedDropdownProps extends DropdownProps {
  sortKey: string;
  pinnedValues?: readonly string[];
  /** Always sort this way and hide the sort control. */
  fixedDirection?: SortDirection;
}

export const NameSortedDropdown: React.FC<NameSortedDropdownProps> = ({
  sortKey,
  options,
  className = "",
  disabled = false,
  pinnedValues = [],
  fixedDirection,
  ...dropdownProps
}) => {
  const [storedDirection, setDirection] = useListSortDirection(sortKey);
  const direction = fixedDirection ?? storedDirection;
  const { items } = useSortedDisplayNames(
    options,
    (option) => option.sortLabel ?? option.label,
    direction,
    { isPinned: (option) => pinnedValues.includes(option.value) },
  );

  return (
    <div className={`min-w-0 ${className}`}>
      <div className="flex min-w-0 items-center gap-2">
        <Dropdown
          {...dropdownProps}
          options={items}
          disabled={disabled}
          className="min-w-0 flex-1 [&>button]:min-w-0"
        />
        {!fixedDirection && (
          <NameSortControl direction={direction} onChange={setDirection} />
        )}
      </div>
    </div>
  );
};
