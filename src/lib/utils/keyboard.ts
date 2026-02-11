/**
 * Keyboard utility functions for handling keyboard events
 */

export type OSType = "macos" | "windows" | "linux" | "unknown";

/**
 * Extract a consistent key name from a KeyboardEvent
 * This function provides cross-platform keyboard event handling
 * and returns key names appropriate for the target operating system
 */
export const getKeyName = (
  e: KeyboardEvent,
  osType: OSType = "unknown",
): string => {
  // Handle special cases first
  if (e.code) {
    const code = e.code;

    // Handle function keys (F1-F24)
    if (code.match(/^F\d+$/)) {
      return code.toLowerCase(); // F1, F2, ..., F14, F15, etc.
    }

    // Handle regular letter keys (KeyA -> a)
    if (code.match(/^Key[A-Z]$/)) {
      return code.replace("Key", "").toLowerCase();
    }

    // Handle digit keys (Digit0 -> 0)
    if (code.match(/^Digit\d$/)) {
      return code.replace("Digit", "");
    }

    // Handle numpad digit keys (Numpad0 -> numpad 0)
    if (code.match(/^Numpad\d$/)) {
      return code.replace("Numpad", "numpad ").toLowerCase();
    }

    // Handle modifier keys - OS-specific naming
    const getModifierName = (baseModifier: string): string => {
      switch (baseModifier) {
        case "shift":
          return "shift";
        case "ctrl":
          return "ctrl";
        case "alt":
          return osType === "macos" ? "option" : "alt";
        case "meta":
          // Windows key on Windows/Linux, Command key on Mac
          if (osType === "macos") return "command";
          if (osType === "windows") return "win";
          return "super";
        default:
          return baseModifier;
      }
    };

    const modifierMap: Record<string, string> = {
      ShiftLeft: getModifierName("shift"),
      ShiftRight: getModifierName("shift"),
      ControlLeft: getModifierName("ctrl"),
      ControlRight: getModifierName("ctrl"),
      AltLeft: getModifierName("alt"),
      AltRight: getModifierName("alt"),
      MetaLeft: getModifierName("meta"),
      MetaRight: getModifierName("meta"),
      OSLeft: getModifierName("meta"),
      OSRight: getModifierName("meta"),
      CapsLock: "caps lock",
      Tab: "tab",
      Enter: "enter",
      Space: "space",
      Backspace: "backspace",
      Delete: "delete",
      Escape: "esc",
      ArrowUp: "up",
      ArrowDown: "down",
      ArrowLeft: "left",
      ArrowRight: "right",
      Home: "home",
      End: "end",
      PageUp: "page up",
      PageDown: "page down",
      Insert: "insert",
      PrintScreen: "print screen",
      ScrollLock: "scroll lock",
      Pause: "pause",
      ContextMenu: "menu",
      NumpadMultiply: "numpad *",
      NumpadAdd: "numadd",
      NumpadSubtract: "numpad -",
      NumpadDecimal: "numpad .",
      NumpadDivide: "numpad /",
      NumLock: "num lock",
    };

    if (modifierMap[code]) {
      return modifierMap[code];
    }

    // Handle punctuation and special characters
    const punctuationMap: Record<string, string> = {
      Semicolon: ";",
      Equal: "=",
      Comma: ",",
      Minus: "-",
      Period: ".",
      Slash: "/",
      Backquote: "`",
      BracketLeft: "[",
      Backslash: "\\",
      BracketRight: "]",
      Quote: "'",
    };

    if (punctuationMap[code]) {
      return punctuationMap[code];
    }

    // For any other codes, try to convert to a reasonable format
    return code.toLowerCase().replace(/([a-z])([A-Z])/g, "$1 $2");
  }

  // Fallback to e.key if e.code is not available
  if (e.key) {
    const key = e.key;

    // Handle special key names with OS-specific formatting
    const keyMap: Record<string, string> = {
      Control: osType === "macos" ? "ctrl" : "ctrl",
      Alt: osType === "macos" ? "option" : "alt",
      Shift: "shift",
      Meta:
        osType === "macos" ? "command" : osType === "windows" ? "win" : "super",
      OS:
        osType === "macos" ? "command" : osType === "windows" ? "win" : "super",
      CapsLock: "caps lock",
      ArrowUp: "up",
      ArrowDown: "down",
      ArrowLeft: "left",
      ArrowRight: "right",
      Escape: "esc",
      " ": "space",
    };

    if (keyMap[key]) {
      return keyMap[key];
    }

    return key.toLowerCase();
  }

  // Last resort fallback
  return `unknown-${e.keyCode || e.which || 0}`;
};

/**
 * Capitalize a key name for display (e.g. "space" -> "Space", "f1" -> "F1")
 */
const capitalizeKey = (key: string): string => {
  if (key === "fn") return "fn";
  if (/^f\d+$/.test(key)) return key.toUpperCase();
  if (key.length === 1) return key.toUpperCase();
  return key.replace(/\b\w/g, (c) => c.toUpperCase());
};

/**
 * Format a single key part for display.
 * Handles _left/_right suffixes, OS-aware modifier names, and capitalization.
 * e.g. "shift_left" -> "Left Shift", "super" -> "Win" (on Windows)
 */
const formatKeyPart = (part: string, osType: OSType): string => {
  const trimmed = part.trim().toLowerCase();
  if (!trimmed) return "";

  // Extract _left/_right suffix and base name
  let baseName = trimmed;
  let side: "left" | "right" | null = null;
  if (trimmed.endsWith("_left")) {
    baseName = trimmed.slice(0, -5);
    side = "left";
  } else if (trimmed.endsWith("_right")) {
    baseName = trimmed.slice(0, -6);
    side = "right";
  }

  // OS-aware modifier mapping
  const modifierMap: Record<string, Record<string, string>> = {
    super: { macos: "Cmd", windows: "Win", linux: "Super", unknown: "Super" },
    meta: { macos: "Cmd", windows: "Win", linux: "Super", unknown: "Super" },
    command: { macos: "Cmd", windows: "Win", linux: "Super", unknown: "Super" },
    win: { macos: "Cmd", windows: "Win", linux: "Super", unknown: "Super" },
    alt: { macos: "Opt", windows: "Alt", linux: "Alt", unknown: "Alt" },
    option: { macos: "Opt", windows: "Alt", linux: "Alt", unknown: "Alt" },
    ctrl: { macos: "Ctrl", windows: "Ctrl", linux: "Ctrl", unknown: "Ctrl" },
    control: { macos: "Ctrl", windows: "Ctrl", linux: "Ctrl", unknown: "Ctrl" },
    shift: { macos: "Shift", windows: "Shift", linux: "Shift", unknown: "Shift" },
  };

  if (baseName === "numadd") {
    return "Numpad +";
  }

  const mapped = modifierMap[baseName]?.[osType];
  const displayName = mapped ?? capitalizeKey(baseName);

  if (side === "left") return `Left ${displayName}`;
  if (side === "right") return `Right ${displayName}`;
  return displayName;
};

/**
 * Get display-friendly key combination string for the current OS.
 * Formats raw hotkey strings like "option_left+shift+space" into
 * human-readable form like "Left Opt + Shift + Space" (on macOS)
 * or "Left Alt + Shift + Space" (on Windows).
 */
export const formatKeyCombination = (
  combination: string,
  osType: OSType,
): string => {
  if (!combination) return "";
  const normalized = combination.replace(/numpad\s*\+/gi, "numadd");
  return normalized.split("+").map((part) => formatKeyPart(part, osType)).join(" + ");
};

/**
 * Normalize modifier keys to handle left/right variants
 */
export const normalizeKey = (key: string): string => {
  // Handle left/right variants of modifier keys
  if (key.startsWith("left ") || key.startsWith("right ")) {
    const parts = key.split(" ");
    if (parts.length === 2) {
      // Return just the modifier name without left/right prefix
      return parts[1];
    }
  }
  return key;
};
