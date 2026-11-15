import { expect, test } from "bun:test";
import {
  DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG,
  parseRecordingOverlayStyleConfig,
  RECORDING_OVERLAY_PRESET_PRESERVED_FIELDS,
  RECORDING_OVERLAY_STYLE_PRESETS,
  RECORDING_OVERLAY_STYLE_SETTING_ENTRIES,
  resolveRecordingOverlayPresetConfig,
  serializeRecordingOverlayStyleConfig,
  type RecordingOverlayStyleConfig,
} from "./recordingOverlayStyleConfig";
import { resolveRecordingOverlayStatusIconStyle } from "./recordingOverlayAppearance";

test("version 4 round-trips the complete appearance without position or mode", () => {
  const style = {
    ...DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG,
    statusIconStyle: "ring" as const,
    widthPx: 320,
    showCancelButton: false,
    cancelButtonInvisible: true,
    statusIconColor: "#123456",
    cancelIconColor: "#abcdef",
    decapitalizeIndicatorMode: "custom" as const,
    decapitalizeIndicatorCustomText: "🟢 ready",
    decapitalizeIndicatorFontFamily: "Consolas",
    decapitalizeIndicatorFontSizePx: 18,
    decapitalizeIndicatorColor: "#654321",
  };
  const code = serializeRecordingOverlayStyleConfig(style);
  expect(JSON.parse(code).version).toBe(4);
  expect(parseRecordingOverlayStyleConfig(code)).toEqual(style);
  const settings = Object.fromEntries(RECORDING_OVERLAY_STYLE_SETTING_ENTRIES(style));
  expect(settings.recording_overlay_width_px).toBe(320);
  expect(settings.recording_overlay_show_cancel_button).toBe(false);
  expect(settings.recording_overlay_decapitalize_indicator_custom_text).toBe("🟢 ready");
  expect(settings).not.toHaveProperty("overlay_position");
  expect(settings).not.toHaveProperty("recording_overlay_custom_enabled");
  expect(settings.recording_overlay_status_icon_style).toBe("ring");
  expect(settings.recording_overlay_cancel_button_invisible).toBe(true);
});

test("version 2 codes import without the icon frame, which falls back to Auto", () => {
  const v2Style: Record<string, unknown> = {
    ...DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG,
    barStyle: "aurora",
  };
  delete v2Style.statusIconStyle;
  delete v2Style.cancelButtonInvisible;
  const current = { ...DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG, statusIconStyle: "dot" as const };
  expect(parseRecordingOverlayStyleConfig(JSON.stringify({ version: 2, style: v2Style }), current))
    .toEqual({ ...DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG, barStyle: "aurora", statusIconStyle: "auto" });
  expect(() => parseRecordingOverlayStyleConfig(JSON.stringify({ version: 3, style: v2Style })))
    .toThrow("Missing style fields: statusIconStyle");
});

test("version 3 codes import without the invisible cancel flag, which stays off", () => {
  const v3Style: Record<string, unknown> = {
    ...DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG,
    barStyle: "aurora",
  };
  delete v3Style.cancelButtonInvisible;
  const current = { ...DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG, cancelButtonInvisible: true };
  expect(parseRecordingOverlayStyleConfig(JSON.stringify({ version: 3, style: v3Style }), current))
    .toEqual({ ...DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG, barStyle: "aurora" });
  expect(() => parseRecordingOverlayStyleConfig(JSON.stringify({ version: 4, style: v3Style })))
    .toThrow("Missing style fields: cancelButtonInvisible");
});

test("Auto keeps the glass capsule on original materials and drops it on flat ones", () => {
  expect(resolveRecordingOverlayStatusIconStyle("auto", "liquid_glass")).toBe("capsule");
  expect(resolveRecordingOverlayStatusIconStyle("auto", "velvet_neon")).toBe("capsule");
  for (const material of ["graphite", "obsidian", "gradient_mesh"] as const) {
    expect(resolveRecordingOverlayStatusIconStyle("auto", material)).toBe("bare");
  }
  expect(resolveRecordingOverlayStatusIconStyle("auto", "clay")).toBe("orb");
  expect(resolveRecordingOverlayStatusIconStyle("auto", "keycap")).toBe("tile");
  expect(resolveRecordingOverlayStatusIconStyle("tile", "graphite")).toBe("tile");
  expect(resolveRecordingOverlayStatusIconStyle("capsule", "obsidian")).toBe("capsule");
});

test("legacy imports preserve appearance fields absent from the old code", () => {
  const current = {
    ...DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG,
    widthPx: 360,
    showCancelButton: false,
    cancelIconColor: "#123456",
    decapitalizeIndicatorMode: "hidden" as const,
  };
  const style = parseRecordingOverlayStyleConfig(JSON.stringify({
    version: 1, style: { bar_style: "aurora", accent_color: "#ABCDEF" },
  }), current);
  expect(style).toEqual({ ...current, barStyle: "aurora", accentColor: "#abcdef" });
  expect(parseRecordingOverlayStyleConfig('{"bar_count":12}', current))
    .toEqual({ ...current, barCount: 12 });
});

test("encoded UTF-8 style codes remain supported", () => {
  const style = { ...DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG, decapitalizeIndicatorCustomText: "🎙️" };
  const code = Buffer.from(serializeRecordingOverlayStyleConfig(style)).toString("base64");
  expect(parseRecordingOverlayStyleConfig(`aivo-overlay:${code}`)).toEqual(style);
});

test("malformed or unrelated imports cannot silently reset the appearance", () => {
  for (const payload of [
    {}, [], null, { unrelated: true },
    { version: 99, style: { theme: "glass" } },
    { version: 1, style: [] },
    { version: 2, style: { theme: "glass" } },
    { version: 1, style: { theme: "glass" }, unexpected: true },
    { theme: "glass", barStyle: "aurora", bar_style: "solid" },
  ]) {
    expect(() => parseRecordingOverlayStyleConfig(JSON.stringify(payload))).toThrow();
  }
  expect(() => parseRecordingOverlayStyleConfig("aivo-overlay:%%%"))
    .toThrow("invalid encoded data");
});

test("imports reject unsupported values with the offending field name", () => {
  for (const [field, value] of [
    ["barCount", "9"], ["barCount", 17], ["barCount", 4.5],
    ["widthPx", 100], ["opacityPercent", 0], ["showCancelButton", null],
    ["theme", "unknown"], ["barStyle", "unknown"], ["accentColor", "red"],
    ["statusIconStyle", "unknown"], ["cancelButtonInvisible", "yes"],
    ["decapitalizeIndicatorMode", "unknown"],
    ["decapitalizeIndicatorFontFamily", "Unknown Font"],
    ["decapitalizeIndicatorCustomText", "a".repeat(25)],
    ["decapitalizeIndicatorCustomText", "two\nlines"],
  ]) {
    expect(() => parseRecordingOverlayStyleConfig(JSON.stringify({ [field as string]: value })))
      .toThrow(field as string);
  }
});

test("built-in presets replace the look but keep the user's width, icons, and indicator", () => {
  const preserved: readonly string[] = RECORDING_OVERLAY_PRESET_PRESERVED_FIELDS;
  const current: RecordingOverlayStyleConfig = {
    ...DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG,
    widthPx: 300,
    showCancelButton: false,
    cancelButtonInvisible: true,
    statusIconColor: "#111111",
    cancelIconColor: "#222222",
    decapitalizeIndicatorMode: "custom",
    decapitalizeIndicatorCustomText: "aa",
    decapitalizeIndicatorFontFamily: "Consolas",
    decapitalizeIndicatorFontSizePx: 20,
    decapitalizeIndicatorColor: "#333333",
    // A preset that leaves these unset must restore the defaults.
    showDragGrip: !DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG.showDragGrip,
    opacityPercent: 41,
  };
  const keys = Object.keys(DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG) as Array<
    keyof RecordingOverlayStyleConfig
  >;
  for (const preset of RECORDING_OVERLAY_STYLE_PRESETS) {
    expect(Object.keys(preset.config).filter((key) => preserved.includes(key))).toEqual([]);
    const resolved = resolveRecordingOverlayPresetConfig(preset, current);
    for (const key of keys) {
      const expected = preserved.includes(key)
        ? current[key]
        : key in preset.config
          ? preset.config[key]
          : DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG[key];
      // A mismatch on a preset field means normalization rejected a typo.
      expect([preset.id, key, resolved[key]]).toEqual([preset.id, key, expected]);
    }
  }
});

test("built-in preset ids are unique", () => {
  const ids = RECORDING_OVERLAY_STYLE_PRESETS.map((preset) => preset.id);
  expect(new Set(ids).size).toBe(ids.length);
});
