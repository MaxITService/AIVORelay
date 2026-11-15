import type { TFunction } from "i18next";
import type { RemoteSttSettings, SttModelSelection } from "@/bindings";
import type { RemoteSttPreset } from "@/lib/constants/remoteSttProviders";

export type RemoteApiRowId =
  | "groq"
  | "gemini_transcribe"
  | "gemini_live"
  | "reson8"
  | "elevenlabs"
  | "openai_transcribe"
  | "openai_live_transcribe"
  | "openai_realtime_whisper"
  | "openai_realtime2"
  | "openai_realtime2_1"
  | "openai_translate"
  | "custom";

export type RemoteApiRow = {
  id: RemoteApiRowId;
  title: string;
  description: string;
  notRecommended?: boolean;
  preset: RemoteSttPreset;
  modelId?: string;
  iconClassName: string;
};

export const getRemoteApiRows = (t: TFunction): RemoteApiRow[] => [
  {
    id: "groq",
    title: t("modelSelector.remoteApiRows.groq.title"),
    description: t("modelSelector.remoteApiRows.groq.description"),
    preset: "groq",
    modelId: "whisper-large-v3-turbo",
    iconClassName: "text-sky-400",
  },
  {
    id: "gemini_transcribe",
    title: t("modelSelector.remoteApiRows.geminiTranscribe.title"),
    description: t("modelSelector.remoteApiRows.geminiTranscribe.description"),
    preset: "vercel",
    modelId: "google/gemini-3.5-transcribe",
    iconClassName: "text-amber-300",
  },
  {
    id: "gemini_live",
    title: t("modelSelector.remoteApiRows.geminiLive.title"),
    description: t("modelSelector.remoteApiRows.geminiLive.description"),
    preset: "vercel",
    modelId: "google/gemini-3.5-transcribe-live",
    iconClassName: "text-amber-400",
  },
  {
    id: "reson8",
    title: t("modelSelector.remoteApiRows.reson8.title"),
    description: t("modelSelector.remoteApiRows.reson8.description"),
    preset: "reson8",
    modelId: "rumble-2",
    iconClassName: "text-emerald-300",
  },
  {
    id: "elevenlabs",
    title: t(
      "modelSelector.remoteApiRows.elevenlabs.title",
      "Remote via ElevenLabs (Scribe v3 Live)",
    ),
    description: t(
      "modelSelector.remoteApiRows.elevenlabs.description",
      "Live dictation and Live Monitor use Scribe v3 Live. Files use Scribe v3 with SRT/VTT subtitles. Speaker separation is available in Live Monitor.",
    ),
    preset: "elevenlabs",
    modelId: "scribe_v3_live",
    iconClassName: "text-fuchsia-300",
  },
  {
    id: "custom",
    title: t("modelSelector.remoteApiRows.custom.title"),
    description: t("modelSelector.remoteApiRows.custom.description"),
    preset: "custom",
    iconClassName: "text-slate-300",
  },
  {
    id: "openai_transcribe",
    title: t("modelSelector.remoteApiRows.openAiTranscribe.title"),
    description: t("modelSelector.remoteApiRows.openAiTranscribe.description"),
    preset: "openai",
    modelId: "gpt-transcribe",
    iconClassName: "text-emerald-400",
  },
  {
    id: "openai_live_transcribe",
    title: t("modelSelector.remoteApiRows.openAiLiveTranscribe.title"),
    description: t("modelSelector.remoteApiRows.openAiLiveTranscribe.description"),
    preset: "openai",
    modelId: "gpt-live-transcribe",
    iconClassName: "text-cyan-400",
  },
  {
    id: "openai_realtime_whisper",
    title: t("modelSelector.remoteApiRows.openAiRealtimeWhisper.title"),
    description: t("modelSelector.remoteApiRows.openAiRealtimeWhisper.description"),
    notRecommended: true,
    preset: "openai",
    modelId: "gpt-realtime-whisper",
    iconClassName: "text-emerald-400",
  },
  {
    id: "openai_realtime2",
    title: t("modelSelector.remoteApiRows.openAiRealtime2.title"),
    description: t("modelSelector.remoteApiRows.openAiRealtime2.description"),
    notRecommended: true,
    preset: "openai",
    modelId: "gpt-realtime-2",
    iconClassName: "text-blue-400",
  },
  {
    id: "openai_realtime2_1",
    title: t("modelSelector.remoteApiRows.openAiRealtime21.title"),
    description: t("modelSelector.remoteApiRows.openAiRealtime21.description"),
    notRecommended: true,
    preset: "openai",
    modelId: "gpt-realtime-2.1",
    iconClassName: "text-cyan-400",
  },
  {
    id: "openai_translate",
    title: t("modelSelector.remoteApiRows.openAiTranslate.title"),
    description: t("modelSelector.remoteApiRows.openAiTranslate.description"),
    notRecommended: true,
    preset: "openai",
    modelId: "gpt-realtime-translate",
    iconClassName: "text-violet-400",
  },
];

export const resolveRemoteApiRow = (
  row: RemoteApiRow,
  currentPreset?: string | null,
): { preset: RemoteApiRow["preset"]; modelId?: string } => {
  const isGeminiRow =
    row.id === "gemini_transcribe" || row.id === "gemini_live";
  const preset =
    isGeminiRow && (currentPreset === "vercel" || currentPreset === "google")
      ? currentPreset
      : row.preset;
  const modelId = isGeminiRow
    ? row.id === "gemini_live"
      ? preset === "google"
        ? "gemini-3.5-transcribe-live"
        : "google/gemini-3.5-transcribe-live"
      : preset === "google"
        ? "gemini-3.5-transcribe"
        : "google/gemini-3.5-transcribe"
    : row.modelId;
  return { preset, modelId };
};

export type RemoteApiGlobalCustomSettings = Pick<
  Partial<RemoteSttSettings>,
  "provider_preset" | "base_url" | "model_id"
>;

export type RemoteApiSelectionPlan =
  | {
      target: "global";
      preset: RemoteApiRow["preset"];
      modelId?: string;
    }
  | {
      target: "profile";
      selection: SttModelSelection;
    }
  | {
      target: "blocked";
      reason: "custom_unconfigured";
    };

export const getConfiguredCustomRemoteSttModelId = (
  globalRemoteStt?: RemoteApiGlobalCustomSettings | null,
): string | null => {
  if (globalRemoteStt?.provider_preset?.trim() !== "custom") {
    return null;
  }
  const baseUrl = globalRemoteStt.base_url?.trim();
  const modelId = globalRemoteStt.model_id?.trim();
  if (!baseUrl || !modelId) {
    return null;
  }
  return modelId;
};

export const resolveRemoteApiSelection = (
  row: RemoteApiRow,
  {
    isCustomProfile,
    currentEffectivePreset,
    globalRemoteStt,
  }: {
    isCustomProfile: boolean;
    currentEffectivePreset?: string | null;
    globalRemoteStt?: RemoteApiGlobalCustomSettings | null;
  },
): RemoteApiSelectionPlan => {
  const { preset, modelId } = resolveRemoteApiRow(row, currentEffectivePreset);
  if (!isCustomProfile) {
    return { target: "global", preset, modelId };
  }

  if (row.id === "custom") {
    const customModelId = getConfiguredCustomRemoteSttModelId(globalRemoteStt);
    if (!customModelId) {
      return { target: "blocked", reason: "custom_unconfigured" };
    }
    return {
      target: "profile",
      selection: {
        provider: "remote_openai_compatible",
        provider_preset: "custom",
        model_id: customModelId,
      },
    };
  }

  const profileModelId = modelId?.trim();
  if (!profileModelId) {
    return { target: "blocked", reason: "custom_unconfigured" };
  }

  return {
    target: "profile",
    selection: {
      provider: "remote_openai_compatible",
      provider_preset: preset,
      model_id: profileModelId,
    },
  };
};
