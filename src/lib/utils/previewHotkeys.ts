import {
  canonicalizeHotkeyToken,
  formatKeyCombination,
  getKeyName,
  normalizeHotkeyString,
  normalizeKey,
  type OSType,
} from "./keyboard";

export function normalizePreviewHotkeyString(value: string): string {
  return normalizeHotkeyString(value);
}

export function buildPreviewHotkeyFromKeyboardEvent(
  event: KeyboardEvent,
  osType: OSType,
): string | null {
  const rawKey = normalizeKey(getKeyName(event, osType));
  const key = canonicalizeHotkeyToken(rawKey);
  if (!key) {
    return null;
  }

  const parts: string[] = [];
  if (event.ctrlKey) {
    parts.push("ctrl");
  }
  if (event.shiftKey) {
    parts.push("shift");
  }
  if (event.altKey) {
    parts.push("alt");
  }
  if (event.metaKey) {
    parts.push("win");
  }
  parts.push(key);

  const normalized = normalizePreviewHotkeyString(parts.join("+"));
  return normalized || null;
}

export function formatPreviewHotkeyForDisplay(
  hotkey: string,
  osType: OSType,
): string {
  const normalized = normalizePreviewHotkeyString(hotkey);
  if (!normalized) {
    return "";
  }
  return formatKeyCombination(normalized, osType);
}
