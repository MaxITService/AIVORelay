import { useCallback, useEffect, useRef, useState } from "react";
import {
  formatKeyCombination,
  getKeyName,
  normalizeHotkeyString,
  normalizeKey,
  type OSType,
} from "@/lib/utils/keyboard";

interface UseKeyboardCaptureOptions {
  isCapturing: boolean;
  osType: OSType;
  onCaptured: (hotkey: string) => void | Promise<void>;
  onCancel: () => void | Promise<void>;
}

/**
 * Shared browser-side keyboard capture for settings that collect a key or
 * combination from DOM KeyboardEvents. Persistence and backend registration
 * remain the caller's responsibility.
 */
export const useKeyboardCapture = ({
  isCapturing,
  osType,
  onCaptured,
  onCancel,
}: UseKeyboardCaptureOptions) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const pressedKeysRef = useRef<string[]>([]);
  const recordedKeysRef = useRef<string[]>([]);
  const completingRef = useRef(false);
  const onCapturedRef = useRef(onCaptured);
  const onCancelRef = useRef(onCancel);
  const [displayKeys, setDisplayKeys] = useState("");

  useEffect(() => {
    onCapturedRef.current = onCaptured;
  }, [onCaptured]);

  useEffect(() => {
    onCancelRef.current = onCancel;
  }, [onCancel]);

  const resetCapture = useCallback(() => {
    pressedKeysRef.current = [];
    recordedKeysRef.current = [];
    setDisplayKeys("");
  }, []);

  useEffect(() => {
    if (!isCapturing) {
      completingRef.current = false;
      resetCapture();
      return;
    }

    const completeCapture = (callback: () => void | Promise<void>) => {
      if (completingRef.current) return;
      completingRef.current = true;
      void Promise.resolve()
        .then(callback)
        .catch((error) => {
          console.error("Keyboard capture callback failed:", error);
        })
        .finally(() => {
          completingRef.current = false;
          resetCapture();
        });
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      if (event.repeat || completingRef.current) return;

      if (event.key === "Escape") {
        completeCapture(() => onCancelRef.current());
        return;
      }

      const key = normalizeKey(getKeyName(event, osType));
      if (!pressedKeysRef.current.includes(key)) {
        pressedKeysRef.current = [...pressedKeysRef.current, key];
      }
      if (!recordedKeysRef.current.includes(key)) {
        recordedKeysRef.current = [...recordedKeysRef.current, key];
      }
      setDisplayKeys(
        formatKeyCombination(recordedKeysRef.current.join("+"), osType),
      );
    };

    const handleKeyUp = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      if (completingRef.current) return;

      const key = normalizeKey(getKeyName(event, osType));
      pressedKeysRef.current = pressedKeysRef.current.filter(
        (pressedKey) => pressedKey !== key,
      );

      if (
        pressedKeysRef.current.length === 0 &&
        recordedKeysRef.current.length > 0
      ) {
        const normalized = normalizeHotkeyString(
          recordedKeysRef.current.join("+"),
        );
        if (normalized) {
          completeCapture(() => onCapturedRef.current(normalized));
        } else {
          completeCapture(() => onCancelRef.current());
        }
      }
    };

    const handlePointerOutside = (event: MouseEvent) => {
      if (
        containerRef.current &&
        !containerRef.current.contains(event.target as Node)
      ) {
        completeCapture(() => onCancelRef.current());
      }
    };

    const handleBlur = () => {
      completeCapture(() => onCancelRef.current());
    };

    window.addEventListener("keydown", handleKeyDown, true);
    window.addEventListener("keyup", handleKeyUp, true);
    document.addEventListener("mousedown", handlePointerOutside);
    window.addEventListener("blur", handleBlur);

    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
      window.removeEventListener("keyup", handleKeyUp, true);
      document.removeEventListener("mousedown", handlePointerOutside);
      window.removeEventListener("blur", handleBlur);
    };
  }, [isCapturing, osType, resetCapture]);

  return { containerRef, displayKeys };
};
