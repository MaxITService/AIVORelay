import assert from "node:assert";
import type { AppSettings } from "@/bindings";
import {
  getActiveProfilePostProcessingEnabled,
  getPostProcessingAvailability,
} from "./postProcessingAvailability";
import type { ProfileOutputSettings } from "./profileOutputSettings";

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

for (const [providerPreset, modelId] of [
  ["vercel", "google/gemini-3.5-transcribe-live"],
  ["google", "gemini-3.5-transcribe-live"],
] as const) {
  assert.equal(
    getPostProcessingAvailability(
      settings({
        transcription_provider: "remote_openai_compatible",
        remote_stt: {
          provider_preset: providerPreset,
          model_id: modelId,
        } as any,
        preview_output_only_enabled: false,
        soniox_live_preview_enabled: false,
      }),
    ).available,
    false,
  );

  assert.equal(
    getPostProcessingAvailability(
      settings({
        transcription_provider: "remote_openai_compatible",
        remote_stt: {
          provider_preset: providerPreset,
          model_id: modelId,
        } as any,
        preview_output_only_enabled: true,
      }),
    ).available,
    true,
  );
}

assert.equal(
  getPostProcessingAvailability(
    settings({
      transcription_provider: "remote_openai_compatible",
      remote_stt: {
        provider_preset: "vercel",
        model_id: "google/gemini-3.5-transcribe-live-custom",
      } as any,
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

const liveProviders: Partial<AppSettings>[] = [
  { transcription_provider: "local", selected_model: "streaming-model",
    native_streaming_live_output_models: ["streaming-model"] },
  { transcription_provider: "remote_soniox", soniox_model: "stt-rt-v5",
    soniox_live_enabled: true },
  { transcription_provider: "remote_deepgram", deepgram_model: "nova-3",
    deepgram_live_enabled: true },
  ...(["openai", "google", "vercel"] as const).map((preset) => ({
    transcription_provider: "remote_openai_compatible" as const,
    remote_stt: { provider_preset: preset, model_id: preset === "openai"
      ? "gpt-live-transcribe" : preset === "google"
        ? "gemini-3.5-transcribe-live" : "google/gemini-3.5-transcribe-live" } as any,
  })),
];

for (const provider of liveProviders) {
  const directSettings = settings({
    ...provider, paste_method: "ctrl_v", auto_submit: false,
    preview_output_only_enabled: false, soniox_live_preview_enabled: false,
  });
  assert.equal(getPostProcessingAvailability(directSettings).available, false);
  for (const output of [
    { auto_submit: true }, { before_insert_key: "enter" },
  ] satisfies ProfileOutputSettings[]) {
    assert.equal(getPostProcessingAvailability(directSettings, {
      profileOutputSettings: output,
    }).available, true);
    assert.equal(getPostProcessingAvailability(directSettings, {
      profileOutputSettings: { ...output, paste_method: "none" },
    }).available, false);
  }

  const bufferedSettings = settings({
    ...directSettings, auto_submit: true,
    transcription_output_defaults: {
      paste_method: null, clipboard_handling: null, clipboard_history_allowed: null,
      convert_lf_to_crlf: null, paste_delay_ms: null, auto_submit: null, auto_submit_key: null,
      before_insert_key: "enter", before_insert_delay_ms: null, auto_submit_delay_ms: null,
    },
  });
  assert.equal(getPostProcessingAvailability(bufferedSettings, {
    profileOutputSettings: { auto_submit: false, before_insert_key: "" },
  }).available, false);
  assert.equal(getPostProcessingAvailability(bufferedSettings, {
    profileOutputSettings: { auto_submit: null, before_insert_key: null },
  }).available, true);

  const activeProfileSettings = settings({
    ...directSettings, active_profile_id: "buffered",
    transcription_profiles: [{ id: "buffered", output_settings: { auto_submit: true } } as any],
  });
  assert.equal(getPostProcessingAvailability(activeProfileSettings).available, true);
  assert.equal(getPostProcessingAvailability(activeProfileSettings, {
    profileOutputSettings: null,
  }).available, false);
}
