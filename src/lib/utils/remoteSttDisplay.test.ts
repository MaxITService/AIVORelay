import assert from "node:assert";
import { getRemoteApiDisplayLabel } from "./remoteSttDisplay";

assert.equal(
  getRemoteApiDisplayLabel({
    provider_preset: "openai",
    model_id: "gpt-transcribe",
  }),
  "OpenAI gpt-transcribe",
);
assert.equal(
  getRemoteApiDisplayLabel({
    provider_preset: "openai",
    model_id: "gpt-live-transcribe",
  }),
  "OpenAI gpt-live-transcribe",
);
assert.equal(
  getRemoteApiDisplayLabel({
    provider_preset: "vercel",
    model_id: "google/gemini-3.5-transcribe",
  }),
  "Vercel: Gemini 3.5 Transcribe",
);
assert.equal(
  getRemoteApiDisplayLabel({
    provider_preset: "google",
    model_id: "gemini-3.5-transcribe",
  }),
  "Google: Gemini 3.5 Transcribe",
);
assert.equal(
  getRemoteApiDisplayLabel({
    provider_preset: "vercel",
    model_id: "google/gemini-3.5-transcribe-live",
  }),
  "Vercel: Gemini 3.5 Transcribe Live",
);
assert.equal(
  getRemoteApiDisplayLabel({
    provider_preset: "google",
    model_id: "gemini-3.5-transcribe-live",
  }),
  "Google: Gemini 3.5 Transcribe Live",
);
assert.equal(
  getRemoteApiDisplayLabel({
    provider_preset: "reson8",
    model_id: "rumble-2",
  }),
  "Reson8: rumble-2",
);
assert.equal(
  getRemoteApiDisplayLabel({
    provider_preset: "elevenlabs",
    model_id: "scribe_v3_live",
  }),
  "ElevenLabs: Scribe v3 Live",
);
assert.equal(
  getRemoteApiDisplayLabel({
    provider_preset: "elevenlabs",
    model_id: "scribe_v3",
  }),
  "ElevenLabs: Scribe v3",
);
assert.equal(
  getRemoteApiDisplayLabel({
    provider_preset: "elevenlabs",
    model_id: "",
  }),
  "ElevenLabs: Scribe v3 Live",
);
assert.equal(
  getRemoteApiDisplayLabel({
    provider_preset: "elevenlabs",
    model_id: "scribe_v2",
  }),
  "ElevenLabs: scribe_v2",
);
