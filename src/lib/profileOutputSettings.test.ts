import { describe, expect, test } from "bun:test";
import { isSupportedOutputKey } from "./profileOutputSettings";
import { getKeyName, normalizeHotkeyString } from "./utils/keyboard";

describe("profile opening key capture", () => {
  test("accepts standard captured lock and numpad keys", () => {
    for (const code of [
      "CapsLock", "Numpad0", "Numpad9", "NumpadMultiply", "NumpadAdd",
      "NumpadSubtract", "NumpadDecimal", "NumpadDivide",
    ]) {
      for (const osType of ["windows", "linux", "macos"] as const) {
        const key = getKeyName({ code } as KeyboardEvent, osType);
        expect(isSupportedOutputKey(normalizeHotkeyString(`ctrl+${key}`), osType)).toBe(true);
      }
    }
  });

  test("applies the backend's platform limits", () => {
    for (const key of ["insert", "printscreen", "pause", "numlock", "scrolllock", "menu", "f21", "f24"]) {
      expect(isSupportedOutputKey(key, "windows")).toBe(true);
      expect(isSupportedOutputKey(key, "linux")).toBe(true);
      expect(isSupportedOutputKey(key, "macos")).toBe(false);
    }
    expect(isSupportedOutputKey("command+f20", "macos")).toBe(true);
  });

  test("rejects unsupported keys, modifier-only input, and multiple primary keys", () => {
    for (const key of ["", "ctrl", "ctrl+shift", "ctrl+", "t+y", "capslock+t", "audiovolumeup", "numpadenter", "f25", "numpad 10"]) {
      expect(isSupportedOutputKey(key, "windows")).toBe(false);
    }
    expect(isSupportedOutputKey("ctrl+shift+t", "windows")).toBe(true);
    expect(isSupportedOutputKey("alt+/", "windows")).toBe(true);
  });
});
