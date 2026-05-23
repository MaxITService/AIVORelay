import type { SidebarSection } from "@/components/Sidebar";

const SHORTCUT_SECTION_BY_ID: Record<string, SidebarSection> = {
  ai_replace_selection: "aiReplace",
  cancel: "debug",
  cycle_profile: "general",
  repaste_last: "history",
  send_screenshot_to_extension: "browserConnector",
  send_to_extension: "browserConnector",
  send_to_extension_with_selection: "browserConnector",
  spawn_button: "userInterface",
  transcribe: "general",
  transcribe_default: "general",
  voice_command: "voiceCommands",
};

export const getShortcutAnchorId = (shortcutId: string): string =>
  `shortcut-${shortcutId.replace(/[^a-zA-Z0-9_-]/g, "-")}`;

export const getShortcutSettingsSection = (
  shortcutId: string,
): SidebarSection => {
  if (shortcutId.startsWith("transcribe_")) {
    return "general";
  }

  return SHORTCUT_SECTION_BY_ID[shortcutId] ?? "general";
};
