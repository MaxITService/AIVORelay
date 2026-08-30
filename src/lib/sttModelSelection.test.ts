import { describe, expect, it } from "bun:test";
import type { SttModelSelection } from "./sttModelSelection";
import {
  sttCatalog,
  sttModelDropdownOptions,
  sttModelCapabilities,
  sttSelectionKey,
  sttSupports,
} from "./sttModelSelection";

describe("workflow-specific STT model menus", () => {
  it("keeps Live Monitor limited to technically compatible live models", () => {
    const options = sttCatalog("live", []);

    expect(options.map(option => option.id)).toEqual([
      "soniox:stt-rt-v5",
      "deepgram:nova-3",
      "vercel:google/gemini-3.5-transcribe-live",
      "google:gemini-3.5-transcribe-live",
    ]);
    expect(options.every(option => option.capabilities.workflows.includes("live"))).toBe(true);
    expect(options.some(option => option.selection.provider === "local")).toBe(false);
  });

  it("keeps Transcribe File file-compatible and exposes route-specific Gemini controls", () => {
    const localModel = {
      id: "local-test-model",
      name: "Local Test Model",
    } as Parameters<typeof sttCatalog>[1][number];
    const optionIds = sttCatalog("file", [localModel]).map(option => option.id);

    expect(optionIds).toContain("local:local-test-model");
    expect(optionIds).toContain("soniox:stt-async-v5");
    expect(optionIds).toContain("vercel:google/gemini-3.5-transcribe");
    expect(optionIds).toContain("google:gemini-3.5-transcribe");
    expect(optionIds.some(id => id.includes("transcribe-live"))).toBe(false);

    const googleGemini: SttModelSelection = {
      provider: "remote_openai_compatible",
      provider_preset: "google",
      model_id: "gemini-3.5-transcribe",
    };
    const vercelGemini: SttModelSelection = {
      provider: "remote_openai_compatible",
      provider_preset: "vercel",
      model_id: "google/gemini-3.5-transcribe",
    };
    const liveGemini: SttModelSelection = {
      provider: "remote_openai_compatible",
      provider_preset: "vercel",
      model_id: "google/gemini-3.5-transcribe-live",
    };

    expect(sttSupports(googleGemini, "diarization", "file")).toBe(true);
    expect(sttSupports(vercelGemini, "diarization", "file")).toBe(false);
    expect(sttSupports(liveGemini, "languageHints", "live")).toBe(true);
    expect(sttSupports(liveGemini, "vocabulary", "live")).toBe(true);
    expect(sttModelCapabilities(liveGemini).workflows).not.toContain("file");
  });

  it("keeps unprepared compatible models selectable and explains their warning", () => {
    const catalog = sttCatalog("file", []);
    const unprepared = catalog.find(option => option.id === "google:gemini-3.5-transcribe")!;
    const reason = "API key is missing";
    const options = sttModelDropdownOptions(
      catalog,
      new Map([[sttSelectionKey(unprepared.selection), reason]]),
    );

    expect(options).toHaveLength(catalog.length);
    expect(options.find(option => option.value === sttSelectionKey(unprepared.selection))).toEqual({
      value: sttSelectionKey(unprepared.selection),
      label: `⚠ ${unprepared.modelLabel}`,
      className: "text-red-400",
      title: reason,
    });
    expect(
      options
        .find(option => option.value !== sttSelectionKey(unprepared.selection))
        ?.label.startsWith("⚠ "),
    ).toBe(false);
  });
});
