import React from "react";
import SelectComponent from "react-select";
import CreatableSelect from "react-select/creatable";
import type {
  ActionMeta,
  Props as ReactSelectProps,
  SingleValue,
  StylesConfig,
} from "react-select";

export type SelectOption = {
  value: string;
  label: string;
  isDisabled?: boolean;
};

type BaseProps = {
  value: string | null;
  options: SelectOption[];
  placeholder?: string;
  disabled?: boolean;
  isLoading?: boolean;
  isClearable?: boolean;
  onChange: (value: string | null, action: ActionMeta<SelectOption>) => void;
  onBlur?: () => void;
  className?: string;
  formatCreateLabel?: (input: string) => string;
};

type CreatableProps = {
  isCreatable: true;
  onCreateOption: (value: string) => void;
};

type NonCreatableProps = {
  isCreatable?: false;
  onCreateOption?: never;
};

export type SelectProps = BaseProps & (CreatableProps | NonCreatableProps);

// Adobe-style dark theme colors
const darkBg = "rgba(30, 30, 30, 0.9)";
const darkBgHover = "rgba(37, 37, 37, 0.95)";
const darkBgFocus = "rgba(43, 43, 43, 0.98)";
const borderColor = "#3c3c3c";
const borderHover = "#4a4a4a";
const accentPrimary = "#ff4d8d";
const textPrimary = "#e8e8e8";
const textMuted = "#6b6b6b";

const selectStyles: StylesConfig<SelectOption, false> = {
  control: (base, state) => ({
    ...base,
    minHeight: 40,
    borderRadius: 6,
    borderColor: state.isFocused ? accentPrimary : borderColor,
    boxShadow: state.isFocused 
      ? `0 0 0 2px rgba(255, 77, 141, 0.2)` 
      : "0 2px 8px rgba(0, 0, 0, 0.2)",
    backgroundColor: state.isFocused ? darkBgFocus : darkBg,
    fontSize: "0.875rem",
    color: textPrimary,
    transition: "all 200ms ease",
    ":hover": {
      borderColor: borderHover,
      backgroundColor: darkBgHover,
    },
  }),
  valueContainer: (base) => ({
    ...base,
    paddingInline: 12,
    paddingBlock: 6,
  }),
  input: (base) => ({
    ...base,
    color: textPrimary,
  }),
  singleValue: (base) => ({
    ...base,
    color: textPrimary,
    fontWeight: 500,
  }),
  dropdownIndicator: (base, state) => ({
    ...base,
    color: state.isFocused ? accentPrimary : textMuted,
    transition: "color 200ms ease",
    ":hover": {
      color: accentPrimary,
    },
  }),
  clearIndicator: (base) => ({
    ...base,
    color: textMuted,
    ":hover": {
      color: "#ff6b9d",
    },
  }),
  menu: (provided) => ({
    ...provided,
    zIndex: 9999,
    backgroundColor: "rgba(37, 37, 37, 0.98)",
    backdropFilter: "blur(12px) saturate(150%)",
    color: textPrimary,
    border: `1px solid ${borderColor}`,
    borderRadius: 8,
    boxShadow: "0 12px 40px rgba(0, 0, 0, 0.5)",
    overflow: "hidden",
  }),
  menuPortal: (base) => ({ ...base, zIndex: 9999 }),
  menuList: (base) => ({
    ...base,
    padding: 4,
  }),
  option: (base, state) => ({
    ...base,
    borderRadius: 4,
    backgroundColor: state.isSelected
      ? "rgba(255, 77, 141, 0.2)"
      : state.isFocused
        ? "rgba(255, 255, 255, 0.05)"
        : "transparent",
    color: state.isSelected ? accentPrimary : textPrimary,
    cursor: state.isDisabled ? "not-allowed" : "pointer",
    opacity: state.isDisabled ? 0.4 : 1,
    transition: "all 150ms ease",
  }),
  placeholder: (base) => ({
    ...base,
    color: textMuted,
  }),
};

export const Select: React.FC<SelectProps> = React.memo(
  ({
    value,
    options,
    placeholder,
    disabled,
    isLoading,
    isClearable = true,
    onChange,
    onBlur,
    className = "",
    isCreatable,
    formatCreateLabel,
    onCreateOption,
  }) => {
    const selectValue = React.useMemo(() => {
      if (!value) return null;
      const existing = options.find((option) => option.value === value);
      if (existing) return existing;
      return { value, label: value, isDisabled: false };
    }, [value, options]);

    const handleChange = (
      option: SingleValue<SelectOption>,
      action: ActionMeta<SelectOption>,
    ) => {
      onChange(option?.value ?? null, action);
    };

    const sharedProps: Partial<ReactSelectProps<SelectOption, false>> = {
      className,
      classNamePrefix: "app-select",
      value: selectValue,
      options,
      onChange: handleChange,
      placeholder,
      isDisabled: disabled,
      isLoading,
      onBlur,
      isClearable,
      styles: selectStyles,
      menuPortalTarget: typeof document !== 'undefined' ? document.body : null,
    };

    if (isCreatable) {
      return (
        <CreatableSelect<SelectOption, false>
          {...sharedProps}
          onCreateOption={onCreateOption}
          formatCreateLabel={formatCreateLabel}
        />
      );
    }

    return <SelectComponent<SelectOption, false> {...sharedProps} />;
  },
);

Select.displayName = "Select";
