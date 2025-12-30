import React from "react";
import { SettingContainer } from "./SettingContainer";

interface ToggleSwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  isUpdating?: boolean;
  label?: string;
  description?: string;
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  tooltipPosition?: "top" | "bottom";
}

export const ToggleSwitch: React.FC<ToggleSwitchProps> = ({
  checked,
  onChange,
  disabled = false,
  isUpdating = false,
  label,
  description,
  descriptionMode = "tooltip",
  grouped = false,
  tooltipPosition = "top",
}) => {
  const toggleElement = (
    <label
      className={`inline-flex items-center relative ${disabled || isUpdating ? "cursor-not-allowed" : "cursor-pointer"}`}
    >
      <input
        type="checkbox"
        value=""
        className="sr-only peer"
        checked={checked}
        disabled={disabled || isUpdating}
        onChange={(e) => onChange(e.target.checked)}
      />
      <div className={`relative w-11 h-6 ${checked ? 'bg-gradient-to-r from-[#ff4d8d] to-[#9b5de5]' : 'bg-[#333333]'} peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-[#ff4d8d]/40 rounded-full peer transition-all duration-200 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all after:shadow-[0_2px_4px_rgba(0,0,0,0.4)] peer-disabled:opacity-40`}></div>
      {isUpdating && (
        <div className="absolute inset-0 flex items-center justify-center">
          <div className="w-4 h-4 border-2 border-logo-primary border-t-transparent rounded-full animate-spin"></div>
        </div>
      )}
    </label>
  );

  // If no label/description provided, render just the toggle (bare mode)
  if (!label && !description) {
    return toggleElement;
  }

  return (
    <SettingContainer
      title={label || ""}
      description={description || ""}
      descriptionMode={descriptionMode}
      grouped={grouped}
      disabled={disabled}
      tooltipPosition={tooltipPosition}
    >
      {toggleElement}
    </SettingContainer>
  );
};
