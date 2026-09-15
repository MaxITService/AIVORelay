import { describe, expect, it } from "bun:test";
import type {
  AppSettings,
  ShortcutBinding,
  TranscriptionProfile,
} from "@/bindings";
import { buildHotkeyGuideCategories, hotkeyGuideManifest } from "./hotkeyGuide";

const binding = (
  id: string,
  overrides: Partial<ShortcutBinding> = {},
): ShortcutBinding => ({
  id,
  name: id,
  description: "",
  default_binding: "ctrl+shift+x",
  current_binding: "ctrl+shift+x",
  ...overrides,
});

const baseSettings = {
  voice_command_enabled: true,
  send_to_extension_enabled: true,
  send_to_extension_with_selection_enabled: true,
  send_screenshot_to_extension_enabled: true,
  text_replacement_decapitalize_after_edit_key_enabled: false,
  tts: {
    enabled: true,
    interactive_history_enabled: true,
    play_history_when_overlay_closed: true,
  },
} as unknown as AppSettings;

describe("hotkeyGuideManifest", () => {
  it("declares the feature gates consumed by shortcut filtering", () => {
    expect(hotkeyGuideManifest.featureGates).toEqual({
      voice_command: "voice_command_enabled",
      send_to_extension: "send_to_extension_enabled",
      send_to_extension_with_selection:
        "send_to_extension_with_selection_enabled",
      send_screenshot_to_extension: "send_screenshot_to_extension_enabled",
      read_clipboard: "tts.enabled",
      read_selection_tts: "tts.enabled",
      read_selection_direct_tts: "tts.directSelectionEnabled",
      tts_play_history_fallback: "tts.historyFallbackEnabled",
    });
    expect(hotkeyGuideManifest.version).toBe(1);
    expect(hotkeyGuideManifest.categories.length).toBeGreaterThan(0);
  });
});

describe("buildHotkeyGuideCategories", () => {
  it("includes assigned bindings in their declared categories", () => {
    const bindings = {
      transcribe: binding("transcribe"),
      cancel: binding("cancel"),
      ai_replace_selection: binding("ai_replace_selection"),
      voice_command: binding("voice_command"),
    };
    const categories = buildHotkeyGuideCategories(bindings, [], baseSettings);
    const byId = Object.fromEntries(
      categories.map((category) => [category.id, category]),
    );

    expect(byId.recording.hotkeys.map(h => h.id)).toEqual(["transcribe", "cancel"]);
    expect(byId.actions.hotkeys.map(h => h.id)).toEqual([
      "ai_replace_selection",
      "voice_command",
    ]);
    expect(byId.profiles).toBeUndefined(); // no profiles => category dropped
  });

  it("includes every assigned TTS binding only while its action is available", () => {
    const bindings = {
      read_clipboard: binding("read_clipboard"),
      read_selection_tts: binding("read_selection_tts"),
      read_selection_direct_tts: binding("read_selection_direct_tts"),
      tts_play_history_fallback: binding("tts_play_history_fallback"),
    };
    const categories = buildHotkeyGuideCategories(
      bindings,
      [],
      baseSettings,
      "windows",
    );
    const ttsIds = categories
      .find((category) => category.id === "textToSpeech")!
      .hotkeys.map((hotkey) => hotkey.id);

    expect(ttsIds).toEqual([
      "read_clipboard",
      "read_selection_tts",
      "read_selection_direct_tts",
      "tts_play_history_fallback",
    ]);

    const disabledSettings = {
      ...baseSettings,
      tts: {
        ...baseSettings.tts,
        enabled: false,
      },
    } as unknown as AppSettings;
    expect(
      buildHotkeyGuideCategories(
        bindings,
        [],
        disabledSettings,
        "windows",
      ),
    ).toHaveLength(0);

    const nonWindowsIds = buildHotkeyGuideCategories(
      bindings,
      [],
      baseSettings,
      "linux",
    )
      .find((category) => category.id === "textToSpeech")!
      .hotkeys.map((hotkey) => hotkey.id);
    expect(nonWindowsIds).not.toContain("read_selection_direct_tts");

    const historyDisabledSettings = {
      ...baseSettings,
      tts: {
        ...baseSettings.tts,
        play_history_when_overlay_closed: false,
      },
    } as unknown as AppSettings;
    const historyDisabledIds = buildHotkeyGuideCategories(
      bindings,
      [],
      historyDisabledSettings,
      "windows",
    )
      .find((category) => category.id === "textToSpeech")!
      .hotkeys.map((hotkey) => hotkey.id);
    expect(historyDisabledIds).not.toContain("tts_play_history_fallback");
  });

  it("routes every transcribe_ binding to Profiles, including per-profile ids", () => {
    const profiles = [{ id: "8f2c" }, { id: "9a1b" }] as unknown as TranscriptionProfile[];
    const bindings = {
      transcribe_default: binding("transcribe_default"),
      transcribe_8f2c: binding("transcribe_8f2c"),
      transcribe_9a1b: binding("transcribe_9a1b"),
      transcribe_not_a_profile: binding("transcribe_not_a_profile"),
    };
    const categories = buildHotkeyGuideCategories(
      bindings,
      profiles,
      baseSettings,
    );
    const profilesCategory = categories.find(c => c.id === "profiles")!;
    const profileIds = profilesCategory.hotkeys.map(h => h.id);

    expect(profileIds).toContain("transcribe_8f2c");
    expect(profileIds).toContain("transcribe_9a1b");
    // transcribe_not_a_profile has no matching profile => dropped
    expect(profileIds).not.toContain("transcribe_not_a_profile");
  });

  it("drops unassigned bindings and feature-gated bindings", () => {
    const gatedOff = {
      ...baseSettings,
      voice_command_enabled: false,
    } as unknown as AppSettings;
    const categories = buildHotkeyGuideCategories(
      { voice_command: binding("voice_command") },
      [],
      gatedOff,
    );
    expect(categories.find(c => c.id === "actions")?.hotkeys ?? []).toHaveLength(0);

    // A binding with an empty current_binding is unassigned and must be dropped.
    const categories2 = buildHotkeyGuideCategories(
      { transcribe: binding("transcribe", { current_binding: "  " }) },
      [],
      baseSettings,
    );
    expect(categories2).toHaveLength(0);
  });

  it("gates send_selected_text_ presets on per-preset enablement", () => {
    const withPreset = {
      ...baseSettings,
      send_selected_text: {
        presets: [{ id: "preset-1", enabled: true }],
      },
    } as unknown as AppSettings;
    const categories = buildHotkeyGuideCategories(
      { "send_selected_text_preset-1": binding("send_selected_text_preset-1") },
      [],
      withPreset,
    );
    const actions = categories.find(c => c.id === "actions")!;
    expect(actions.hotkeys.map(h => h.id)).toEqual(["send_selected_text_preset-1"]);

    // Same preset disabled => binding dropped even though the id is assigned.
    const withPresetOff = {
      ...withPreset,
      send_selected_text: {
        presets: [{ id: "preset-1", enabled: false }],
      },
    } as unknown as AppSettings;
    const categories2 = buildHotkeyGuideCategories(
      { "send_selected_text_preset-1": binding("send_selected_text_preset-1") },
      [],
      withPresetOff,
    );
    expect(categories2.find(c => c.id === "actions")?.hotkeys ?? []).toHaveLength(0);
  });

});
