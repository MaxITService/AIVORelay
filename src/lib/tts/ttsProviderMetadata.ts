export type TtsProvider =
  | "soniox"
  | "deepgram"
  | "openai"
  | "murf"
  | "elevenlabs"
  | "cartesia"
  | "openai_compatible"
  | "edge"
  | "local_qwen"
  | "local_kokoro"
  | "windows";

export type TtsProviderOption = {
  value: TtsProvider;
  label: string;
  group: string;
};

export type TtsProviderDocumentation = {
  overview: string;
  voices: string;
  parameters: string;
  authentication?: string;
  playground?: string;
};

export const DEEPGRAM_FLUX_DEFAULT_MODEL = "flux-kit-en";
export const DEEPGRAM_FLUX_SPEED_RANGE: [number, number] = [0.85, 1.15];

export const isDeepgramFluxModel = (model: string): boolean =>
  model.trim().toLowerCase().startsWith("flux-");

export const TTS_PROVIDER_OPTIONS: TtsProviderOption[] = [
  { value: "soniox", label: "Soniox", group: "Cloud" },
  { value: "deepgram", label: "Deepgram", group: "Cloud" },
  { value: "openai", label: "OpenAI", group: "Cloud" },
  { value: "murf", label: "Murf AI", group: "Cloud" },
  { value: "elevenlabs", label: "ElevenLabs", group: "Cloud" },
  { value: "cartesia", label: "Cartesia", group: "Cloud" },
  { value: "openai_compatible", label: "OpenAI-compatible", group: "Cloud" },
  {
    value: "edge",
    label: "Microsoft Read Aloud (unofficial)",
    group: "Online without an API key",
  },
  { value: "local_qwen", label: "Qwen3-TTS (Local)", group: "On device" },
  {
    value: "local_kokoro",
    label: "Kokoro 82M (Local)",
    group: "On device",
  },
  { value: "windows", label: "Windows voices", group: "System" },
];

export const TTS_PROVIDER_DEFAULTS: Record<
  TtsProvider,
  { model: string; voice: string; language: string; speed: number }
> = {
  soniox: {
    model: "tts-rt-v1",
    voice: "Maya",
    language: "en",
    speed: 1,
  },
  deepgram: {
    model: DEEPGRAM_FLUX_DEFAULT_MODEL,
    voice: DEEPGRAM_FLUX_DEFAULT_MODEL,
    language: "en",
    speed: 1,
  },
  openai: {
    model: "gpt-4o-mini-tts",
    voice: "marin",
    language: "",
    speed: 1,
  },
  murf: {
    model: "falcon-2",
    voice: "Gordon",
    language: "en-US",
    speed: 1,
  },
  elevenlabs: {
    model: "eleven_flash_v2_5",
    voice: "JBFqnCBsd6RMkjVDRZzb",
    language: "en",
    speed: 1,
  },
  cartesia: {
    model: "sonic-3.5",
    voice: "f786b574-daa5-4673-aa0c-cbe3e8534c02",
    language: "en",
    speed: 1,
  },
  openai_compatible: {
    model: "tts-1",
    voice: "alloy",
    language: "",
    speed: 1,
  },
  edge: {
    model: "microsoft-edge-read-aloud",
    voice: "en-US-AriaNeural",
    language: "en-US",
    speed: 1,
  },
  local_qwen: {
    model: "qwen3-tts-12hz-0.6b-customvoice",
    voice: "Ryan",
    language: "Auto",
    speed: 1,
  },
  local_kokoro: {
    model: "kokoro-82m",
    voice: "af_maple",
    language: "English",
    speed: 1,
  },
  windows: { model: "windows-media-speech", voice: "", language: "", speed: 1 },
};

export const TTS_PROVIDER_SPEED_RANGES: Record<TtsProvider, [number, number]> =
  {
    soniox: [0.7, 1.3],
    deepgram: [0.7, 1.5],
    openai: [0.25, 4],
    murf: [1, 1],
    elevenlabs: [0.7, 1.2],
    cartesia: [0.6, 1.5],
    openai_compatible: [0.25, 4],
    edge: [0.5, 2],
    local_qwen: [0.5, 2],
    local_kokoro: [0.5, 2],
    windows: [0.5, 2],
  };

export const TTS_PROVIDER_DOCUMENTATION: Record<
  TtsProvider,
  TtsProviderDocumentation
> = {
  soniox: {
    overview: "https://soniox.com/docs/tts/get-started",
    voices: "https://soniox.com/docs/tts/concepts/voices",
    parameters: "https://soniox.com/docs/api-reference/tts/generate_tts",
    authentication: "https://soniox.com/docs/tts/get-started",
  },
  deepgram: {
    overview:
      "https://developers.deepgram.com/docs/tts-models-languages-overview",
    voices:
      "https://developers.deepgram.com/docs/tts-models-languages-overview",
    parameters:
      "https://developers.deepgram.com/reference/text-to-speech/speak-request",
    authentication:
      "https://developers.deepgram.com/guides/fundamentals/authenticating",
    playground: "https://playground.deepgram.com/",
  },
  openai: {
    overview: "https://developers.openai.com/api/docs/guides/text-to-speech",
    voices:
      "https://developers.openai.com/api/docs/guides/text-to-speech#voice-options",
    parameters:
      "https://developers.openai.com/api/reference/resources/audio/subresources/speech/methods/create",
    authentication: "https://developers.openai.com/api/docs/quickstart",
    playground: "https://www.openai.fm/",
  },
  murf: {
    overview: "https://murf.ai/api/docs/introduction/overview",
    voices: "https://murf.ai/api/docs/api-reference/voices/get-voices",
    parameters:
      "https://murf.ai/api/docs/text-to-speech/speech-customization",
    authentication: "https://murf.ai/api/docs/introduction/overview",
  },
  elevenlabs: {
    overview:
      "https://elevenlabs.io/docs/overview/capabilities/text-to-speech",
    voices: "https://elevenlabs.io/docs/api-reference/voices/search",
    parameters:
      "https://elevenlabs.io/docs/api-reference/text-to-speech/convert",
    authentication:
      "https://elevenlabs.io/docs/api-reference/authentication",
  },
  cartesia: {
    overview:
      "https://docs.cartesia.ai/build-with-cartesia/tts-models/latest",
    voices: "https://docs.cartesia.ai/api-reference/voices/list",
    parameters:
      "https://docs.cartesia.ai/build-with-cartesia/capability-guides/volume-speed-emotion",
    authentication:
      "https://docs.cartesia.ai/use-the-api/api-conventions",
  },
  openai_compatible: {
    overview: "https://platform.openai.com/docs/guides/text-to-speech",
    voices:
      "https://platform.openai.com/docs/guides/text-to-speech#voice-options",
    parameters:
      "https://platform.openai.com/docs/api-reference/audio/createSpeech",
  },
  edge: {
    overview: "https://www.microsoft.com/en-us/edge/features/read-aloud",
    voices:
      "https://learn.microsoft.com/en-us/azure/ai-services/speech-service/language-support",
    parameters:
      "https://support.microsoft.com/en-us/accessibility/edge/accessibility-features-in-microsoft-edge",
  },
  local_qwen: {
    overview: "https://github.com/QwenLM/Qwen3-TTS",
    voices: "https://huggingface.co/Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice",
    parameters: "https://huggingface.co/Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice",
    playground: "https://huggingface.co/spaces/Qwen/Qwen3-TTS",
  },
  local_kokoro: {
    overview:
      "https://k2-fsa.github.io/sherpa/onnx/tts/pretrained_models/kokoro.html",
    voices:
      "https://k2-fsa.github.io/sherpa/onnx/tts/all/Chinese-English/kokoro-multi-lang-v1_1.html",
    parameters:
      "https://k2-fsa.github.io/sherpa/onnx/tts/pretrained_models/kokoro.html",
    playground: "https://huggingface.co/spaces/hexgrad/Kokoro-TTS",
  },
  windows: {
    overview:
      "https://learn.microsoft.com/uwp/api/windows.media.speechsynthesis.speechsynthesizer",
    voices:
      "https://support.microsoft.com/windows/download-languages-and-voices-for-immersive-reader-read-mode-and-read-aloud-4c83a8d8-7486-42f7-8e46-2b0fdf753130",
    parameters:
      "https://learn.microsoft.com/uwp/api/windows.media.speechsynthesis.speechsynthesizeroptions",
  },
};

export const SONIOX_MODEL_OPTIONS = ["tts-rt-v1", "tts-rt-v1-preview"];

export const OPENAI_MODEL_OPTIONS = [
  "gpt-4o-mini-tts",
  "gpt-4o-mini-tts-2025-12-15",
  "tts-1",
  "tts-1-hd",
];

export const MURF_MODEL_OPTIONS = ["falcon-2", "gen2"];

export const ELEVENLABS_MODEL_OPTIONS = [
  "eleven_flash_v2_5",
  "eleven_multilingual_v2",
  "eleven_v3",
];

export const CARTESIA_MODEL_OPTIONS = ["sonic-3.5"];

export const AIVORELAY_TTS_GUIDE_URL =
  "https://github.com/MaxITService/AIVORelay/blob/main/CLI-TEXT-TO-SPEECH.md";

export const SONIOX_LANGUAGE_OPTIONS = [
  ["af", "Afrikaans"],
  ["sq", "Albanian"],
  ["ar", "Arabic"],
  ["az", "Azerbaijani"],
  ["eu", "Basque"],
  ["be", "Belarusian"],
  ["bn", "Bengali"],
  ["bs", "Bosnian (Latin script)"],
  ["bg", "Bulgarian"],
  ["ca", "Catalan"],
  ["zh", "Chinese (Simplified)"],
  ["hr", "Croatian"],
  ["cs", "Czech"],
  ["da", "Danish"],
  ["nl", "Dutch"],
  ["en", "English"],
  ["et", "Estonian"],
  ["fi", "Finnish"],
  ["fr", "French"],
  ["gl", "Galician"],
  ["de", "German"],
  ["el", "Greek"],
  ["gu", "Gujarati"],
  ["he", "Hebrew"],
  ["hi", "Hindi"],
  ["hu", "Hungarian"],
  ["id", "Indonesian"],
  ["it", "Italian"],
  ["ja", "Japanese"],
  ["kn", "Kannada"],
  ["kk", "Kazakh (Cyrillic script)"],
  ["ko", "Korean"],
  ["lv", "Latvian"],
  ["lt", "Lithuanian"],
  ["mk", "Macedonian"],
  ["ms", "Malay"],
  ["ml", "Malayalam"],
  ["mr", "Marathi"],
  ["no", "Norwegian"],
  ["fa", "Persian"],
  ["pl", "Polish"],
  ["pt", "Portuguese"],
  ["pa", "Punjabi"],
  ["ro", "Romanian"],
  ["ru", "Russian"],
  ["sr", "Serbian (Latin script)"],
  ["sk", "Slovak"],
  ["sl", "Slovenian"],
  ["es", "Spanish"],
  ["sw", "Swahili"],
  ["sv", "Swedish"],
  ["tl", "Tagalog"],
  ["ta", "Tamil"],
  ["te", "Telugu"],
  ["th", "Thai"],
  ["tr", "Turkish"],
  ["uk", "Ukrainian"],
  ["ur", "Urdu"],
  ["vi", "Vietnamese"],
  ["cy", "Welsh"],
] as const;

export const LOCAL_TTS_INSTALL_METADATA = {
  qwen: {
    author: "Qwen Team (Alibaba Cloud)",
    sourceUrl:
      "https://huggingface.co/Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice/tree/85e237c12c027371202489a0ec509ded67b5e4b5",
    licenseName: "Apache License 2.0",
    licenseUrl:
      "https://huggingface.co/Qwen/Qwen3-TTS-12Hz-0.6B-CustomVoice/blob/85e237c12c027371202489a0ec509ded67b5e4b5/README.md",
    estimatedInstallBytes: 16 * 1024 ** 3,
  },
  kokoro: {
    author: "k2-fsa (sherpa-onnx), based on hexgrad Kokoro-82M",
    sourceUrl: "https://github.com/k2-fsa/sherpa-onnx/releases/tag/tts-models",
    licenseName: "Apache License 2.0",
    licenseUrl:
      "https://huggingface.co/csukuangfj/kokoro-int8-multi-lang-v1_1/blob/main/LICENSE",
    estimatedInstallBytes: 2 * 1024 ** 3,
  },
} as const;
