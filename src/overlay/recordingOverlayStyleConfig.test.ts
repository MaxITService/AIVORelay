import { expect, test } from "bun:test";
import {
  DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG,
  parseRecordingOverlayStyleConfig,
  RECORDING_OVERLAY_STYLE_SETTING_ENTRIES,
  serializeRecordingOverlayStyleConfig,
} from "./recordingOverlayStyleConfig";

test("version 2 round-trips the complete appearance without position or mode", () => {
  const style = {
    ...DEFAULT_RECORDING_OVERLAY_STYLE_CONFIG,
    widthPx: 320,
    showCancelButton: false,
    statusIconColor: "#123456",
    cancelIconColor: "#abcdef",
    decapitalizeIndicatorMode: "custom" as const,
    decapitalizeIndicatorCustomText: "🟢 ready",
    decapitalizeIndicatorFontFamily: "Consolas",
    decapitalizeIndicatorFontSizePx: 18,
    decapitalizeIndicatorColor: "#654321",
  };
  const code = serializeRecordingOverlayStyleConfig(style);
  expect(JSON.parse(code).version).toBe(2);
  expect(parseRecordingOverlayStyleConfig(code)).toEqual(style);
  const settings = Object.fromEntries(RECORDING_OVERLAY_STYLE_SETTING_ENTRIES(style));
  expect(settings.recording_overlay_width_px).toBe(320);
  expect(settings.recording_overlay_show_cancel_button).toBe(false);
  expect(settings.recording_overlay_decapitalize_indicator_custom_text).toBe("🟢 ready");
  expect(settings).not.toHaveProperty("overlay_position");
  expect(settings).not.toHaveProperty("recording_overlay_custom_enabled");
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
    ["decapitalizeIndicatorMode", "unknown"],
    ["decapitalizeIndicatorFontFamily", "Unknown Font"],
    ["decapitalizeIndicatorCustomText", "a".repeat(25)],
    ["decapitalizeIndicatorCustomText", "two\nlines"],
  ]) {
    expect(() => parseRecordingOverlayStyleConfig(JSON.stringify({ [field as string]: value })))
      .toThrow(field as string);
  }
});
