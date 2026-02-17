import React, { useCallback, useEffect, useRef, useState } from "react";
import {
  getKeyName,
  normalizeKey,
  formatKeyCombination,
  type OSType,
} from "@/lib/utils/keyboard";
import { normalizePreviewHotkeyString } from "@/lib/utils/previewHotkeys";
import { ResetButton } from "./ResetButton";

interface HotkeyCaptureProps {
  value: string;
  isCapturing: boolean;
  onStartCapture: () => void;
  onCaptured: (hotkey: string) => void;
  onCancel: () => void;
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
  const containerRef = useRef<HTMLDivElement>(null);
  const keyPressedRef = useRef<string[]>([]);
  const recordedKeysRef = useRef<string[]>([]);
  const [displayKeys, setDisplayKeys] = useState("");

  const resetCapture = useCallback(() => {
    keyPressedRef.current = [];
    recordedKeysRef.current = [];
    setDisplayKeys("");
  }, []);

  useEffect(() => {
    if (!isCapturing) {
      resetCapture();
      return;
    }

    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.repeat) return;

      if (e.key === "Escape") {
        onCancel();
        return;
      }

      const rawKey = getKeyName(e, osType);
      const key = normalizeKey(rawKey);

      if (!keyPressedRef.current.includes(key)) {
        keyPressedRef.current = [...keyPressedRef.current, key];
      }
      if (!recordedKeysRef.current.includes(key)) {
        recordedKeysRef.current = [...recordedKeysRef.current, key];
      }
      setDisplayKeys(
        formatKeyCombination(recordedKeysRef.current.join("+"), osType),
      );
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      e.preventDefault();

      const rawKey = getKeyName(e, osType);
      const key = normalizeKey(rawKey);

      keyPressedRef.current = keyPressedRef.current.filter((k) => k !== key);

      if (
        keyPressedRef.current.length === 0 &&
        recordedKeysRef.current.length > 0
      ) {
        const normalized = normalizePreviewHotkeyString(
          recordedKeysRef.current.join("+"),
        );
        if (normalized) {
          onCaptured(normalized);
        } else {
          onCancel();
        }
      }
    };

    const handleClickOutside = (e: MouseEvent) => {
      if (
        containerRef.current &&
        !containerRef.current.contains(e.target as Node)
      ) {
        onCancel();
      }
    };

    const handleBlur = () => {
      onCancel();
    };

    window.addEventListener("keydown", handleKeyDown, true);
    window.addEventListener("keyup", handleKeyUp, true);
    document.addEventListener("mousedown", handleClickOutside);
    window.addEventListener("blur", handleBlur);

    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
      window.removeEventListener("keyup", handleKeyUp, true);
      document.removeEventListener("mousedown", handleClickOutside);
      window.removeEventListener("blur", handleBlur);
    };
  }, [isCapturing, osType, onCaptured, onCancel, resetCapture]);

  const formattedValue = value
    ? formatKeyCombination(value, osType)
    : "";

  return (
    <div ref={containerRef} className="flex items-center space-x-1">
      {isCapturing ? (
        <div className="px-2 py-1 text-sm font-semibold border border-logo-primary bg-logo-primary/30 rounded min-w-[120px] text-center select-none">
          {displayKeys || "Press keys..."}
        </div>
      ) : (
        <div
          className={`px-2 py-1 text-sm font-semibold bg-mid-gray/10 border border-mid-gray/80 rounded min-w-[120px] text-center select-none ${
            disabled
              ? "opacity-40 cursor-not-allowed"
              : "hover:bg-logo-primary/10 cursor-pointer hover:border-logo-primary"
          }`}
          onClick={disabled ? undefined : onStartCapture}
        >
          {formattedValue || placeholder}
        </div>
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
