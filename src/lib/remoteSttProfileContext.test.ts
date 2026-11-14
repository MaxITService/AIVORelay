import { describe, expect, test } from "bun:test";
import type { AppSettings, TranscriptionProfile } from "@/bindings";
import { REMOTE_STT_PRESETS } from "@/lib/constants/remoteSttProviders";
import {
  buildOpenAiReadinessSelection,
  createCredentialRequestGate,
  DEFAULT_DEEPGRAM_MODEL,
  DEFAULT_SONIOX_MODEL,
  getOpenAiCredentialAccessMode,
  normalizeRemoteSttPreset,
  planDeepgramModelChange,
  planGeminiRouteChange,
  planProviderChange,
  planRemoteInterfaceChange,
  planRemoteModelIdChange,
  planSonioxModelChange,
  resolveActiveTranscriptionProfile,
  resolveRemoteSttProfileContext,
} from "./remoteSttProfileContext";

type TestSettings = Pick<
  AppSettings,
  | "active_profile_id"
  | "transcription_profiles"
  | "transcription_provider"
  | "remote_stt"
  | "soniox_model"
  | "deepgram_model"
  | "selected_model"
>;

function makeSettings(overrides: Partial<TestSettings> = {}): TestSettings {
  return {
    active_profile_id: "default",
    transcription_profiles: [],
    transcription_provider: "local",
    remote_stt: {
      provider_preset: "groq",
      base_url: REMOTE_STT_PRESETS.groq.baseUrl,
      model_id: REMOTE_STT_PRESETS.groq.defaultModel,
      allow_insecure_http: false,
      debug_capture: false,
      debug_mode: "normal",
    },
    soniox_model: DEFAULT_SONIOX_MODEL,
    deepgram_model: DEFAULT_DEEPGRAM_MODEL,
    selected_model: "whisper-medium",
    ...overrides,
  };
}

function makeProfile(
  overrides: Partial<TranscriptionProfile> = {},
): TranscriptionProfile {
  return {
    id: "profile-work",
    name: "Work",
    language: "en",
    translate_to_english: false,
    include_in_cycle: true,
    is_default: false,
    system_prompt: "",
    stt_prompt_override_enabled: false,
    post_process_enabled_override: null,
    post_process_provider_id_override: null,
    post_process_model_override: null,
    post_process_prompt_override: null,
    stt_model_selection_override: null,
    ...overrides,
  };
}

describe("remoteSttProfileContext", () => {
  test("resolves profile Reson8 override when global provider is local", () => {
    const profile = makeProfile({
      id: "profile-reson8",
      name: "Reson8",
      stt_model_selection_override: {
        provider: "remote_openai_compatible",
        provider_preset: "reson8",
        model_id: "rumble-2",
      },
    });
    const settings = makeSettings({
      active_profile_id: profile.id,
      transcription_profiles: [profile],
      transcription_provider: "local",
      remote_stt: {
        provider_preset: "groq",
        base_url: REMOTE_STT_PRESETS.groq.baseUrl,
        model_id: "whisper-large-v3-turbo",
        allow_insecure_http: false,
        debug_capture: false,
        debug_mode: "normal",
      },
    });

    const activeProfile = resolveActiveTranscriptionProfile(settings);
    const context = resolveRemoteSttProfileContext(settings, activeProfile);

    expect(context.isProfileScoped).toBe(true);
    expect(context.hasProfileOverride).toBe(true);
    expect(context.globalProvider).toBe("local");
    expect(context.globalRemotePreset).toBe("groq");
    expect(context.provider).toBe("remote_openai_compatible");
    expect(context.remotePreset).toBe("reson8");
    expect(context.effectiveRemoteModelId).toBe("rumble-2");
    expect(context.effectiveModelId).toBe("rumble-2");
    expect(context.effectiveRemoteBaseUrl).toBe(REMOTE_STT_PRESETS.reson8.baseUrl);
    expect(context.canMutateOpenAiCredentials).toBe(false);
    expect(context.canReadOpenAiReadiness).toBe(true);
    expect(getOpenAiCredentialAccessMode(context)).toBe("readiness_readonly");
    expect(buildOpenAiReadinessSelection(context)).toEqual({
      provider: "remote_openai_compatible",
      provider_preset: "reson8",
      model_id: "rumble-2",
    });
  });

  test("resolves profile Reson8 override without leaking global Groq settings", () => {
    const profile = makeProfile({
      id: "profile-reson8",
      stt_model_selection_override: {
        provider: "remote_openai_compatible",
        provider_preset: "reson8",
        model_id: "",
      },
    });
    const settings = makeSettings({
      active_profile_id: profile.id,
      transcription_profiles: [profile],
      transcription_provider: "remote_openai_compatible",
      remote_stt: {
        provider_preset: "groq",
        base_url: REMOTE_STT_PRESETS.groq.baseUrl,
        model_id: "whisper-large-v3",
        allow_insecure_http: false,
        debug_capture: false,
        debug_mode: "normal",
      },
    });

    const context = resolveRemoteSttProfileContext(
      settings,
      resolveActiveTranscriptionProfile(settings),
    );

    expect(context.provider).toBe("remote_openai_compatible");
    expect(context.remotePreset).toBe("reson8");
    expect(context.effectiveRemoteModelId).toBe(
      REMOTE_STT_PRESETS.reson8.defaultModel,
    );
    expect(context.effectiveRemoteBaseUrl).toBe(REMOTE_STT_PRESETS.reson8.baseUrl);
    expect(context.canMutateOpenAiCredentials).toBe(false);
    expect(context.canReadOpenAiReadiness).toBe(true);
    expect(getOpenAiCredentialAccessMode(context)).toBe("readiness_readonly");
  });

  test("inherits global selection for non-default profile with null override and still routes edits to profile override", () => {
    const profile = makeProfile({
      id: "profile-inherited",
      stt_model_selection_override: null,
    });
    const settings = makeSettings({
      active_profile_id: profile.id,
      transcription_profiles: [profile],
      transcription_provider: "remote_openai_compatible",
      remote_stt: {
        provider_preset: "groq",
        base_url: REMOTE_STT_PRESETS.groq.baseUrl,
        model_id: "whisper-large-v3",
        allow_insecure_http: false,
        debug_capture: false,
        debug_mode: "normal",
      },
      soniox_model: "stt-async-v6",
      deepgram_model: "nova-3-general",
    });

    const context = resolveRemoteSttProfileContext(settings, profile);

    expect(context.isProfileScoped).toBe(true);
    expect(context.hasProfileOverride).toBe(false);
    expect(context.provider).toBe("remote_openai_compatible");
    expect(context.remotePreset).toBe("groq");
    expect(context.effectiveRemoteModelId).toBe("whisper-large-v3");
    expect(context.effectiveSonioxModel).toBe("stt-async-v6");
    expect(context.effectiveDeepgramModel).toBe("nova-3-general");
    expect(context.canMutateOpenAiCredentials).toBe(true);
    expect(getOpenAiCredentialAccessMode(context)).toBe("global_rw");

    expect(planProviderChange(context, "remote_soniox", settings)).toEqual({
      kind: "profile_override",
      selection: {
        provider: "remote_soniox",
        model_id: "stt-async-v6",
        provider_preset: "",
      },
    });
    expect(
      planRemoteInterfaceChange(context, "reson8", "rumble-2"),
    ).toEqual({
      kind: "profile_override",
      selection: {
        provider: "remote_openai_compatible",
        provider_preset: "reson8",
        model_id: "rumble-2",
      },
    });
    expect(planRemoteModelIdChange(context, "whisper-large-v3-turbo")).toEqual({
      kind: "profile_override",
      selection: {
        provider: "remote_openai_compatible",
        provider_preset: "groq",
        model_id: "whisper-large-v3-turbo",
      },
    });
  });

  test("resolves and routes Soniox and Deepgram profile model overrides independently of global models", () => {
    const sonioxProfile = makeProfile({
      id: "profile-soniox",
      stt_model_selection_override: {
        provider: "remote_soniox",
        provider_preset: "",
        model_id: "stt-async-v6",
      },
    });
    const deepgramProfile = makeProfile({
      id: "profile-deepgram",
      stt_model_selection_override: {
        provider: "remote_deepgram",
        provider_preset: "",
        model_id: "nova-3-medical",
      },
    });
    const settings = makeSettings({
      soniox_model: "stt-rt-v6",
      deepgram_model: "nova-3",
      transcription_provider: "local",
    });

    const sonioxContext = resolveRemoteSttProfileContext(
      settings,
      sonioxProfile,
    );
    expect(sonioxContext.provider).toBe("remote_soniox");
    expect(sonioxContext.effectiveSonioxModel).toBe("stt-async-v6");
    expect(sonioxContext.effectiveModelId).toBe("stt-async-v6");
    expect(planSonioxModelChange(sonioxContext, "stt-rt-v6")).toEqual({
      kind: "profile_override",
      selection: {
        provider: "remote_soniox",
        provider_preset: "",
        model_id: "stt-rt-v6",
      },
    });

    const deepgramContext = resolveRemoteSttProfileContext(
      settings,
      deepgramProfile,
    );
    expect(deepgramContext.provider).toBe("remote_deepgram");
    expect(deepgramContext.effectiveDeepgramModel).toBe("nova-3-medical");
    expect(deepgramContext.effectiveModelId).toBe("nova-3-medical");
    expect(planDeepgramModelChange(deepgramContext, "nova-3-general")).toEqual({
      kind: "profile_override",
      selection: {
        provider: "remote_deepgram",
        provider_preset: "",
        model_id: "nova-3-general",
      },
    });
  });

  test("allows same-preset OpenAI credential mutations even when global transcription_provider is local, and denies mismatched presets", () => {
    const samePresetProfile = makeProfile({
      id: "profile-groq",
      stt_model_selection_override: {
        provider: "remote_openai_compatible",
        provider_preset: "groq",
        model_id: "whisper-large-v3",
      },
    });
    const mismatchedProfile = makeProfile({
      id: "profile-openai",
      stt_model_selection_override: {
        provider: "remote_openai_compatible",
        provider_preset: "openai",
        model_id: "gpt-transcribe",
      },
    });
    const customMismatchProfile = makeProfile({
      id: "profile-custom",
      stt_model_selection_override: {
        provider: "remote_openai_compatible",
        provider_preset: "custom",
        model_id: "my-model",
      },
    });
    const settings = makeSettings({
      transcription_provider: "local",
      remote_stt: {
        provider_preset: "groq",
        base_url: REMOTE_STT_PRESETS.groq.baseUrl,
        model_id: "whisper-large-v3-turbo",
        allow_insecure_http: false,
        debug_capture: false,
        debug_mode: "normal",
      },
    });

    const sameContext = resolveRemoteSttProfileContext(
      settings,
      samePresetProfile,
    );
    expect(sameContext.canMutateOpenAiCredentials).toBe(true);
    expect(sameContext.canReadOpenAiReadiness).toBe(true);
    expect(getOpenAiCredentialAccessMode(sameContext)).toBe("global_rw");

    const mismatchContext = resolveRemoteSttProfileContext(
      settings,
      mismatchedProfile,
    );
    expect(mismatchContext.canMutateOpenAiCredentials).toBe(false);
    expect(mismatchContext.canReadOpenAiReadiness).toBe(true);
    expect(getOpenAiCredentialAccessMode(mismatchContext)).toBe(
      "readiness_readonly",
    );

    const customMismatchContext = resolveRemoteSttProfileContext(
      settings,
      customMismatchProfile,
    );
    expect(customMismatchContext.canMutateOpenAiCredentials).toBe(false);
    expect(customMismatchContext.canReadOpenAiReadiness).toBe(false);
    expect(customMismatchContext.canEditCustomEndpoint).toBe(false);
    expect(customMismatchContext.effectiveRemoteBaseUrl).toBe("");
    expect(customMismatchContext.allowInsecureHttp).toBe(false);
    expect(getOpenAiCredentialAccessMode(customMismatchContext)).toBe(
      "unavailable_custom",
    );
  });

  test("allows custom endpoint editing only when both effective and global presets are custom", () => {
    const customProfile = makeProfile({
      id: "profile-custom",
      stt_model_selection_override: {
        provider: "remote_openai_compatible",
        provider_preset: "custom",
        model_id: "my-custom-stt",
      },
    });
    const settings = makeSettings({
      transcription_provider: "local",
      remote_stt: {
        provider_preset: "custom",
        base_url: "http://127.0.0.1:8080/v1",
        model_id: "other-model",
        allow_insecure_http: true,
        debug_capture: false,
        debug_mode: "normal",
      },
    });

    const context = resolveRemoteSttProfileContext(settings, customProfile);
    expect(context.canEditCustomEndpoint).toBe(true);
    expect(context.canMutateOpenAiCredentials).toBe(true);
    expect(context.effectiveRemoteBaseUrl).toBe("http://127.0.0.1:8080/v1");
    expect(context.allowInsecureHttp).toBe(true);
    expect(context.effectiveRemoteModelId).toBe("my-custom-stt");
  });

  test("routes Gemini route and default-profile changes appropriately", () => {
    const geminiProfile = makeProfile({
      id: "profile-gemini",
      stt_model_selection_override: {
        provider: "remote_openai_compatible",
        provider_preset: "vercel",
        model_id: "google/gemini-3.5-transcribe-live",
      },
    });
    const settings = makeSettings({
      transcription_provider: "remote_openai_compatible",
    });

    const profileContext = resolveRemoteSttProfileContext(
      settings,
      geminiProfile,
    );
    expect(planGeminiRouteChange(profileContext, "google", true)).toEqual({
      kind: "profile_override",
      selection: {
        provider: "remote_openai_compatible",
        provider_preset: "google",
        model_id: "gemini-3.5-transcribe-live",
      },
    });
    expect(planGeminiRouteChange(profileContext, "vercel", false)).toEqual({
      kind: "profile_override",
      selection: {
        provider: "remote_openai_compatible",
        provider_preset: "vercel",
        model_id: REMOTE_STT_PRESETS.vercel.defaultModel,
      },
    });

    const defaultContext = resolveRemoteSttProfileContext(settings, null);
    expect(defaultContext.isProfileScoped).toBe(false);
    expect(
      planProviderChange(defaultContext, "remote_openai_compatible", settings),
    ).toEqual({
      kind: "global",
      setting: {
        type: "transcription_provider",
        provider: "remote_openai_compatible",
      },
    });
    expect(planGeminiRouteChange(defaultContext, "google", false)).toEqual({
      kind: "global",
      setting: {
        type: "remote_stt_preset_and_model",
        preset: "google",
        modelId: REMOTE_STT_PRESETS.google.defaultModel,
      },
    });
    expect(planSonioxModelChange(defaultContext, "stt-async-v6")).toEqual({
      kind: "global",
      setting: {
        type: "soniox_model",
        modelId: "stt-async-v6",
      },
    });
    expect(planDeepgramModelChange(defaultContext, "nova-3-medical")).toEqual({
      kind: "global",
      setting: {
        type: "deepgram_model",
        modelId: "nova-3-medical",
      },
    });
  });

  test("ignores an old credential response after an A-B-A context round trip", async () => {
    const gate = createCredentialRequestGate();
    gate.setContext("profile-a:reson8");
    const isCurrentContext = gate.capture();
    let finishOldRequest!: () => void;
    const oldRequest = new Promise<void>((resolve) => {
      finishOldRequest = resolve;
    });
    let typedKey = "old input";
    let status = "saving";
    const response = oldRequest.then(() => {
      if (!isCurrentContext()) return;
      typedKey = "";
      status = "saved";
    });

    gate.setContext("profile-b:groq");
    gate.setContext("profile-a:reson8");
    typedKey = "new input";
    status = "ready";
    finishOldRequest();
    await response;
    expect(typedKey).toBe("new input");
    expect(status).toBe("ready");

    const isFreshContext = gate.capture();
    await Promise.resolve().then(() => {
      if (!isFreshContext()) return;
      status = "saved";
    });
    expect(status).toBe("saved");
  });

  test("keeps pending credential responses valid when the context stays the same", async () => {
    const gate = createCredentialRequestGate();
    gate.setContext("profile-a:reson8");
    const isCurrentContext = gate.capture();
    gate.setContext("profile-a:reson8");
    const result = await Promise.resolve("connected").then((value) =>
      isCurrentContext() ? value : "stale",
    );
    expect(result).toBe("connected");
  });

  test("normalizes unknown or empty presets defensively", () => {
    expect(normalizeRemoteSttPreset(undefined)).toBe("groq");
    expect(normalizeRemoteSttPreset("")).toBe("groq");
    expect(normalizeRemoteSttPreset("reson8")).toBe("reson8");
    expect(normalizeRemoteSttPreset("unknown_vendor")).toBe("custom");
  });
});
