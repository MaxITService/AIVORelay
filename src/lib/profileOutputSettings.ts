import type { AutoSubmitKey, ClipboardHandling, PasteMethod } from "@/bindings";
import type { OSType } from "@/lib/utils/keyboard";

// Keep this set aligned with clipboard.rs::parse_output_key. Global shortcuts
// use other backends and intentionally accept a wider set of captured keys.
const OUTPUT_KEYS = new Set([
  "enter", "return", "tab", "space", "escape", "esc", "backspace", "delete",
  "capslock", "home", "end", "pageup", "pagedown", "up", "arrowup", "down",
  "arrowdown", "left", "arrowleft", "right", "arrowright", "numpad *",
  "numadd", "numpad -", "numpad .", "numpad /",
  ";", "=", ",", "-", ".", "/", "`", "[", "\\", "]", "'",
]);
const OUTPUT_MODIFIERS = new Set([
  "ctrl", "control", "shift", "alt", "option", "win", "super", "meta",
  "command", "cmd",
]);
const DESKTOP_OUTPUT_KEYS = new Set([
  "insert", "printscreen", "pause", "numlock", "scrolllock", "menu",
]);

export const isSupportedOutputKey = (value: string, osType: OSType): boolean => {
  const parts = value.split("+").map(part => part.trim().toLowerCase());
  const key = parts.pop() ?? "";
  if (parts.some(part => !OUTPUT_MODIFIERS.has(part))) return false;
  if (OUTPUT_KEYS.has(key) || /^[a-z0-9]$/.test(key) || /^numpad [0-9]$/.test(key)) return true;
  const functionKey = /^f([1-9]|1\d|2[0-4])$/.exec(key);
  if (functionKey) return Number(functionKey[1]) <= (osType === "macos" ? 20 : 24);
  return osType !== "macos" && DESKTOP_OUTPUT_KEYS.has(key);
};

// Absent fields use the program defaults; profiles never follow the Default profile.
export interface ProfileOutputSettings {
  paste_method?: PasteMethod | null;
  clipboard_handling?: ClipboardHandling | null;
  clipboard_history_allowed?: boolean | null;
  convert_lf_to_crlf?: boolean | null;
  paste_delay_ms?: number | null;
  check_active_window?: boolean | null;
  live_window_resume_delay_ms?: number | null;
  live_window_stop_action?: "history" | "preview" | "wait_for_window" | null;
  live_window_wait_limit_secs?: number | null;
  live_window_wait_limit_action?: "preview" | "history" | null;
  auto_submit?: boolean | null;
  auto_submit_key?: AutoSubmitKey | null;
  before_insert_key?: string | null;
  before_insert_delay_ms?: number | null;
  recording_start_key?: string | null;
  recording_start_delay_ms?: number | null;
  auto_submit_delay_ms?: number | null;
  cancel_key_after_early_open?: string | null;
}
