import { describe, expect, it } from "bun:test";
import { getShortcutSettingsSection } from "./shortcutAnchors";

describe("getShortcutSettingsSection", () => {
  it("routes known shortcut ids to their settings sidebar section", () => {
    const expectedSections = {
      ai_replace_selection: "aiReplace",
      cancel: "debug",
      cycle_profile: "general",
      repaste_last: "history",
      read_clipboard: "textToSpeech",
      read_selection_tts: "textToSpeech",
      read_selection_direct_tts: "textToSpeech",
      send_screenshot_to_extension: "browserConnector",
      send_to_extension: "browserConnector",
      send_to_extension_with_selection: "browserConnector",
      spawn_button: "userInterface",
      text_replacement_decapitalize_after_edit_key: "textReplacement",
      text_replacement_decapitalize_after_edit_secondary_key: "textReplacement",
      transcribe: "general",
      transcribe_default: "general",
      voice_command: "voiceCommands",
      "send_selected_text_preset-1": "sendSelectedText",
    } as const;

    for (const [shortcutId, expectedSection] of Object.entries(expectedSections)) {
      expect(getShortcutSettingsSection(shortcutId)).toBe(expectedSection);
    }
  });

  it("routes every transcribe_ binding to General, including per-profile ids", () => {
    expect(getShortcutSettingsSection("transcribe")).toBe("general");
    expect(getShortcutSettingsSection("transcribe_default")).toBe("general");
    expect(getShortcutSettingsSection("transcribe_8f2c-profile")).toBe("general");
  });
});
