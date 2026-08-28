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
