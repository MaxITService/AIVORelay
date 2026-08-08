import React from "react";
import { formatKeyCombination, type OSType } from "@/lib/utils/keyboard";
import { useKeyboardCapture } from "@/hooks/useKeyboardCapture";
import { ResetButton } from "./ResetButton";

interface HotkeyCaptureProps {
  value: string;
  isCapturing: boolean;
  onStartCapture: () => void;
  onCaptured: (hotkey: string) => void | Promise<void>;
  onCancel: () => void | Promise<void>;
  onClear?: () => void;
  disabled?: boolean;
  osType: OSType;
  placeholder?: string;
}

export const HotkeyCapture: React.FC<HotkeyCaptureProps> = ({
  value,
  isCapturing,
  onStartCapture,
  onCaptured,
  onCancel,
  onClear,
  disabled = false,
  osType,
  placeholder = "Not set",
}) => {
  const { containerRef, displayKeys } = useKeyboardCapture({
    isCapturing,
    osType,
    onCaptured,
    onCancel,
  });

  const formattedValue = value
    ? formatKeyCombination(value, osType)
    : "";

  return (
    <div ref={containerRef} className="flex items-center space-x-1">
      {isCapturing ? (
        <div
          role="status"
          aria-live="polite"
          className="px-2 py-1 text-sm font-semibold border border-logo-primary bg-logo-primary/30 rounded min-w-[120px] text-center select-none"
        >
          {displayKeys || "Press keys..."}
        </div>
      ) : (
        <button
          type="button"
          className={`px-2 py-1 text-sm font-semibold bg-mid-gray/10 border border-mid-gray/80 rounded min-w-[120px] text-center select-none ${
            disabled
              ? "opacity-40 cursor-not-allowed"
              : "hover:bg-logo-primary/10 cursor-pointer hover:border-logo-primary"
          }`}
          onClick={onStartCapture}
          disabled={disabled}
        >
          {formattedValue || placeholder}
        </button>
      )}
      {onClear && (
        <ResetButton
          onClick={onClear}
          disabled={disabled || !value}
        />
      )}
    </div>
  );
};
