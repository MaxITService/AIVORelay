import type {
  AppSettings,
  SttModelSelection,
  TranscriptionProfile,
  TranscriptionProvider,
} from "@/bindings";
import {
  REMOTE_STT_PRESETS,
  type RemoteSttPreset,
} from "@/lib/constants/remoteSttProviders";

export const DEFAULT_SONIOX_MODEL = "stt-rt-v6";
export const DEFAULT_DEEPGRAM_MODEL = "nova-3";

/** A returning context is a new generation: old requests must not update its UI. */
export function createCredentialRequestGate() {
  let context: string | undefined;
  let generation = 0;
  return {
    setContext(next: string) {
      if (next !== context) {
        context = next;
        generation += 1;
      }
    },
    capture() {
      const startedIn = generation;
      return () => startedIn === generation;
    },
  };
}

export interface RemoteSttProfileContext {
  isProfileScoped: boolean;
  hasProfileOverride: boolean;
  globalProvider: TranscriptionProvider;
  globalRemotePreset: RemoteSttPreset;
  provider: TranscriptionProvider;
  remotePreset: RemoteSttPreset;
  effectiveRemoteModelId: string;
  effectiveSonioxModel: string;
  effectiveDeepgramModel: string;
  effectiveModelId: string;
  effectiveRemoteBaseUrl: string;
  allowInsecureHttp: boolean;
  canMutateOpenAiCredentials: boolean;
  canReadOpenAiReadiness: boolean;
  canEditCustomEndpoint: boolean;
}

export type OpenAiCredentialAccessMode =
  | "global_rw"
  | "readiness_readonly"
  | "unavailable_custom";

export type RemoteSttMutationTarget =
  | {
      kind: "profile_override";
      selection: SttModelSelection;
    }
  | {
      kind: "global";
      setting:
        | { type: "transcription_provider"; provider: TranscriptionProvider }
        | { type: "remote_stt_model"; modelId: string }
        | {
            type: "remote_stt_preset_and_model";
            preset: RemoteSttPreset;
            modelId: string;
          }
        | { type: "soniox_model"; modelId: string }
        | { type: "deepgram_model"; modelId: string };
    };

export function normalizeRemoteSttPreset(
  preset?: string | null,
): RemoteSttPreset {
  const trimmed = preset?.trim();
  if (!trimmed) {
    return "groq";
  }
  return trimmed in REMOTE_STT_PRESETS
    ? (trimmed as RemoteSttPreset)
    : "custom";
}

export function resolveActiveTranscriptionProfile(
  settings: Pick<
    AppSettings,
    "active_profile_id" | "transcription_profiles"
  > | null | undefined,
): TranscriptionProfile | null {
  const activeProfileId = settings?.active_profile_id ?? "default";
  if (activeProfileId === "default") {
    return null;
  }
  return (
    (settings?.transcription_profiles ?? []).find(
      (profile) => profile.id === activeProfileId,
    ) ?? null
  );
}

export function resolveRemoteSttProfileContext(
  settings:
    | Pick<
        AppSettings,
        | "transcription_provider"
        | "remote_stt"
        | "soniox_model"
        | "deepgram_model"
        | "selected_model"
      >
    | null
    | undefined,
  activeProfile: Pick<TranscriptionProfile, "stt_model_selection_override"> | null | undefined,
): RemoteSttProfileContext {
  const isProfileScoped = activeProfile != null;
  const override = activeProfile?.stt_model_selection_override ?? null;
  const hasProfileOverride = override != null;

  const globalProvider = (settings?.transcription_provider ??
    "local") as TranscriptionProvider;
  const globalRemotePreset = normalizeRemoteSttPreset(
    settings?.remote_stt?.provider_preset,
  );
  const provider = (override?.provider ??
    globalProvider) as TranscriptionProvider;

  const hasRemoteOverridePreset =
    override?.provider === "remote_openai_compatible" &&
    Boolean(override.provider_preset?.trim());
  const remotePreset = hasRemoteOverridePreset
    ? normalizeRemoteSttPreset(override?.provider_preset)
    : globalRemotePreset;

  const globalRemoteModelId = settings?.remote_stt?.model_id ?? "";
  let effectiveRemoteModelId = globalRemoteModelId;
  if (override?.provider === "remote_openai_compatible") {
    const overrideModel = override.model_id?.trim() ?? "";
    if (overrideModel) {
      effectiveRemoteModelId = overrideModel;
    } else if (remotePreset === globalRemotePreset && globalRemoteModelId.trim()) {
      effectiveRemoteModelId = globalRemoteModelId;
    } else {
      effectiveRemoteModelId = REMOTE_STT_PRESETS[remotePreset].defaultModel;
    }
  }

  const effectiveSonioxModel =
    (override?.provider === "remote_soniox" && override.model_id?.trim()
      ? override.model_id.trim()
      : settings?.soniox_model?.trim()) || DEFAULT_SONIOX_MODEL;

  const effectiveDeepgramModel =
    (override?.provider === "remote_deepgram" && override.model_id?.trim()
      ? override.model_id.trim()
      : settings?.deepgram_model?.trim()) || DEFAULT_DEEPGRAM_MODEL;

  const effectiveModelId =
    provider === "remote_soniox"
      ? effectiveSonioxModel
      : provider === "remote_deepgram"
        ? effectiveDeepgramModel
        : provider === "remote_openai_compatible"
          ? effectiveRemoteModelId
          : (override?.provider === "local" && override.model_id?.trim()
              ? override.model_id.trim()
              : settings?.selected_model) ?? "";

  const canEditCustomEndpoint =
    remotePreset === "custom" && globalRemotePreset === "custom";
  const effectiveRemoteBaseUrl =
    remotePreset === "custom"
      ? canEditCustomEndpoint
        ? (settings?.remote_stt?.base_url ?? "")
        : ""
      : REMOTE_STT_PRESETS[remotePreset].baseUrl;

  const allowInsecureHttp = canEditCustomEndpoint
    ? Boolean(settings?.remote_stt?.allow_insecure_http ?? false)
    : false;

  const canMutateOpenAiCredentials =
    !isProfileScoped || remotePreset === globalRemotePreset;
  const canReadOpenAiReadiness =
    canMutateOpenAiCredentials || remotePreset !== "custom";

  return {
    isProfileScoped,
    hasProfileOverride,
    globalProvider,
    globalRemotePreset,
    provider,
    remotePreset,
    effectiveRemoteModelId,
    effectiveSonioxModel,
    effectiveDeepgramModel,
    effectiveModelId,
    effectiveRemoteBaseUrl,
    allowInsecureHttp,
    canMutateOpenAiCredentials,
    canReadOpenAiReadiness,
    canEditCustomEndpoint,
  };
}

export function getOpenAiCredentialAccessMode(
  context: Pick<
    RemoteSttProfileContext,
    "canMutateOpenAiCredentials" | "canReadOpenAiReadiness"
  >,
): OpenAiCredentialAccessMode {
  if (context.canMutateOpenAiCredentials) {
    return "global_rw";
  }
  if (context.canReadOpenAiReadiness) {
    return "readiness_readonly";
  }
  return "unavailable_custom";
}

export function buildOpenAiReadinessSelection(
  context: Pick<
    RemoteSttProfileContext,
    "remotePreset" | "effectiveRemoteModelId"
  >,
): SttModelSelection {
  return {
    provider: "remote_openai_compatible",
    provider_preset: context.remotePreset,
    model_id:
      context.effectiveRemoteModelId.trim() ||
      REMOTE_STT_PRESETS[context.remotePreset].defaultModel,
  };
}

export function planProviderChange(
  context: RemoteSttProfileContext,
  nextProvider: TranscriptionProvider,
  settings?: Pick<AppSettings, "selected_model"> | null,
): RemoteSttMutationTarget {
  if (!context.isProfileScoped) {
    return {
      kind: "global",
      setting: { type: "transcription_provider", provider: nextProvider },
    };
  }

  switch (nextProvider) {
    case "local":
      return {
        kind: "profile_override",
        selection: {
          provider: "local",
          model_id:
            (context.provider === "local"
              ? context.effectiveModelId.trim()
              : "") ||
            settings?.selected_model?.trim() ||
            "",
          provider_preset: "",
        },
      };
    case "remote_soniox":
      return {
        kind: "profile_override",
        selection: {
          provider: "remote_soniox",
          model_id: context.effectiveSonioxModel || DEFAULT_SONIOX_MODEL,
          provider_preset: "",
        },
      };
    case "remote_deepgram":
      return {
        kind: "profile_override",
        selection: {
          provider: "remote_deepgram",
          model_id: context.effectiveDeepgramModel || DEFAULT_DEEPGRAM_MODEL,
          provider_preset: "",
        },
      };
    case "remote_openai_compatible":
      return {
        kind: "profile_override",
        selection: {
          provider: "remote_openai_compatible",
          provider_preset: context.remotePreset,
          model_id:
            context.effectiveRemoteModelId.trim() ||
            REMOTE_STT_PRESETS[context.remotePreset].defaultModel,
        },
      };
  }
}

export function planRemoteInterfaceChange(
  context: Pick<RemoteSttProfileContext, "isProfileScoped">,
  nextPreset: RemoteSttPreset,
  nextModelId: string,
): RemoteSttMutationTarget {
  const normalizedModel =
    nextModelId.trim() || REMOTE_STT_PRESETS[nextPreset].defaultModel;
  if (context.isProfileScoped) {
    return {
      kind: "profile_override",
      selection: {
        provider: "remote_openai_compatible",
        provider_preset: nextPreset,
        model_id: normalizedModel,
      },
    };
  }
  return {
    kind: "global",
    setting: {
      type: "remote_stt_preset_and_model",
      preset: nextPreset,
      modelId: normalizedModel,
    },
  };
}

export function planRemoteModelIdChange(
  context: Pick<RemoteSttProfileContext, "isProfileScoped" | "remotePreset">,
  modelId: string,
): RemoteSttMutationTarget {
  const trimmed = modelId.trim();
  if (context.isProfileScoped) {
    return {
      kind: "profile_override",
      selection: {
        provider: "remote_openai_compatible",
        provider_preset: context.remotePreset,
        model_id: trimmed,
      },
    };
  }
  return {
    kind: "global",
    setting: {
      type: "remote_stt_model",
      modelId: trimmed,
    },
  };
}

export function planGeminiRouteChange(
  context: Pick<RemoteSttProfileContext, "isProfileScoped">,
  route: "vercel" | "google",
  isGeminiLive: boolean,
): RemoteSttMutationTarget {
  const nextModel = isGeminiLive
    ? route === "vercel"
      ? "google/gemini-3.5-transcribe-live"
      : "gemini-3.5-transcribe-live"
    : REMOTE_STT_PRESETS[route].defaultModel;

  if (context.isProfileScoped) {
    return {
      kind: "profile_override",
      selection: {
        provider: "remote_openai_compatible",
        provider_preset: route,
        model_id: nextModel,
      },
    };
  }
  return {
    kind: "global",
    setting: {
      type: "remote_stt_preset_and_model",
      preset: route,
      modelId: nextModel,
    },
  };
}

export function planSonioxModelChange(
  context: Pick<RemoteSttProfileContext, "isProfileScoped">,
  modelId: string,
): RemoteSttMutationTarget {
  const normalized = modelId.trim() || DEFAULT_SONIOX_MODEL;
  if (context.isProfileScoped) {
    return {
      kind: "profile_override",
      selection: {
        provider: "remote_soniox",
        provider_preset: "",
        model_id: normalized,
      },
    };
  }
  return {
    kind: "global",
    setting: {
      type: "soniox_model",
      modelId: normalized,
    },
  };
}

export function planDeepgramModelChange(
  context: Pick<RemoteSttProfileContext, "isProfileScoped">,
  modelId: string,
): RemoteSttMutationTarget {
  const normalized = modelId.trim() || DEFAULT_DEEPGRAM_MODEL;
  if (context.isProfileScoped) {
    return {
      kind: "profile_override",
      selection: {
        provider: "remote_deepgram",
        provider_preset: "",
        model_id: normalized,
      },
    };
  }
  return {
    kind: "global",
    setting: {
      type: "deepgram_model",
      modelId: normalized,
    },
  };
}
