import assert from "node:assert";
import type { AppSettings, ModelInfo, SttModelSelection } from "@/bindings";
import {
  getActiveProfilePostProcessingEnabled,
  getNativeDirectOutputModelId,
  getOutputProviderInfo,
  getPostProcessingAvailability,
  getProfileOutputSummary,
  buffersOutputForKeyActions,
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
  // Unset key fields of a profile mean no key: the Default profile's keys are not inherited.
  assert.equal(getPostProcessingAvailability(bufferedSettings, {
    profileOutputSettings: { auto_submit: null, before_insert_key: null },
  }).available, false);
  assert.equal(getPostProcessingAvailability(bufferedSettings).available, true);

  const activeProfileSettings = settings({
    ...directSettings, active_profile_id: "buffered",
    transcription_profiles: [{ id: "buffered", output_settings: { auto_submit: true } } as any],
  });
  assert.equal(getPostProcessingAvailability(activeProfileSettings).available, true);
  assert.equal(getPostProcessingAvailability(activeProfileSettings, {
    profileOutputSettings: null,
  }).available, false);

  // A recording-start key: live text goes into the chat while recording,
  // so a live provider has no reversible text left to post-process.
  assert.equal(getPostProcessingAvailability(directSettings, {
    profileOutputSettings: { recording_start_key: "enter", auto_submit: true },
  }).available, false);
  // A key before insertion still holds the text until completion.
  assert.equal(getPostProcessingAvailability(directSettings, {
    profileOutputSettings: { recording_start_key: "enter", before_insert_key: "enter" },
  }).available, true);
  assert.equal(getPostProcessingAvailability(directSettings, {
    profileOutputSettings: { recording_start_key: "", auto_submit: true },
  }).available, true);
}

// Moonshine streaming never uses native direct output (the backend checks the model id), so
// a stale list entry must not block post-processing.
assert.equal(
  getPostProcessingAvailability(
    settings({
      transcription_provider: "local",
      selected_model: "handy-computer/moonshine-streaming-tiny-gguf",
      native_streaming_live_output_models: ["handy-computer/moonshine-streaming-tiny-gguf"],
    }),
  ).available,
  true,
);

// Profile card labels: one decision for the output route and LLM availability.
const localModel = (id: string, engine_type: string, supports_streaming: boolean) =>
  ({ id, engine_type, supports_streaming }) as unknown as ModelInfo;
const streamingModel = localModel("streaming-model", "TranscribeCpp", true);
const nativeLocal = (overrides: Partial<AppSettings> = {}) => settings({
  transcription_provider: "local", selected_model: "streaming-model",
  native_streaming_live_output_models: ["streaming-model"], paste_method: "ctrl_v",
  auto_submit: false, ...overrides,
});
const previewProfile = { profilePreviewOutputOnlyEnabled: true };

// Unknown inputs show neither label.
assert.equal(getProfileOutputSummary(null, previewProfile, [streamingModel]), null);
assert.equal(getProfileOutputSummary(nativeLocal(), previewProfile, []), null);

// Native direct output beats Preview and blocks LLM.
assert.deepEqual(getProfileOutputSummary(nativeLocal(), previewProfile, [streamingModel]),
  { route: "insertion", llmAvailable: false });
// A listed model that cannot stream natively (or is gone from the catalog) stays in Preview.
assert.deepEqual(
  getProfileOutputSummary(nativeLocal(), previewProfile, [localModel("streaming-model", "Whisper", false)]),
  { route: "preview", llmAvailable: false });
assert.deepEqual(
  getProfileOutputSummary(nativeLocal(), previewProfile, [localModel("other", "TranscribeCpp", true)]),
  { route: "preview", llmAvailable: false });
// Moonshine streaming is never direct output, even if listed; no model info is needed to know.
const moonshine = "handy-computer/moonshine-streaming-tiny-gguf";
assert.deepEqual(getProfileOutputSummary(nativeLocal({
  selected_model: moonshine, native_streaming_live_output_models: [moonshine],
}), previewProfile, []), { route: "preview", llmAvailable: true });
// Not listed: the route is known without model info.
assert.deepEqual(getProfileOutputSummary(nativeLocal({ native_streaming_live_output_models: [] }), previewProfile, []),
  { route: "preview", llmAvailable: true });
assert.deepEqual(getProfileOutputSummary(nativeLocal(), { profilePreviewOutputOnlyEnabled: false }, []),
  { route: "insertion", llmAvailable: false });
assert.deepEqual(getProfileOutputSummary(nativeLocal({ paste_method: "none" }),
  { profilePreviewOutputOnlyEnabled: false }, [streamingModel]), { route: "none", llmAvailable: false });
// Key actions buffer the output, so LLM stays available.
assert.deepEqual(getProfileOutputSummary(nativeLocal({ auto_submit: true }),
  { profilePreviewOutputOnlyEnabled: false }, [streamingModel]), { route: "insertion", llmAvailable: true });
// Live remote provider: Preview only with the live preview window; otherwise it types directly.
const sonioxLive = (overrides: Partial<AppSettings> = {}) => settings({
  transcription_provider: "remote_soniox", soniox_model: "stt-rt-v5", soniox_live_enabled: true,
  paste_method: "ctrl_v", auto_submit: false, ...overrides,
});
assert.equal(buffersOutputForKeyActions("external_script", "", "", false), true);
assert.equal(getPostProcessingAvailability(sonioxLive({ paste_method: "external_script" })).available, true);
assert.deepEqual(getProfileOutputSummary(sonioxLive({ soniox_live_preview_enabled: true }),
  { profilePreviewOutputOnlyEnabled: false }, []), { route: "preview", llmAvailable: true });
assert.deepEqual(getProfileOutputSummary(sonioxLive(), { profilePreviewOutputOnlyEnabled: false }, []),
  { route: "insertion", llmAvailable: false });

// Without an explicit selection, the active profile's model override decides the route
// (the general LLM page), not the global model.
const whisper = localModel("whisper-small", "Whisper", false);
const profileOverride = (stt_model_selection_override: SttModelSelection, extra = {}) => ({
  active_profile_id: "custom",
  transcription_profiles: [{ id: "custom", stt_model_selection_override, ...extra } as any],
});
const localWhisper: SttModelSelection = { provider: "local", provider_preset: "", model_id: "whisper-small" };
const localStreaming: SttModelSelection = { provider: "local", provider_preset: "", model_id: "streaming-model" };
const sonioxRealtime: SttModelSelection = { provider: "remote_soniox", provider_preset: "", model_id: "stt-rt-v5" };
assert.equal(getPostProcessingAvailability(sonioxLive(profileOverride(localWhisper))).available, true);
assert.equal(getPostProcessingAvailability(sonioxLive({
  transcription_provider: "local", selected_model: "whisper-small", ...profileOverride(sonioxRealtime),
})).available, false);
assert.equal(getOutputProviderInfo(nativeLocal({
  selected_model: "whisper-small", ...profileOverride(localStreaming, { preview_output_only_enabled: true }),
}), {}, [streamingModel, whisper]).heldInPreview, false);

// Enabling a profile's Preview turns off direct output of the model that profile records with.
const modelInfo = (id: string) => [streamingModel, whisper].find((model) => model.id === id);
const ordinaryGlobal = nativeLocal({ selected_model: "whisper-small" });
assert.equal(getNativeDirectOutputModelId(ordinaryGlobal, null, modelInfo), null);
assert.equal(getNativeDirectOutputModelId(ordinaryGlobal, localStreaming, modelInfo), "streaming-model");
assert.equal(getNativeDirectOutputModelId(nativeLocal(), null, modelInfo), "streaming-model");
assert.equal(getNativeDirectOutputModelId(nativeLocal(), sonioxRealtime, modelInfo), null);
assert.equal(getNativeDirectOutputModelId(nativeLocal(), localWhisper, modelInfo), null);
