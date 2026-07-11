const MODEL_RELEASE_DATES: Record<string, string> = {
  // Legacy bundled models.
  small: "2022-09-21",
  medium: "2022-09-21",
  turbo: "2024-09-30",
  large: "2023-11-06",
  "parakeet-tdt-0.6b-v2": "2025-05-01",
  "parakeet-tdt-0.6b-v3": "2025-08-14",
  "moonshine-base": "2024-10-21",
  "moonshine-tiny-streaming-en": "2026-02-13",
  "moonshine-small-streaming-en": "2026-02-13",
  "moonshine-medium-streaming-en": "2026-02-13",
  "sense-voice-int8": "2024-07-04",
  "gigaam-v3-e2e-ctc": "2025-11-20",
  "canary-180m-flash": "2025-03-12",
  "canary-1b-v2": "2025-08-14",
  "cohere-int8": "2026-03-26",

  // Catalog repositories. Quantizations share their upstream model's date.
  "handy-computer/parakeet-unified-en-0.6b-gguf": "2026-04-07",
  "handy-computer/nemotron-3.5-asr-streaming-0.6b-gguf": "2026-06-04",
  "handy-computer/canary-180m-flash-gguf": "2025-03-12",
  "handy-computer/cohere-transcribe-03-2026-gguf": "2026-03-26",
  "handy-computer/whisper-medium-gguf": "2022-09-21",
  "handy-computer/Voxtral-Mini-4B-Realtime-2602-gguf": "2026-02-04",
  "handy-computer/parakeet-tdt-0.6b-v3-gguf": "2025-08-14",
  "handy-computer/parakeet-tdt-0.6b-v2-gguf": "2025-05-01",
  "handy-computer/canary-1b-flash-gguf": "2025-03-07",
  "handy-computer/canary-1b-v2-gguf": "2025-08-14",
  "handy-computer/canary-1b-gguf": "2024-02-08",
  "handy-computer/canary-qwen-2.5b-gguf": "2025-07-17",
  "handy-computer/gigaam-v3-ctc-gguf": "2025-11-20",
  "handy-computer/gigaam-v3-e2e-ctc-gguf": "2025-11-20",
  "handy-computer/gigaam-v3-rnnt-gguf": "2025-11-20",
  "handy-computer/gigaam-v3-e2e-rnnt-gguf": "2025-11-20",
  "handy-computer/granite-4.0-1b-speech-gguf": "2026-03-06",
  "handy-computer/granite-speech-4.1-2b-gguf": "2026-04-29",
  "handy-computer/granite-speech-4.1-2b-plus-gguf": "2026-04-28",
  "handy-computer/medasr-gguf": "2025-12-19",
  "handy-computer/moonshine-tiny-gguf": "2024-10-21",
  "handy-computer/moonshine-streaming-tiny-gguf": "2026-02-13",
  "handy-computer/moonshine-tiny-ar-gguf": "2025-09-01",
  "handy-computer/moonshine-tiny-ja-gguf": "2025-09-01",
  "handy-computer/moonshine-base-gguf": "2024-10-21",
  "handy-computer/moonshine-base-ar-gguf": "2025-09-28",
  "handy-computer/moonshine-base-ko-gguf": "2025-09-25",
  "handy-computer/moonshine-base-vi-gguf": "2025-09-25",
  "handy-computer/moonshine-base-zh-gguf": "2025-11-02",
  "handy-computer/moonshine-streaming-small-gguf": "2026-02-13",
  "handy-computer/moonshine-streaming-medium-gguf": "2026-02-13",
  "handy-computer/nemotron-speech-streaming-en-0.6b-gguf": "2026-01-05",
  "handy-computer/parakeet-tdt_ctc-110m-gguf": "2024-09-17",
  "handy-computer/parakeet-ctc-0.6b-gguf": "2023-12-28",
  "handy-computer/parakeet-rnnt-0.6b-gguf": "2023-12-28",
  "handy-computer/parakeet-ctc-1.1b-gguf": "2023-12-28",
  "handy-computer/parakeet-tdt-1.1b-gguf": "2024-01-31",
  "handy-computer/parakeet-rnnt-1.1b-gguf": "2023-12-28",
  "handy-computer/parakeet-tdt_ctc-1.1b-gguf": "2024-05-07",
  "handy-computer/SenseVoiceSmall-gguf": "2024-07-04",
  "handy-computer/Voxtral-Mini-3B-2507-gguf": "2025-07-15",
  "handy-computer/Voxtral-Small-24B-2507-gguf": "2025-07-15",
  "handy-computer/whisper-tiny-gguf": "2022-09-21",
  "handy-computer/whisper-tiny.en-gguf": "2022-09-21",
  "handy-computer/whisper-base-gguf": "2022-09-21",
  "handy-computer/whisper-base.en-gguf": "2022-09-21",
  "handy-computer/whisper-small.en-gguf": "2022-09-21",
  "handy-computer/whisper-small-gguf": "2022-09-21",
  "handy-computer/whisper-medium.en-gguf": "2022-09-21",
  "handy-computer/whisper-large-v3-turbo-gguf": "2024-09-30",
  "handy-computer/whisper-large-v3-gguf": "2023-11-06",
  "handy-computer/whisper-large-gguf": "2022-09-21",
  "handy-computer/whisper-large-v2-gguf": "2022-12-05",
  "handy-computer/Breeze-ASR-25-gguf": "2025-06-16",
};

export function getModelReleaseDate(modelId: string): string | undefined {
  const directDate = MODEL_RELEASE_DATES[modelId];
  if (directDate) return directDate;

  const parts = modelId.split("/");
  if (parts.length >= 3) {
    return MODEL_RELEASE_DATES[`${parts[0]}/${parts[1]}`];
  }

  return undefined;
}
