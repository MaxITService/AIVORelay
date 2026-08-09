import assert from "node:assert";
import type { AppSettings } from "@/bindings";
import {
  getActiveProfilePostProcessingEnabled,
  getPostProcessingAvailability,
} from "./postProcessingAvailability";

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
  getActiveProfilePostProcessingEnabled(
    settings({
      active_profile_id: "default",
      post_process_enabled: true,
      transcription_profiles: [],
    }),
  ),
  true,
);

assert.equal(
  getActiveProfilePostProcessingEnabled(
    settings({
      active_profile_id: "profile-1",
      post_process_enabled: false,
      transcription_profiles: [
        {
          id: "profile-1",
          llm_post_process_enabled: true,
        } as any,
      ],
    }),
  ),
  true,
);

assert.equal(
  getActiveProfilePostProcessingEnabled(
    settings({
      active_profile_id: "missing-profile",
      post_process_enabled: true,
      transcription_profiles: [],
    }),
  ),
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
