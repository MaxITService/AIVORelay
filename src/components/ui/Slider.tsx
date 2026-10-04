import React from "react";
import { SettingContainer } from "./SettingContainer";
import "./Slider.css";

interface SliderProps {
  id?: string;
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
  id,
  value,
  onChange,
  onChangeComplete,
  min,
  max,
  step = 0.01,
  disabled = false,
  label,
  description,
  descriptionMode = "inline",
  grouped = false,
  showValue = true,
  formatValue = (v) => v.toFixed(2),
}) => {
  const [internalValue, setInternalValue] = React.useState(value);
  const latestValueRef = React.useRef(value);
  const [isInteracting, setIsInteracting] = React.useState(false);

  React.useEffect(() => {
    setInternalValue(value);
    latestValueRef.current = value;
  }, [value]);

  // Pointer drag state drives the thumb squash, fill glow and value emphasis
  const [isDragging, setIsDragging] = React.useState(false);
  const isDraggingRef = React.useRef(false);
  // Incrementing key remounts the value label to replay its pop animation
  const [popKey, setPopKey] = React.useState(0);

  React.useEffect(() => {
    if (!isDragging) {
      return;
    }

    const handleDragEnd = () => {
      isDraggingRef.current = false;
      setIsDragging(false);
    };

    window.addEventListener("pointerup", handleDragEnd);
    window.addEventListener("pointercancel", handleDragEnd);

    return () => {
      window.removeEventListener("pointerup", handleDragEnd);
      window.removeEventListener("pointercancel", handleDragEnd);
    };
  }, [isDragging]);

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const nextValue = parseFloat(e.target.value);
    setInternalValue(nextValue);
    latestValueRef.current = nextValue;
    // Pop only on discrete changes; while dragging the label stays enlarged
    if (!isDraggingRef.current) {
      setPopKey((k) => k + 1);
    }
    onChange(nextValue);
  };

  const commitValue = React.useCallback(() => {
    if (onChangeComplete) {
      onChangeComplete(latestValueRef.current);
    }
  }, [onChangeComplete]);

  React.useEffect(() => {
    if (!isInteracting) {
      return;
    }

    const handleInteractionEnd = () => {
      setIsInteracting(false);
      commitValue();
    };

    window.addEventListener("mouseup", handleInteractionEnd);
    window.addEventListener("touchend", handleInteractionEnd);
    window.addEventListener("touchcancel", handleInteractionEnd);

    return () => {
      window.removeEventListener("mouseup", handleInteractionEnd);
      window.removeEventListener("touchend", handleInteractionEnd);
      window.removeEventListener("touchcancel", handleInteractionEnd);
    };
  }, [commitValue, isInteracting]);

  const handleInteractionStart = () => {
    if (onChangeComplete) {
      setIsInteracting(true);
    }
  };

  const fillPercent =
    max > min
      ? Math.min(100, Math.max(0, ((internalValue - min) / (max - min)) * 100))
      : 0;

  return (
    <SettingContainer
      id={id}
      title={label}
      description={description}
      descriptionMode={descriptionMode}
      grouped={grouped}
      layout="horizontal"
      disabled={disabled}
    >
      <div
        className="aivo-slider w-full"
        data-active={isDragging && !disabled}
      >
        <div className="flex items-center space-x-2 h-6">
          <div className="relative flex-grow flex items-center h-6">
            <span
              className="aivo-slider__glow"
              style={{ width: `${fillPercent}%` }}
              aria-hidden="true"
            />
            <input
              type="range"
              min={min}
              max={max}
              step={step}
              value={internalValue}
              onChange={handleChange}
              onMouseDown={handleInteractionStart}
              onTouchStart={handleInteractionStart}
              onPointerDown={(event) => {
                if (disabled || event.button !== 0) return;
                isDraggingRef.current = true;
                setIsDragging(true);
              }}
              onBlur={() => {
                if (!isInteracting) {
                  commitValue();
                }
              }}
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
              aria-label={label}
              aria-valuetext={formatValue(internalValue)}
              className="aivo-slider__input relative z-[1] w-full h-2 rounded-full appearance-none cursor-pointer focus:outline-none focus:ring-2 focus:ring-[#ff4d8d]/40 disabled:opacity-40 disabled:cursor-not-allowed"
              style={{
                background: `linear-gradient(to right, #ff4d8d ${fillPercent}%, #333333 ${fillPercent}%)`,
              }}
            />
          </div>
          {showValue && (
            <span className="text-sm font-semibold text-[#ff4d8d] min-w-12 text-right tabular-nums">
              <span
                key={popKey}
                className={`aivo-slider__value${popKey > 0 ? " aivo-slider__value--pop" : ""}`}
              >
                {formatValue(internalValue)}
              </span>
            </span>
          )}
        </div>
      </div>
    </SettingContainer>
  );
};
