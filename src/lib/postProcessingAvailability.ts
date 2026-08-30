import type { AppSettings, SttModelSelection } from "@/bindings";

export type PostProcessingUnavailableReason = "direct_realtime_output";

export interface PostProcessingAvailability {
  available: boolean;
  reason: PostProcessingUnavailableReason | null;
}

interface AvailabilityOptions {
  profilePreviewOutputOnlyEnabled?: boolean;
  sttSelection?: SttModelSelection;
}

const REALTIME_OPENAI_MODELS = new Set([
  "gpt-realtime-whisper",
  "gpt-live-transcribe",
]);

/**
 * Returns the saved LLM post-processing choice for the profile used by the
 * main Transcribe action. The default profile owns the global setting; custom
 * profiles own their individual setting.
 */
export const getActiveProfilePostProcessingEnabled = (
  settings: AppSettings | null | undefined,
): boolean => {
  if (!settings) return false;

  const activeProfileId = String(settings.active_profile_id || "default");
  if (activeProfileId !== "default") {
    const activeProfile = settings.transcription_profiles?.find(
      (profile) => profile.id === activeProfileId,
    );
    if (activeProfile) {
      return Boolean(activeProfile.llm_post_process_enabled);
    }
  }

  return Boolean(settings.post_process_enabled);
};

/**
 * Mirrors the backend's output-route capability rule.
 *
 * Post-processing is available while the complete transcript is still held in
 * Preview. It is unavailable after realtime chunks start entering the target
 * application because a later LLM result cannot safely replace that text.
 * Local preview auto-flush is deliberately irrelevant here: it writes to the
 * reversible Preview workflow, not directly to the target application.
 */
export const getPostProcessingAvailability = (
  settings: AppSettings | null | undefined,
  options: AvailabilityOptions = {},
): PostProcessingAvailability => {
  if (!settings) return { available: true, reason: null };

  const provider = String(
    options.sttSelection?.provider ?? settings.transcription_provider ?? "local",
  );
  const activeProfileId = String(settings.active_profile_id || "default");
  const activeProfile =
    activeProfileId === "default"
      ? undefined
      : settings.transcription_profiles?.find(
          (profile) => profile.id === activeProfileId,
        );
  const profilePreviewEnabled =
    options.profilePreviewOutputOnlyEnabled ??
    activeProfile?.preview_output_only_enabled ??
    Boolean(settings.preview_output_only_enabled);
  const livePreviewEnabled = Boolean(settings.soniox_live_preview_enabled);
  const outputIsHeldInPreview = profilePreviewEnabled || livePreviewEnabled;

  let insertsRealtimeTextDirectly = false;

  if (provider === "local") {
    const selectedModel = String(
      options.sttSelection?.model_id ?? settings.selected_model ?? "",
    );
    const directOutputModels =
      settings.native_streaming_live_output_models ?? [];
    // Native direct output takes precedence over Preview in the backend.
    insertsRealtimeTextDirectly = directOutputModels.includes(selectedModel);
  } else if (provider === "remote_soniox") {
    const model = String(
      options.sttSelection?.model_id ?? settings.soniox_model ?? "",
    ).trim();
    const realtimeModel = model.length === 0 || model.startsWith("stt-rt");
    insertsRealtimeTextDirectly =
      Boolean(settings.soniox_live_enabled) &&
      realtimeModel &&
      !outputIsHeldInPreview;
  } else if (provider === "remote_deepgram") {
    const model = String(
      options.sttSelection?.model_id ?? settings.deepgram_model ?? "",
    ).trim();
    insertsRealtimeTextDirectly =
      Boolean(settings.deepgram_live_enabled) &&
      model.length > 0 &&
      !outputIsHeldInPreview;
  } else if (provider === "remote_openai_compatible") {
    const preset = String(
      options.sttSelection?.provider_preset ??
        settings.remote_stt?.provider_preset ??
        "",
    );
    const model = String(
      options.sttSelection?.model_id ?? settings.remote_stt?.model_id ?? "",
    ).toLowerCase();
    insertsRealtimeTextDirectly =
      preset === "openai" &&
      REALTIME_OPENAI_MODELS.has(model) &&
      !Boolean(settings.openai_realtime_whisper_flatten_enabled) &&
      !outputIsHeldInPreview;
  }

  return insertsRealtimeTextDirectly
    ? { available: false, reason: "direct_realtime_output" }
    : { available: true, reason: null };
};
