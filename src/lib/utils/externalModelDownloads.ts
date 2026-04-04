export type ExternalDownloadInfoTemplate = {
  sourceLabel: string;
  sourceUrl: string;
  privacyUrl: string;
  termsUrl: string;
  files: string[];
};

export const HUGGING_FACE_PRIVACY_URL = "https://huggingface.co/privacy";
export const HUGGING_FACE_TERMS_URL = "https://huggingface.co/terms-of-service";

export const EXTERNAL_MODEL_DOWNLOADS: Record<string, ExternalDownloadInfoTemplate> = {
  "cohere-fp32": {
    sourceLabel: "Hugging Face (eschmidbauer + ONNX Community)",
    sourceUrl: "https://huggingface.co/eschmidbauer/cohere-transcribe-03-2026-onnx",
    privacyUrl: HUGGING_FACE_PRIVACY_URL,
    termsUrl: HUGGING_FACE_TERMS_URL,
    files: [
      "eschmidbauer/cohere-transcribe-03-2026-onnx/encoder-0.onnx",
      "eschmidbauer/cohere-transcribe-03-2026-onnx/encoder-1.onnx",
      "eschmidbauer/cohere-transcribe-03-2026-onnx/encoder-2.onnx",
      "eschmidbauer/cohere-transcribe-03-2026-onnx/encoder-3.onnx",
      "eschmidbauer/cohere-transcribe-03-2026-onnx/cross_kv.onnx",
      "eschmidbauer/cohere-transcribe-03-2026-onnx/decoder.onnx",
      "onnx-community/cohere-transcribe-03-2026-ONNX/config.json",
      "onnx-community/cohere-transcribe-03-2026-ONNX/generation_config.json",
      "onnx-community/cohere-transcribe-03-2026-ONNX/preprocessor_config.json",
      "onnx-community/cohere-transcribe-03-2026-ONNX/processor_config.json",
      "onnx-community/cohere-transcribe-03-2026-ONNX/tokenizer.json",
      "onnx-community/cohere-transcribe-03-2026-ONNX/tokenizer_config.json",
    ],
  },
};

export const hasExternalModelDownload = (modelId: string): boolean =>
  Boolean(EXTERNAL_MODEL_DOWNLOADS[modelId]);
