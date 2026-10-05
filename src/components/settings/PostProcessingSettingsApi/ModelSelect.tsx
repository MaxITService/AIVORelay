import React from "react";
import type { ModelOption } from "./types";
import { NameSortedSelect } from "../../ui/NameSortedSelect";
import type { SortDirection } from "../../ui/SortControl";

type ModelSelectProps = {
  sortKey: string;
  fixedDirection?: SortDirection;
  value: string;
  options: ModelOption[];
  disabled?: boolean;
  placeholder?: string;
  isLoading?: boolean;
  onSelect: (value: string) => void;
  onCreate: (value: string) => void;
  onBlur: () => void;
  className?: string;
};

export const ModelSelect: React.FC<ModelSelectProps> = React.memo(
  ({
    sortKey,
    fixedDirection,
    value,
    options,
    disabled,
    placeholder,
    isLoading,
    onSelect,
    onCreate,
    onBlur,
    className = "flex-1 min-w-[360px]",
  }) => {
    const handleCreate = (inputValue: string) => {
      const trimmed = inputValue.trim();
      if (!trimmed) return;
      onCreate(trimmed);
    };

    const computedClassName = `text-sm ${className}`;

    return (
      <NameSortedSelect
        sortKey={sortKey}
        fixedDirection={fixedDirection}
        className={computedClassName}
        value={value || null}
        options={options}
        onChange={(selected) => onSelect(selected ?? "")}
        onCreateOption={handleCreate}
        onBlur={onBlur}
        placeholder={placeholder}
        disabled={disabled}
        isLoading={isLoading}
        isCreatable
        formatCreateLabel={(input) => `Use "${input}"`}
      />
    );
  },
);

ModelSelect.displayName = "ModelSelect";
