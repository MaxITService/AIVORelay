import type {
  AppSettings,
  ModelInfo,
  SttModelSelection as BindingSttModelSelection,
} from "@/bindings";

export type SttWorkflow = "dictation" | "file" | "live";

export type SttModelSelection = BindingSttModelSelection;

export type SttCatalogOption = {
  id: string;
  providerId: string;
  providerLabel: string;
  modelLabel: string;
  selection: SttModelSelection;
  localModel?: ModelInfo;
};

const remoteSelection = (
  providerId: string,
  providerLabel: string,
  modelId: string,
  modelLabel: string,
): SttCatalogOption => ({
  id: `${providerId}:${modelId}`,
  providerId,
  providerLabel,
  modelLabel,
  selection: {
    provider: "remote_openai_compatible",
    model_id: modelId,
    provider_preset: providerId,
  },
});

export const sttSelectionKey = (selection: SttModelSelection): string =>
  `${selection.provider}|${selection.provider_preset}|${selection.model_id}`;

export const sttProviderId = (selection: SttModelSelection): string => {
  if (selection.provider === "local") return "local";
  if (selection.provider === "remote_soniox") return "soniox";
  if (selection.provider === "remote_deepgram") return "deepgram";
  return selection.provider_preset || "custom";
};

export const globalSttSelection = (
  settings: AppSettings | null | undefined,
): SttModelSelection => {
  const provider = settings?.transcription_provider ?? "local";
  if (provider === "remote_soniox") {
    return {
      provider,
      model_id: settings?.soniox_model || "stt-rt-v5",
      provider_preset: "",
    };
  }
  if (provider === "remote_deepgram") {
    return {
      provider,
      model_id: settings?.deepgram_model || "nova-3",
      provider_preset: "",
    };
  }
  if (provider === "remote_openai_compatible") {
    return {
      provider,
      model_id: settings?.remote_stt?.model_id || "whisper-large-v3-turbo",
      provider_preset: settings?.remote_stt?.provider_preset || "groq",
    };
  }
  return {
    provider: "local",
    model_id: settings?.selected_model || "",
    provider_preset: "",
  };
};

export const legacyLiveSttSelection = (
  settings: AppSettings | null | undefined,
): SttModelSelection => {
  const liveProvider = String(
    (settings as any)?.live_sound_transcription_provider ?? "remote_soniox",
  );
  if (liveProvider === "remote_deepgram") {
    return {
      provider: "remote_deepgram",
      model_id: settings?.deepgram_model || "nova-3",
      provider_preset: "",
    };
  }
  if (liveProvider === "remote_openai_compatible") {
    return {
      provider: "remote_openai_compatible",
      model_id: settings?.remote_stt?.model_id || "gpt-live-transcribe",
      provider_preset: settings?.remote_stt?.provider_preset || "openai",
    };
  }
  return {
    provider: "remote_soniox",
    model_id: settings?.soniox_model || "stt-rt-v5",
    provider_preset: "",
  };
};

export const sttCatalog = (
  workflow: SttWorkflow,
  localModels: ModelInfo[],
): SttCatalogOption[] => {
  const localOptions =
    workflow !== "live"
      ? localModels.map((model) => ({
          id: `local:${model.id}`,
          providerId: "local",
          providerLabel: "Local",
          modelLabel: model.name || model.id,
          selection: {
            provider: "local" as const,
            model_id: model.id,
            provider_preset: "",
          },
          localModel: model,
        }))
      : [];

  const liveOptions: SttCatalogOption[] = [
      {
        id: "soniox:stt-rt-v5",
        providerId: "soniox",
        providerLabel: "Soniox",
        modelLabel: "STT RT v5",
        selection: {
          provider: "remote_soniox",
          model_id: "stt-rt-v5",
          provider_preset: "",
        },
      },
      {
        id: "deepgram:nova-3",
        providerId: "deepgram",
        providerLabel: "Deepgram",
        modelLabel: "Nova 3",
        selection: {
          provider: "remote_deepgram",
          model_id: "nova-3",
          provider_preset: "",
        },
      },
      remoteSelection(
        "vercel",
        "Vercel",
        "google/gemini-3.5-transcribe-live",
        "Gemini 3.5 Transcribe Live",
      ),
      remoteSelection(
        "google",
        "Google",
        "gemini-3.5-transcribe-live",
        "Gemini 3.5 Transcribe Live",
      ),
      remoteSelection(
        "openai",
        "OpenAI",
        "gpt-live-transcribe",
        "GPT Live Transcribe",
      ),
      remoteSelection(
        "openai",
        "OpenAI",
        "gpt-realtime-whisper",
        "GPT Realtime Whisper · Legacy",
      ),
    ];

  if (workflow === "live") {
    return liveOptions;
  }

  const fileOptions: SttCatalogOption[] = [
    {
      id: "soniox:stt-async-v5",
      providerId: "soniox",
      providerLabel: "Soniox",
      modelLabel: "STT Async v5",
      selection: {
        provider: "remote_soniox",
        model_id: "stt-async-v5",
        provider_preset: "",
      },
    },
    {
      id: "deepgram:nova-3",
      providerId: "deepgram",
      providerLabel: "Deepgram",
      modelLabel: "Nova 3",
      selection: {
        provider: "remote_deepgram",
        model_id: "nova-3",
        provider_preset: "",
      },
    },
    remoteSelection(
      "vercel",
      "Vercel",
      "google/gemini-3.5-transcribe",
      "Gemini 3.5 Transcribe",
    ),
    remoteSelection(
      "google",
      "Google",
      "gemini-3.5-transcribe",
      "Gemini 3.5 Transcribe",
    ),
    remoteSelection("openai", "OpenAI", "gpt-transcribe", "GPT Transcribe"),
    remoteSelection(
      "groq",
      "Groq",
      "whisper-large-v3-turbo",
      "Whisper Large v3 Turbo",
    ),
  ];

  if (workflow === "file") {
    return [...localOptions, ...fileOptions];
  }

  const remoteOptions = [...liveOptions, ...fileOptions].filter(
    (option, index, options) =>
      options.findIndex(
        (candidate) =>
          sttSelectionKey(candidate.selection) ===
          sttSelectionKey(option.selection),
      ) === index,
  );
  return [...localOptions, ...remoteOptions];
};
