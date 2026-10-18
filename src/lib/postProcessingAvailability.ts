import type { AppSettings, ModelInfo, SttModelSelection } from "@/bindings";
import { STT_MODEL_IDS } from "@/lib/sttModelSelection";
import type { ProfileOutputSettings } from "@/lib/profileOutputSettings";
import { isMoonshineStreamingModel } from "@/components/model-selector/nativeStreamingModel";

export type PostProcessingUnavailableReason = "direct_realtime_output";

export interface PostProcessingAvailability {
  available: boolean;
  reason: PostProcessingUnavailableReason | null;
}

interface AvailabilityOptions {
  profilePreviewOutputOnlyEnabled?: boolean;
  // Undefined follows the active profile's model override, then the global model.
  sttSelection?: SttModelSelection;
  // Null selects the Default profile; undefined follows the active profile.
  profileOutputSettings?: ProfileOutputSettings | null;
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

const getOutputRouteContext = (
  settings: AppSettings | null | undefined,
  options: AvailabilityOptions = {},
) => {
  const activeProfileId = String(settings?.active_profile_id || "default");
  const activeProfile =
    activeProfileId === "default"
      ? undefined
      : settings?.transcription_profiles?.find(
          (profile) => profile.id === activeProfileId,
        );
  const sttSelection = options.sttSelection ?? (options.profileOutputSettings === undefined
    ? activeProfile?.stt_model_selection_override ?? undefined : undefined);
  const provider = String(
    sttSelection?.provider ?? settings?.transcription_provider ?? "local",
  );
  const profilePreviewEnabled =
    options.profilePreviewOutputOnlyEnabled ??
    activeProfile?.preview_output_only_enabled ??
    Boolean(settings?.preview_output_only_enabled);
  const livePreviewEnabled = Boolean(settings?.soniox_live_preview_enabled);
  const outputIsHeldInPreview = profilePreviewEnabled || livePreviewEnabled;
  const output = options.profileOutputSettings === undefined
    ? activeProfile?.output_settings : options.profileOutputSettings;
  // A profile never follows the Default profile: its unset fields use the program defaults
  // (a paste method other than "none", no keys). Only "none" matters for the paste method here.
  const isDefaultProfile = output == null;
  const pasteMethod = output?.paste_method ?? (isDefaultProfile ? settings?.paste_method ?? "ctrl_v" : "ctrl_v");
  const autoSubmit = output?.auto_submit ?? (isDefaultProfile && (settings?.auto_submit ?? false));
  const keys = output ?? settings?.transcription_output_defaults;
  const openingKey = keys?.before_insert_key ?? "";
  const startKey = keys?.recording_start_key ?? "";

  let nativeLiveOutput = false;
  let remoteLiveOutput = false;
  let localModelId = "";

  if (provider === "local") {
    const selectedModel = String(
      sttSelection?.model_id?.trim() ?? settings?.selected_model ?? "",
    );
    localModelId = selectedModel;
    const directOutputModels =
      settings?.native_streaming_live_output_models ?? [];
    // Native direct output takes precedence over Preview in the backend. Mirrors
    // native_streaming_live_output_enabled: Moonshine streaming never uses it (the
    // backend checks the model id only).
    nativeLiveOutput = directOutputModels.includes(selectedModel) &&
      !isMoonshineStreamingModel({ id: selectedModel, name: "", filename: "" });
  } else if (provider === "remote_soniox") {
    const model = String(
      sttSelection?.model_id ?? settings?.soniox_model ?? "",
    ).trim();
    const realtimeModel = model.length === 0 || model.startsWith("stt-rt");
    remoteLiveOutput = Boolean(settings?.soniox_live_enabled) && realtimeModel;
  } else if (provider === "remote_deepgram") {
    const model = String(
      sttSelection?.model_id ?? settings?.deepgram_model ?? "",
    ).trim();
    remoteLiveOutput = Boolean(settings?.deepgram_live_enabled) && model.length > 0;
  } else if (provider === "remote_openai_compatible") {
    const preset = String(
      sttSelection?.provider_preset ??
        settings?.remote_stt?.provider_preset ??
        "",
    );
    const model = String(
      sttSelection?.model_id ?? settings?.remote_stt?.model_id ?? "",
    ).toLowerCase();
    const isOpenAiRealtime =
      preset === "openai" &&
      REALTIME_OPENAI_MODELS.has(model) &&
      !Boolean(settings?.openai_realtime_whisper_flatten_enabled);
    const isGeminiRealtime =
      (preset === "vercel" && model === STT_MODEL_IDS.vercelGeminiLive) ||
      (preset === "google" && model === STT_MODEL_IDS.googleGeminiLive);
    remoteLiveOutput = isOpenAiRealtime || isGeminiRealtime;
  }

  return {
    profilePreviewEnabled, livePreviewEnabled, outputIsHeldInPreview,
    pasteMethod, autoSubmit, openingKey, startKey, nativeLiveOutput, remoteLiveOutput, localModelId,
  };
};

/**
 * Mirrors ResolvedOutputSettings::buffers_external_output. A completion key holds text;
 * a separate start key permits live text when no completion key is configured.
 */
export const buffersOutputForKeyActions = (
  pasteMethod: string, openingKey: string, startKey: string, autoSubmit: boolean,
): boolean => pasteMethod !== "none" && (pasteMethod === "external_script" || openingKey !== "" || (autoSubmit && startKey === ""));

/**
 * Mirrors should_route_output_to_preview_for_captured_profile minus native direct output.
 * Returns null while that cannot be decided: the native direct-output check needs the local
 * model's engine and streaming support, which are unavailable until local models have loaded.
 */
const resolveHeldInPreview = (
  route: ReturnType<typeof getOutputRouteContext>,
  localModels: readonly ModelInfo[],
): boolean | null => {
  if (!route.profilePreviewEnabled && !(route.livePreviewEnabled && route.remoteLiveOutput)) {
    return false;
  }
  if (!route.nativeLiveOutput) return true;
  if (localModels.length === 0) return null;
  const localModel = localModels.find(model => model.id === route.localModelId);
  const nativeDirectOutput =
    localModel?.engine_type === "TranscribeCpp" && localModel.supports_streaming;
  return !nativeDirectOutput;
};

// Callers that cannot show "unknown" keep the earlier behaviour: Preview while loading.
const isHeldInPreview = (
  route: ReturnType<typeof getOutputRouteContext>,
  localModels: readonly ModelInfo[],
): boolean => resolveHeldInPreview(route, localModels) ?? true;

/**
 * The local model whose native direct output would win over a profile's Preview, or null.
 * The profile's model override applies; without one the global model is used.
 */
export const getNativeDirectOutputModelId = (
  settings: AppSettings | null | undefined,
  sttOverride: SttModelSelection | null | undefined,
  getModelInfo: (modelId: string) => ModelInfo | null | undefined,
): string | null => {
  const provider = String(sttOverride?.provider ?? settings?.transcription_provider ?? "local");
  const modelId = String(sttOverride ? sttOverride.model_id?.trim() ?? "" : settings?.selected_model ?? "");
  if (provider !== "local" || !modelId) return null;
  const model = getModelInfo(modelId);
  return model?.engine_type === "TranscribeCpp" && model.supports_streaming === true &&
    (settings?.native_streaming_live_output_models ?? []).includes(modelId) ? modelId : null;
};

/**
 * Where a profile's text goes and whether its speech provider streams text live into the
 * target application while recording (the same capability rule as post-processing).
 */
export const getOutputProviderInfo = (
  settings: AppSettings | null | undefined,
  options: AvailabilityOptions = {},
  localModels: readonly ModelInfo[] = [],
): { heldInPreview: boolean; streamsLiveText: boolean } => {
  const route = getOutputRouteContext(settings, options);
  return {
    heldInPreview: isHeldInPreview(route, localModels),
    streamsLiveText: route.nativeLiveOutput || (route.remoteLiveOutput && !route.outputIsHeldInPreview),
  };
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
  const route = getOutputRouteContext(settings, options);
  if (buffersOutputForKeyActions(route.pasteMethod, route.openingKey, route.startKey, route.autoSubmit)) {
    return { available: true, reason: null };
  }
  const insertsRealtimeTextDirectly = route.nativeLiveOutput ||
    (route.remoteLiveOutput && !route.outputIsHeldInPreview);
  return insertsRealtimeTextDirectly
    ? { available: false, reason: "direct_realtime_output" }
    : { available: true, reason: null };
};

export interface ProfileOutputSummary {
  /** Where the dictation text goes, mirroring the backend recording path. */
  route: "preview" | "insertion" | "none";
  /** Whether LLM post-processing can run for this output route. */
  llmAvailable: boolean;
}

/**
 * One decision for the profile card's output labels. Returns null when the inputs are not
 * known yet (settings or local models still loading), so the card shows no label instead of
 * a guess that flips once loading ends.
 */
export const getProfileOutputSummary = (
  settings: AppSettings | null | undefined,
  options: AvailabilityOptions = {},
  localModels: readonly ModelInfo[] = [],
): ProfileOutputSummary | null => {
  if (!settings) return null;
  const route = getOutputRouteContext(settings, options);
  const held = resolveHeldInPreview(route, localModels);
  if (held === null) return null;
  return {
    route: held ? "preview" : route.pasteMethod === "none" ? "none" : "insertion",
    llmAvailable: getPostProcessingAvailability(settings, options).available,
  };
};
