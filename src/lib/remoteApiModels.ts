import type { TFunction } from "i18next";

export type RemoteApiRowId =
  | "groq"
  | "gemini_transcribe"
  | "gemini_live"
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
  preset: "groq" | "openai" | "vercel" | "google" | "custom";
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
