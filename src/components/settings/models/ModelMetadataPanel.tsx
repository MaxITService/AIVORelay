import React, { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown } from "lucide-react";
import type { ModelInfo } from "@/bindings";
import { LANGUAGES } from "@/lib/constants/languages";

const FALLBACK_LANGUAGE_LABELS = new Map(
  LANGUAGES.map((language) => [language.value, language.label] as const),
);

FALLBACK_LANGUAGE_LABELS.set("zh", "Chinese (Mandarin)");
FALLBACK_LANGUAGE_LABELS.set("yue", "Cantonese");

type MetadataRow = {
  label: string;
  value: string;
};

type MetadataView = {
  badges: string[];
  rows: MetadataRow[];
  languages: string[];
  languageCount: number;
};

type ModelDetailsCopy = {
  badgeGgml: string;
  badgeOnnx: string;
  badgeStreaming: string;
  badgePackage: string;
  badgeSingleFile: string;
  badgeTranslation: string;
  badgeAsrOnly: string;
  summary: string;
  runtime: string;
  runtimeWhisper: string;
  runtimeOnnx: string;
  runtimeOnnxStreaming: string;
  format: string;
  formatGgml: string;
  formatOnnx: string;
  formatOnnxStreaming: string;
  formatCustomWhisper: string;
  precision: string;
  downloadContents: string;
  downloadFolder: string;
  downloadFile: string;
  downloadCustom: string;
  translation: string;
  translationYes: string;
  translationNo: string;
  languages: string;
  languageUnknown: string;
};

function isRussianLocale(locale: string): boolean {
  return locale.toLowerCase().startsWith("ru");
}

function getModelDetailsCopy(locale: string): ModelDetailsCopy {
  if (isRussianLocale(locale)) {
    return {
      badgeGgml: "GGML",
      badgeOnnx: "ONNX",
      badgeStreaming: "Streaming",
      badgePackage: "Пакет",
      badgeSingleFile: "Один файл",
      badgeTranslation: "Перевод в английский",
      badgeAsrOnly: "Только ASR",
      summary: "Технические детали и поддерживаемые языки",
      runtime: "Рантайм",
      runtimeWhisper: "whisper.cpp / GGML",
      runtimeOnnx: "ONNX Runtime",
      runtimeOnnxStreaming: "ONNX Runtime (streaming)",
      format: "Формат",
      formatGgml: "GGML model file",
      formatOnnx: "ONNX package",
      formatOnnxStreaming: "Streaming ONNX package",
      formatCustomWhisper: "Custom Whisper GGML .bin",
      precision: "Точность / квантование",
      downloadContents: "Что скачивается",
      downloadFolder: "Распакованная папка модели",
      downloadFile: "Один скачиваемый файл",
      downloadCustom: "Локальный файл пользователя",
      translation: "Перевод",
      translationYes: "Поддерживается в английский",
      translationNo: "Не поддерживается",
      languages: "Поддерживаемые языки",
      languageUnknown: "Не указано",
    };
  }

  return {
    badgeGgml: "GGML",
    badgeOnnx: "ONNX",
    badgeStreaming: "Streaming",
    badgePackage: "Package",
    badgeSingleFile: "Single file",
    badgeTranslation: "Translates to English",
    badgeAsrOnly: "ASR only",
    summary: "Technical details & supported languages",
    runtime: "Runtime",
    runtimeWhisper: "whisper.cpp / GGML",
    runtimeOnnx: "ONNX Runtime",
    runtimeOnnxStreaming: "ONNX Runtime (streaming)",
    format: "Format",
    formatGgml: "GGML model file",
    formatOnnx: "ONNX package",
    formatOnnxStreaming: "Streaming ONNX package",
    formatCustomWhisper: "Custom Whisper GGML .bin",
    precision: "Precision / quantization",
    downloadContents: "Download contents",
    downloadFolder: "Extracted model folder",
    downloadFile: "Single downloaded file",
    downloadCustom: "User-provided local file",
    translation: "Translation",
    translationYes: "Supported to English",
    translationNo: "Not supported",
    languages: "Supported languages",
    languageUnknown: "Not declared",
  };
}

function formatLanguageCount(count: number, locale: string): string {
  return isRussianLocale(locale) ? `${count} языков` : `${count} languages`;
}

function formatTotalCount(count: number, locale: string): string {
  return isRussianLocale(locale) ? `${count} всего` : `${count} total`;
}

function inferPrecision(model: ModelInfo): string | null {
  const hint = `${model.id} ${model.filename}`.toLowerCase();

  if (hint.includes("int8")) return "INT8";
  if (hint.includes("q4_1")) return "Q4_1";
  if (hint.includes("q5_0")) return "Q5_0";
  if (hint.includes("q5_k")) return "Q5_K";

  return null;
}

function getRuntimeLabel(model: ModelInfo, copy: ModelDetailsCopy): string {
  switch (model.engine_type) {
    case "Whisper":
      return copy.runtimeWhisper;
    case "MoonshineStreaming":
      return copy.runtimeOnnxStreaming;
    default:
      return copy.runtimeOnnx;
  }
}

function getFormatLabel(model: ModelInfo, copy: ModelDetailsCopy): string {
  if (model.is_custom) {
    return copy.formatCustomWhisper;
  }

  if (model.engine_type === "Whisper") {
    return copy.formatGgml;
  }

  if (model.engine_type === "MoonshineStreaming") {
    return copy.formatOnnxStreaming;
  }

  return copy.formatOnnx;
}

function getDownloadContentsLabel(
  model: ModelInfo,
  copy: ModelDetailsCopy,
): string {
  if (model.is_custom) {
    return copy.downloadCustom;
  }

  if (model.is_directory) {
    return copy.downloadFolder;
  }

  return copy.downloadFile;
}

function normalizeLanguageCode(
  code: string,
  supportedLanguages: string[],
): string {
  if (
    supportedLanguages.includes("zh") &&
    (code === "zh-Hans" || code === "zh-Hant")
  ) {
    return "zh";
  }

  return code;
}

function getLocalizedLanguageLabel(code: string, locale: string): string {
  if (code === "zh") {
    return locale.startsWith("ru")
      ? "Китайский (мандарин)"
      : "Chinese (Mandarin)";
  }

  try {
    const displayNames = new Intl.DisplayNames([locale], { type: "language" });
    const localized = displayNames.of(code);

    if (localized) {
      return localized.charAt(0).toUpperCase() + localized.slice(1);
    }
  } catch {
    // Fall through to static labels below when Intl.DisplayNames is unavailable.
  }

  return FALLBACK_LANGUAGE_LABELS.get(code) ?? code;
}

function getLocalizedLanguages(model: ModelInfo, locale: string): string[] {
  const seen = new Set<string>();
  const labels: string[] = [];

  for (const rawCode of model.supported_languages) {
    const code = normalizeLanguageCode(rawCode, model.supported_languages);
    if (seen.has(code)) {
      continue;
    }

    seen.add(code);
    labels.push(getLocalizedLanguageLabel(code, locale));
  }

  return labels;
}

function buildMetadataView(model: ModelInfo, locale: string): MetadataView {
  const copy = getModelDetailsCopy(locale);
  const precision = inferPrecision(model);
  const languages = getLocalizedLanguages(model, locale);
  const badges = [
    model.engine_type === "Whisper"
      ? copy.badgeGgml
      : model.engine_type === "MoonshineStreaming"
        ? copy.badgeStreaming
        : copy.badgeOnnx,
    ...(precision ? [precision] : []),
    model.is_directory ? copy.badgePackage : copy.badgeSingleFile,
    model.supports_translation ? copy.badgeTranslation : copy.badgeAsrOnly,
  ];

  if (languages.length > 0) {
    badges.push(formatLanguageCount(languages.length, locale));
  }

  const rows: MetadataRow[] = [
    {
      label: copy.runtime,
      value: getRuntimeLabel(model, copy),
    },
    {
      label: copy.format,
      value: getFormatLabel(model, copy),
    },
    {
      label: copy.downloadContents,
      value: getDownloadContentsLabel(model, copy),
    },
    {
      label: copy.translation,
      value: model.supports_translation
        ? copy.translationYes
        : copy.translationNo,
    },
  ];

  if (precision) {
    rows.splice(2, 0, {
      label: copy.precision,
      value: precision,
    });
  }

  if (languages.length > 0) {
    rows.push({
      label: copy.languages,
      value: formatTotalCount(languages.length, locale),
    });
  } else {
    rows.push({
      label: copy.languages,
      value: copy.languageUnknown,
    });
  }

  return {
    badges,
    rows,
    languages,
    languageCount: languages.length,
  };
}

export const ModelMetadataPanel: React.FC<{ model: ModelInfo }> = ({
  model,
}) => {
  const { i18n } = useTranslation();
  const copy = useMemo(
    () => getModelDetailsCopy(i18n.language),
    [i18n.language],
  );

  const metadata = useMemo(
    () => buildMetadataView(model, i18n.language),
    [i18n.language, model],
  );

  return (
    <div className="mt-3 space-y-3">
      <div className="flex flex-wrap gap-2">
        {metadata.badges.map((badge) => (
          <span
            key={badge}
            className="rounded-full border border-[#3d3d3d] bg-[#1b1b1b] px-2.5 py-1 text-[11px] text-[#cfcfcf]"
          >
            {badge}
          </span>
        ))}
      </div>

      <details className="group rounded-lg border border-[#3d3d3d] bg-[#141414] overflow-hidden">
        <summary className="flex cursor-pointer list-none items-center justify-between gap-3 px-3 py-2.5 text-xs text-[#d6d6d6] hover:bg-white/5 [&::-webkit-details-marker]:hidden">
          <span>{copy.summary}</span>
          <span className="flex items-center gap-2 text-[#8a8a8a]">
            <span>
              {metadata.languageCount > 0
                ? formatLanguageCount(metadata.languageCount, i18n.language)
                : copy.languageUnknown}
            </span>
            <ChevronDown className="h-4 w-4 transition-transform group-open:rotate-180" />
          </span>
        </summary>

        <div className="space-y-3 border-t border-[#3d3d3d] px-3 py-3">
          <div className="grid gap-2 sm:grid-cols-2">
            {metadata.rows.map((row) => (
              <div
                key={row.label}
                className="rounded-md border border-[#2b2b2b] bg-black/20 p-2.5"
              >
                <p className="text-[11px] uppercase tracking-wide text-[#7f7f7f]">
                  {row.label}
                </p>
                <p className="mt-1 text-xs text-[#f0f0f0]">{row.value}</p>
              </div>
            ))}
          </div>

          {metadata.languages.length > 0 && (
            <div className="space-y-2">
              <p className="text-[11px] uppercase tracking-wide text-[#7f7f7f]">
                {copy.languages}
              </p>
              <div className="flex flex-wrap gap-2">
                {metadata.languages.map((language) => (
                  <span
                    key={language}
                    className="rounded-full border border-[#2f2f2f] bg-[#1b1b1b] px-2.5 py-1 text-[11px] text-[#d8d8d8]"
                  >
                    {language}
                  </span>
                ))}
              </div>
            </div>
          )}
        </div>
      </details>
    </div>
  );
};
