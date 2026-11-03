import { expect, test } from "bun:test";
import { getShortcutGuideMode } from "./shortcutGuide";

test("shortcut guidance follows the active profile and automatic mode", () => {
  expect(getShortcutGuideMode(null)).toBeNull();
  expect(getShortcutGuideMode({ push_to_talk: true })).toBe("hold");
  expect(getShortcutGuideMode({ push_to_talk: false })).toBe("toggle");
  expect(getShortcutGuideMode({ push_to_talk: true, auto_shortcut_activation: true })).toBe("auto");
  const settings = { push_to_talk: true, auto_shortcut_activation: true, active_profile_id: "p", transcription_profiles: [{ id: "p", push_to_talk: false }] };
  expect(getShortcutGuideMode(settings)).toBe("toggle");
  expect(getShortcutGuideMode({ ...settings, transcription_profiles: [{ id: "p", push_to_talk: false, auto_shortcut_activation: true }] })).toBe("auto");
  expect(getShortcutGuideMode({ ...settings, active_profile_id: "missing" })).toBe("hold");
});
