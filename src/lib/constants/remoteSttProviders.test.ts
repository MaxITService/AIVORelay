import { describe, expect, it } from "bun:test";
import {
  REMOTE_STT_ELEVENLABS_BASE_URL,
  REMOTE_STT_ELEVENLABS_DEFAULT_MODEL,
  REMOTE_STT_ELEVENLABS_FILE_MODEL,
  REMOTE_STT_PRESET_ELEVENLABS,
  REMOTE_STT_PRESET_IDS,
  REMOTE_STT_PRESETS,
} from "./remoteSttProviders";

describe("remoteSttProviders", () => {
  it("defines the canonical elevenlabs preset and model defaults", () => {
    expect(REMOTE_STT_PRESETS.elevenlabs).toEqual({
      id: "elevenlabs",
      label: "ElevenLabs",
      baseUrl: "https://api.elevenlabs.io/v1",
      defaultModel: "scribe_v3_live",
    });
    expect(REMOTE_STT_PRESET_ELEVENLABS).toBe("elevenlabs");
    expect(REMOTE_STT_ELEVENLABS_BASE_URL).toBe("https://api.elevenlabs.io/v1");
    expect(REMOTE_STT_ELEVENLABS_DEFAULT_MODEL).toBe("scribe_v3_live");
    expect(REMOTE_STT_ELEVENLABS_FILE_MODEL).toBe("scribe_v3");
    expect(REMOTE_STT_PRESET_IDS).toContain("elevenlabs");
  });
});
