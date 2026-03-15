import React from "react";
import { SettingContainer } from "./SettingContainer";

interface SliderProps {
  value: number;
  onChange: (value: number) => void;
  onChangeComplete?: (value: number) => void;
  min: number;
  max: number;
  step?: number;
  disabled?: boolean;
  label: string;
  description: string;
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  showValue?: boolean;
  formatValue?: (value: number) => string;
}

export const Slider: React.FC<SliderProps> = ({
  value,
  onChange,
  onChangeComplete,
  min,
  max,
  step = 0.01,
  disabled = false,
  label,
  description,
  descriptionMode = "tooltip",
  grouped = false,
  showValue = true,
  formatValue = (v) => v.toFixed(2),
}) => {
  const [internalValue, setInternalValue] = React.useState(value);

  React.useEffect(() => {
    setInternalValue(value);
  }, [value]);

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const nextValue = parseFloat(e.target.value);
    setInternalValue(nextValue);
    onChange(nextValue);
  };

  const commitValue = () => {
    if (onChangeComplete) {
      onChangeComplete(internalValue);
    }
  };

  return (
    <SettingContainer
      title={label}
      description={description}
      descriptionMode={descriptionMode}
      grouped={grouped}
      layout="horizontal"
      disabled={disabled}
    >
      <div className="w-full">
        <div className="flex items-center space-x-2 h-6">
          <input
            type="range"
            min={min}
            max={max}
            step={step}
            value={internalValue}
            onChange={handleChange}
            onMouseUp={commitValue}
            onTouchEnd={commitValue}
            onKeyUp={(event) => {
              if (
                event.key.startsWith("Arrow") ||
                event.key === "Home" ||
                event.key === "End" ||
                event.key === "PageUp" ||
                event.key === "PageDown"
              ) {
                commitValue();
              }
            }}
            disabled={disabled}
            className="flex-grow h-2 rounded-full appearance-none cursor-pointer focus:outline-none focus:ring-2 focus:ring-[#ff4d8d]/40 disabled:opacity-40 disabled:cursor-not-allowed"
            style={{
              background: `linear-gradient(to right, #ff4d8d ${
                ((internalValue - min) / (max - min)) * 100
              }%, #333333 ${
                ((internalValue - min) / (max - min)) * 100
              }%)`,
            }}
          />
          {showValue && (
            <span className="text-sm font-semibold text-[#ff4d8d] min-w-12 text-right tabular-nums">
              {formatValue(internalValue)}
            </span>
          )}
        </div>
      </div>
    </SettingContainer>
  );
};
