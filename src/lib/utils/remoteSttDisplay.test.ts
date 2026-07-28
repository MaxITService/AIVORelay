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
