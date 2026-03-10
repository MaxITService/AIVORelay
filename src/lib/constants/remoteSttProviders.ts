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
  custom: {
    id: "custom",
    label: "Custom",
    baseUrl: "",
    defaultModel: "",
  },
} as const;

export type RemoteSttPreset = keyof typeof REMOTE_STT_PRESETS;

export const REMOTE_STT_PRESET_IDS = Object.keys(
  REMOTE_STT_PRESETS,
) as RemoteSttPreset[];
