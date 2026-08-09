import assert from "node:assert";
import type { AppSettings } from "@/bindings";
import { getPostProcessingAvailability } from "./postProcessingAvailability";

const settings = (overrides: Partial<AppSettings>): AppSettings =>
  overrides as AppSettings;

assert.equal(
  getPostProcessingAvailability(
    settings({
      transcription_provider: "remote_soniox",
      soniox_model: "stt-rt-v5",
      soniox_live_enabled: false,
    }),
  ).available,
  true,
);

assert.equal(
  getPostProcessingAvailability(
    settings({
      transcription_provider: "remote_soniox",
      soniox_model: "stt-async-v4",
      soniox_live_enabled: true,
    }),
  ).available,
  true,
);

assert.equal(
  getPostProcessingAvailability(
    settings({
      transcription_provider: "remote_soniox",
      soniox_model: "stt-rt-v5",
      soniox_live_enabled: true,
      preview_output_only_enabled: false,
      soniox_live_preview_enabled: false,
    }),
  ).available,
  false,
);

assert.equal(
  getPostProcessingAvailability(
    settings({
      transcription_provider: "remote_soniox",
      soniox_model: "stt-rt-v5",
      soniox_live_enabled: true,
      preview_output_only_enabled: true,
    }),
  ).available,
  true,
);

assert.equal(
  getPostProcessingAvailability(
    settings({
      transcription_provider: "local",
      selected_model: "streaming-model",
      native_streaming_live_output_models: ["streaming-model"],
    }),
  ).available,
  false,
);

assert.equal(
  getPostProcessingAvailability(
    settings({
      transcription_provider: "local",
      selected_model: "streaming-model",
      native_streaming_live_output_models: [],
      preview_output_only_enabled: true,
      local_preview_auto_flush_enabled: true,
    }),
  ).available,
  true,
);
