import {
  formatKeyCombination,
  getKeyName,
  normalizeKey,
  type OSType,
} from "./keyboard";

const MODIFIER_ORDER = ["ctrl", "shift", "alt", "win"] as const;

function canonicalizeToken(token: string): string {
  const trimmed = token.trim().toLowerCase();
  if (!trimmed) {
    return "";
  }
  switch (trimmed) {
    case "control":
      return "ctrl";
    case "option":
      return "alt";
    case "command":
    case "meta":
    case "super":
      return "win";
    default:
      return trimmed;
  }
}

function sortAndDedupeParts(parts: string[]): string[] {
  const seen = new Set<string>();
  const modifiersPresent = new Set<string>();
  const nonModifiers: string[] = [];

  for (const rawPart of parts) {
    const part = canonicalizeToken(rawPart);
    if (!part || seen.has(part)) {
      continue;
    }
    seen.add(part);
    if (MODIFIER_ORDER.includes(part as (typeof MODIFIER_ORDER)[number])) {
      modifiersPresent.add(part);
    } else {
      nonModifiers.push(part);
    }
  }

  const orderedModifiers = MODIFIER_ORDER.filter((modifier) =>
    modifiersPresent.has(modifier),
  );
  return [...orderedModifiers, ...nonModifiers];
}

export function normalizePreviewHotkeyString(value: string): string {
  if (!value || typeof value !== "string") {
    return "";
  }
  const rawParts = value.split("+");
  return sortAndDedupeParts(rawParts).join("+");
}

export function buildPreviewHotkeyFromKeyboardEvent(
  event: KeyboardEvent,
  osType: OSType,
): string | null {
  const rawKey = normalizeKey(getKeyName(event, osType));
  const key = canonicalizeToken(rawKey);
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

