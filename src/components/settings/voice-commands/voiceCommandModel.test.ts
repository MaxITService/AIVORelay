import { describe, expect, test } from "bun:test";

import { resolveVoiceCommandModel } from "./voiceCommandModel";

describe("Voice Command model resolution", () => {
  test("separate → inherited → separate preserves the separate model", () => {
    const settings = {
      post_process_models: { provider: "post-model-a" },
      voice_command_models: { provider: "voice-model-b" },
    };

    expect(resolveVoiceCommandModel(settings, "provider", false)).toBe(
      "voice-model-b",
    );
    expect(resolveVoiceCommandModel(settings, "provider", true)).toBe(
      "post-model-a",
    );
    expect(resolveVoiceCommandModel(settings, "provider", false)).toBe(
      "voice-model-b",
    );
  });

  test("missing, empty, and whitespace-only separate models use the post-processing model", () => {
    const base = { post_process_models: { provider: "post-model" } };

    expect(resolveVoiceCommandModel(base, "provider", false)).toBe("post-model");
    expect(
      resolveVoiceCommandModel(
        { ...base, voice_command_models: { provider: "" } },
        "provider",
        false,
      ),
    ).toBe("post-model");
    expect(
      resolveVoiceCommandModel(
        { ...base, voice_command_models: { provider: "   " } },
        "provider",
        false,
      ),
    ).toBe("post-model");
  });
});
