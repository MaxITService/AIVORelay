import { describe, expect, it } from "bun:test";
import type { TFunction } from "i18next";
import {
  getConfiguredCustomRemoteSttModelId,
  getRemoteApiRows,
  resolveRemoteApiRow,
  resolveRemoteApiSelection,
  type RemoteApiRowId,
} from "./remoteApiModels";

const t = ((key: string) => key) as unknown as TFunction;

const getRow = (id: RemoteApiRowId) => {
  const row = getRemoteApiRows(t).find((entry) => entry.id === id);
  if (!row) {
    throw new Error(`Missing remote API row fixture for ${id}`);
  }
  return row;
};

describe("getConfiguredCustomRemoteSttModelId", () => {
  it("returns the trimmed custom model id only when custom preset, base_url, and model_id are configured", () => {
    expect(
      getConfiguredCustomRemoteSttModelId({
        provider_preset: "custom",
        base_url: " https://example.com/v1 ",
        model_id: " my-custom-stt ",
      }),
    ).toBe("my-custom-stt");
  });

  it("returns null when global remote_stt is missing or using another preset", () => {
    expect(getConfiguredCustomRemoteSttModelId(undefined)).toBeNull();
    expect(getConfiguredCustomRemoteSttModelId(null)).toBeNull();
    expect(
      getConfiguredCustomRemoteSttModelId({
        provider_preset: "groq",
        base_url: "https://example.com/v1",
        model_id: "whisper-large-v3-turbo",
      }),
    ).toBeNull();
  });

  it("treats empty or whitespace-only base_url or model_id as unconfigured without inventing defaults", () => {
    expect(
      getConfiguredCustomRemoteSttModelId({
        provider_preset: "custom",
        base_url: "   ",
        model_id: "my-custom-stt",
      }),
    ).toBeNull();
    expect(
      getConfiguredCustomRemoteSttModelId({
        provider_preset: "custom",
        base_url: "https://example.com/v1",
        model_id: "",
      }),
    ).toBeNull();
    expect(
      getConfiguredCustomRemoteSttModelId({
        provider_preset: "custom",
        base_url: "https://example.com/v1",
        model_id: "   ",
      }),
    ).toBeNull();
  });
});

describe("resolveRemoteApiSelection", () => {
  it("preserves Default-profile global target behavior including custom without modelId", () => {
    const customRow = getRow("custom");
    const groqRow = getRow("groq");
    const geminiLiveRow = getRow("gemini_live");

    expect(
      resolveRemoteApiSelection(customRow, {
        isCustomProfile: false,
        currentEffectivePreset: "groq",
        globalRemoteStt: {
          provider_preset: "groq",
          base_url: "https://api.groq.com/openai/v1",
          model_id: "whisper-large-v3-turbo",
        },
      }),
    ).toEqual({
      target: "global",
      preset: "custom",
      modelId: undefined,
    });

    expect(
      resolveRemoteApiSelection(groqRow, {
        isCustomProfile: false,
        currentEffectivePreset: "custom",
      }),
    ).toEqual({
      target: "global",
      preset: "groq",
      modelId: "whisper-large-v3-turbo",
    });

    expect(
      resolveRemoteApiSelection(geminiLiveRow, {
        isCustomProfile: false,
        currentEffectivePreset: "google",
      }),
    ).toEqual({
      target: "global",
      preset: "google",
      modelId: "gemini-3.5-transcribe-live",
    });
  });

  it("blocks custom row selection under a custom profile when global Custom is unconfigured", () => {
    const customRow = getRow("custom");

    expect(
      resolveRemoteApiSelection(customRow, {
        isCustomProfile: true,
        currentEffectivePreset: "groq",
        globalRemoteStt: {
          provider_preset: "groq",
          base_url: "https://api.groq.com/openai/v1",
          model_id: "whisper-large-v3-turbo",
        },
      }),
    ).toEqual({
      target: "blocked",
      reason: "custom_unconfigured",
    });

    expect(
      resolveRemoteApiSelection(customRow, {
        isCustomProfile: true,
        currentEffectivePreset: "custom",
        globalRemoteStt: {
          provider_preset: "custom",
          base_url: "https://example.com/v1",
          model_id: "   ",
        },
      }),
    ).toEqual({
      target: "blocked",
      reason: "custom_unconfigured",
    });

    expect(
      resolveRemoteApiSelection(customRow, {
        isCustomProfile: true,
        currentEffectivePreset: "custom",
        globalRemoteStt: {
          provider_preset: "custom",
          base_url: "",
          model_id: "custom-model",
        },
      }),
    ).toEqual({
      target: "blocked",
      reason: "custom_unconfigured",
    });
  });

  it("selects a profile-scoped Custom override using the configured global endpoint and model", () => {
    const customRow = getRow("custom");

    expect(
      resolveRemoteApiSelection(customRow, {
        isCustomProfile: true,
        currentEffectivePreset: "groq",
        globalRemoteStt: {
          provider_preset: "custom",
          base_url: "http://127.0.0.1:8080/v1",
          model_id: " local-whisper-v3 ",
        },
      }),
    ).toEqual({
      target: "profile",
      selection: {
        provider: "remote_openai_compatible",
        provider_preset: "custom",
        model_id: "local-whisper-v3",
      },
    });
  });

  it("resolves non-custom rows as profile overrides under a custom profile", () => {
    const reson8Row = getRow("reson8");
    const geminiLiveRow = getRow("gemini_live");

    expect(
      resolveRemoteApiSelection(reson8Row, {
        isCustomProfile: true,
        currentEffectivePreset: "groq",
        globalRemoteStt: {
          provider_preset: "groq",
          base_url: "https://api.groq.com/openai/v1",
          model_id: "whisper-large-v3-turbo",
        },
      }),
    ).toEqual({
      target: "profile",
      selection: {
        provider: "remote_openai_compatible",
        provider_preset: "reson8",
        model_id: "rumble-2",
      },
    });

    expect(
      resolveRemoteApiSelection(geminiLiveRow, {
        isCustomProfile: true,
        currentEffectivePreset: "google",
      }),
    ).toEqual({
      target: "profile",
      selection: {
        provider: "remote_openai_compatible",
        provider_preset: "google",
        model_id: "gemini-3.5-transcribe-live",
      },
    });

    // Keep resolveRemoteApiRow behavior intact for ModelSelector.
    expect(resolveRemoteApiRow(getRow("custom"), "groq")).toEqual({
      preset: "custom",
      modelId: undefined,
    });
  });

  it("exposes ElevenLabs Scribe v3 Live row and resolves global and profile targets", () => {
    const elevenLabsRow = getRow("elevenlabs");

    expect(elevenLabsRow.preset).toBe("elevenlabs");
    expect(elevenLabsRow.modelId).toBe("scribe_v3_live");
    expect(elevenLabsRow.notRecommended).toBeUndefined();

    expect(resolveRemoteApiRow(elevenLabsRow, "groq")).toEqual({
      preset: "elevenlabs",
      modelId: "scribe_v3_live",
    });

    expect(
      resolveRemoteApiSelection(elevenLabsRow, {
        isCustomProfile: false,
        currentEffectivePreset: "groq",
      }),
    ).toEqual({
      target: "global",
      preset: "elevenlabs",
      modelId: "scribe_v3_live",
    });

    expect(
      resolveRemoteApiSelection(elevenLabsRow, {
        isCustomProfile: true,
        currentEffectivePreset: "groq",
        globalRemoteStt: {
          provider_preset: "groq",
          base_url: "https://api.groq.com/openai/v1",
          model_id: "whisper-large-v3-turbo",
        },
      }),
    ).toEqual({
      target: "profile",
      selection: {
        provider: "remote_openai_compatible",
        provider_preset: "elevenlabs",
        model_id: "scribe_v3_live",
      },
    });
  });
});
