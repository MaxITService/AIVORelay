export const REMOTE_STT_PRESETS = {
  groq: {
    id: "groq",
    label: "Groq",
    baseUrl: "https://api.groq.com/openai/v1",
    defaultModel: "whisper-large-v3-turbo",
  },
  openai: {
    id: "openai",
    label: "OpenAI",
    baseUrl: "https://api.openai.com/v1",
    defaultModel: "whisper-1",
  },
  vercel: {
    id: "vercel",
    label: "Vercel AI Gateway",
    baseUrl: "https://ai-gateway.vercel.sh/v4/ai",
    defaultModel: "google/gemini-3.5-transcribe",
  },
  google: {
    id: "google",
    label: "Google Gemini API",
    baseUrl: "https://generativelanguage.googleapis.com/v1beta",
    defaultModel: "gemini-3.5-transcribe",
  },
  reson8: {
    id: "reson8",
    label: "Reson8",
    baseUrl: "https://api.reson8.dev/v1",
    defaultModel: "rumble-2",
  },
  elevenlabs: {
    id: "elevenlabs",
    label: "ElevenLabs",
    baseUrl: "https://api.elevenlabs.io/v1",
    defaultModel: "scribe_v3_live",
  },
  custom: {
    id: "custom",
    label: "Custom",
    baseUrl: "https://api.openai.com/v1",
    defaultModel: "gpt-realtime-2.1",
  },
} as const;

export type RemoteSttPreset = keyof typeof REMOTE_STT_PRESETS;

export const REMOTE_STT_PRESET_IDS = Object.keys(
  REMOTE_STT_PRESETS,
) as RemoteSttPreset[];

export const REMOTE_STT_PRESET_ELEVENLABS = "elevenlabs" as const;
export const REMOTE_STT_ELEVENLABS_BASE_URL =
  REMOTE_STT_PRESETS.elevenlabs.baseUrl;
export const REMOTE_STT_ELEVENLABS_DEFAULT_MODEL =
  REMOTE_STT_PRESETS.elevenlabs.defaultModel;
export const REMOTE_STT_ELEVENLABS_FILE_MODEL = "scribe_v3" as const;
