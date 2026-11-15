import { expect, test } from "bun:test";
import {
  buildPreviewHotkeyFromKeyboardEvent,
  formatPreviewHotkeyForDisplay,
} from "./previewHotkeys";

test("preview hotkeys use physical key codes consistently across keyboard layouts", () => {
  const event = {
    code: "KeyF",
    key: "А",
    ctrlKey: true,
    shiftKey: false,
    altKey: false,
    metaKey: false,
  } as KeyboardEvent;

  const hotkey = buildPreviewHotkeyFromKeyboardEvent(event, "windows");
  expect(hotkey).toBe("ctrl+f");
  expect(formatPreviewHotkeyForDisplay(hotkey!, "windows")).toBe("Ctrl + F");
});

test("shifted punctuation retains its physical base key and explicit shift modifier", () => {
  const event = {
    code: "Equal",
    key: "+",
    ctrlKey: true,
    shiftKey: true,
    altKey: false,
    metaKey: false,
  } as KeyboardEvent;

  const hotkey = buildPreviewHotkeyFromKeyboardEvent(event, "windows");
  expect(hotkey).toBe("ctrl+shift+=");
  expect(formatPreviewHotkeyForDisplay(hotkey!, "windows")).toBe("Ctrl + Shift + =");
});

test("numpad plus is captured as one key instead of a hotkey separator", () => {
  const event = {
    code: "NumpadAdd",
    key: "+",
    ctrlKey: true,
    shiftKey: false,
    altKey: false,
    metaKey: false,
  } as KeyboardEvent;

  const hotkey = buildPreviewHotkeyFromKeyboardEvent(event, "windows");
  expect(hotkey).toBe("ctrl+numadd");
  expect(formatPreviewHotkeyForDisplay(hotkey!, "windows")).toBe("Ctrl + Numpad +");
});
