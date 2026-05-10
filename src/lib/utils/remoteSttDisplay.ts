interface RemoteSttDisplaySettings {
  provider_preset?: string | null;
  base_url?: string | null;
  model_id?: string | null;
}

const getHostname = (baseUrl?: string | null) => {
  const value = baseUrl?.trim();
  if (!value) {
    return "";
  }

  try {
    return new URL(value).hostname;
  } catch {
    return value.replace(/^https?:\/\//i, "").replace(/\/.*$/, "");
  }
};

export const getRemoteApiDisplayLabel = (
  remoteStt?: RemoteSttDisplaySettings | null,
) => {
  const preset = remoteStt?.provider_preset?.trim();
  const modelId = remoteStt?.model_id?.trim();

  if (preset === "groq") {
    return "Groq";
  }

  if (preset === "openai") {
    return modelId === "gpt-realtime-translate"
      ? "GPT Realtime Translate"
      : "GPT Realtime";
  }

  if (preset === "custom") {
    return getHostname(remoteStt?.base_url) || "Custom API";
  }

  return "Remote API";
};
