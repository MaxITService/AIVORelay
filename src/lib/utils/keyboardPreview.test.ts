import { expect, it } from "bun:test";
import {
  canonicalizeHotkeyToken,
  formatKeyCombination,
  getKeyName,
  isModifierOnlyShortcut,
  normalizeHotkeyString,
  normalizeKey,
} from "./keyboard";
import {
  buildPreviewHotkeyFromKeyboardEvent,
} from "./previewHotkeys";

it("canonicalizes modifier aliases and orders hotkey tokens canonically without duplicates", () => {
  expect(canonicalizeHotkeyToken("control")).toBe("ctrl");
  expect(canonicalizeHotkeyToken("OPTION")).toBe("alt");
  expect(canonicalizeHotkeyToken("command")).toBe("win");
  expect(canonicalizeHotkeyToken("meta")).toBe("win");
  expect(canonicalizeHotkeyToken("super")).toBe("win");
  expect(canonicalizeHotkeyToken("shift")).toBe("shift");
  expect(canonicalizeHotkeyToken("f5")).toBe("f5");
  expect(canonicalizeHotkeyToken("  ")).toBe("");

  expect(normalizeHotkeyString("super+option+control+shift")).toBe(
    "ctrl+shift+alt+win",
  );
  expect(
    normalizeHotkeyString("ctrl+control+shift+shift+alt+option+win+meta+k"),
  ).toBe("ctrl+shift+alt+win+k");
  expect(normalizeHotkeyString("z+win+alt+ctrl+a")).toBe("ctrl+alt+win+z+a");
  expect(normalizeHotkeyString("")).toBe("");
  expect(normalizeHotkeyString("   ")).toBe("");
});

it("identifies modifier-only shortcuts across side-specific and delimited variants", () => {
  expect(isModifierOnlyShortcut("ctrl+alt")).toBe(true);
  expect(isModifierOnlyShortcut("ctrl_left+alt_right")).toBe(true);
  expect(isModifierOnlyShortcut("left-shift+right_meta+fn")).toBe(true);
  expect(isModifierOnlyShortcut("cmd+opt+windows+super")).toBe(true);

  expect(isModifierOnlyShortcut("ctrl+space")).toBe(false);
  expect(isModifierOnlyShortcut("alt+f4")).toBe(false);
  expect(isModifierOnlyShortcut("shift+escape")).toBe(false);
  expect(isModifierOnlyShortcut("")).toBe(false);

  expect(normalizeKey("left ctrl")).toBe("ctrl");
  expect(normalizeKey("right shift")).toBe("shift");
  expect(normalizeKey("ctrl")).toBe("ctrl");
  expect(normalizeKey("left arrow key")).toBe("left arrow key");
});

it("formats key combinations with OS-specific modifier names and casing rules", () => {
  expect(formatKeyCombination("ctrl+shift+alt+win", "macos")).toBe(
    "Ctrl + Shift + Opt + Cmd",
  );
  expect(formatKeyCombination("ctrl+shift+alt+win", "windows")).toBe(
    "Ctrl + Shift + Alt + Win",
  );
  expect(formatKeyCombination("ctrl+shift+alt+win", "linux")).toBe(
    "Ctrl + Shift + Alt + Super",
  );

  expect(formatKeyCombination("shift_left+ctrl_right+k", "windows")).toBe(
    "Left Shift + Right Ctrl + K",
  );
  expect(formatKeyCombination("fn+f12+caps lock+numadd", "windows")).toBe(
    "fn + F12 + Caps Lock + Numpad +",
  );
  expect(formatKeyCombination("ctrl+numpad+", "windows")).toBe(
    "Ctrl + Numpad +",
  );

  expect(getKeyName({ code: "AltLeft" } as KeyboardEvent, "macos")).toBe(
    "option",
  );
  expect(getKeyName({ code: "MetaLeft" } as KeyboardEvent, "windows")).toBe(
    "win",
  );
  expect(getKeyName({ code: "MetaLeft" } as KeyboardEvent, "linux")).toBe(
    "super",
  );
  expect(getKeyName({ key: " " } as KeyboardEvent, "windows")).toBe("space");
  expect(getKeyName({} as KeyboardEvent, "windows")).toBe("unknown-0");
});

it("builds canonical preview hotkeys from KeyboardEvent modifier flags and key codes", () => {
  const standardEvent = {
    code: "KeyP",
    ctrlKey: true,
    altKey: true,
    shiftKey: false,
    metaKey: false,
  } as unknown as KeyboardEvent;
  expect(buildPreviewHotkeyFromKeyboardEvent(standardEvent, "windows")).toBe(
    "ctrl+alt+p",
  );

  const modifierOnlyEvent = {
    code: "ShiftLeft",
    shiftKey: true,
    ctrlKey: false,
    altKey: false,
    metaKey: false,
  } as unknown as KeyboardEvent;
  expect(
    buildPreviewHotkeyFromKeyboardEvent(modifierOnlyEvent, "windows"),
  ).toBe("shift");

  const metaMacEvent = {
    code: "MetaLeft",
    metaKey: true,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
  } as unknown as KeyboardEvent;
  expect(buildPreviewHotkeyFromKeyboardEvent(metaMacEvent, "macos")).toBe(
    "win",
  );

  const functionKeyEvent = {
    code: "F9",
    ctrlKey: true,
    shiftKey: true,
    altKey: false,
    metaKey: false,
  } as unknown as KeyboardEvent;
  expect(buildPreviewHotkeyFromKeyboardEvent(functionKeyEvent, "linux")).toBe(
    "ctrl+shift+f9",
  );

  const keyFallbackEvent = {
    key: "Escape",
    ctrlKey: true,
    shiftKey: false,
    altKey: false,
    metaKey: false,
  } as unknown as KeyboardEvent;
  expect(buildPreviewHotkeyFromKeyboardEvent(keyFallbackEvent, "windows")).toBe(
    "ctrl+esc",
  );

  const emptyEvent = {
    code: "   ",
    key: "   ",
  } as unknown as KeyboardEvent;
  expect(
    buildPreviewHotkeyFromKeyboardEvent(emptyEvent, "windows"),
  ).toBeNull();
});
