import React, { useState } from "react";
import { SettingContainer } from "./SettingContainer";
import "./ToggleSwitch.css";

interface ToggleSwitchProps {
  id?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  isUpdating?: boolean;
  label?: string;
  description?: string;
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  tooltipPosition?: "top" | "bottom";
  ariaLabel?: string;
}

export const ToggleSwitch: React.FC<ToggleSwitchProps> = ({
  id,
  checked,
  onChange,
  disabled = false,
  isUpdating = false,
  label,
  description,
  descriptionMode = "inline",
  grouped = false,
  tooltipPosition = "top",
  ariaLabel,
}) => {
  // Visual interactivity ignores the short async isUpdating phase to avoid
  // cursor/hover flicker after each click; the input itself stays disabled.
  const isInteractive = !disabled;
  // Press state drives the thumb "stretch" effect
  const [isPressed, setIsPressed] = useState(false);
  // Incrementing key remounts the pulse element to replay its animation
  const [pulseKey, setPulseKey] = useState(0);

  const press = () => {
    if (isInteractive && !isUpdating) setIsPressed(true);
  };
  const release = () => setIsPressed(false);

  const toggleElement = (
    <label
      id={!label && !description ? id : undefined}
      tabIndex={!label && !description && id ? -1 : undefined}
      className={`aivo-toggle inline-flex items-center relative ${isInteractive ? "cursor-pointer" : "cursor-not-allowed"}`}
      data-checked={checked}
      data-pressed={isPressed && isInteractive}
      data-interactive={isInteractive}
      onPointerDown={press}
      onPointerUp={release}
      onPointerLeave={release}
      onPointerCancel={release}
    >
      <input
        type="checkbox"
        value=""
        className="aivo-toggle__input sr-only"
        checked={checked}
        disabled={disabled || isUpdating}
        onChange={(e) => {
          if (e.target.checked) setPulseKey((k) => k + 1);
          onChange(e.target.checked);
        }}
        onKeyDown={(e) => {
          if (e.key === " ") press();
        }}
        onKeyUp={release}
        onBlur={release}
        aria-label={ariaLabel || label}
      />
      <span className="aivo-toggle__track" aria-hidden="true">
        {pulseKey > 0 && (
          <span key={pulseKey} className="aivo-toggle__pulse" />
        )}
        <span className="aivo-toggle__thumb" />
      </span>
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
      id={id}
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
