import React from "react";
import { useTranslation } from "react-i18next";
import { SortControl, type SortDirection } from "./SortControl";

interface NameSortControlProps {
  direction: SortDirection;
  onChange: (direction: SortDirection) => void;
  ariaLabel?: string;
  /** Visible caption for controls placed away from the list they sort. */
  label?: string;
}

export const NameSortControl: React.FC<NameSortControlProps> = ({
  direction,
  onChange,
  ariaLabel,
  label,
}) => {
  const { t } = useTranslation();
  const control = (
    <SortControl
      variant="subtle"
      direction={direction}
      onChange={onChange}
      ariaLabel={ariaLabel ?? label ?? t("listSorting.nameLabel", "Sort by name")}
      labels={{
        off: t("listSorting.off", "Off"),
        asc: t("listSorting.ascending", "A → Z"),
        desc: t("listSorting.descending", "Z → A"),
      }}
      offTooltip={t(
        "listSorting.offTooltip",
        "Off shows items in their original order.",
      )}
      activeTooltip={t(
        "listSorting.activeTooltip",
        "Sorted by name. Shift-click to turn sorting off.",
      )}
    />
  );
  if (!label) return control;

  return (
    <span className="inline-flex shrink-0 items-center gap-1.5">
      <span className="text-xs text-[#8a8a8a]">{label}</span>
      {control}
    </span>
  );
};
