import { useId, useMemo, useState, type FormEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/Button";
import { Textarea } from "@/components/ui/Textarea";
import { sessionToast as toast } from "@/lib/sessionToast";
import { useBenchmarkStore, type BenchmarkClipInfo } from "@/stores/benchmarkStore";
import { compareTranscripts, type TranscriptPart } from "./transcriptComparison";

export function BenchmarkClipReferenceEditor({ clip }: { clip: BenchmarkClipInfo }) {
  const { t } = useTranslation();
  const updateClip = useBenchmarkStore((state) => state.updateClip);
  const [draft, setDraft] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const inputId = useId();

  const saveReference = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (draft === null || saving) return;
    const referenceText = draft.trim();
    if (referenceText === (clip.referenceText ?? "")) {
      setDraft(null);
      return;
    }
    setSaving(true);
    try {
      await updateClip(clip.id, { referenceText });
      setDraft(null);
    } catch (error) {
      console.error("Failed to save benchmark reference:", error);
      toast.error(t("benchmark.reference.saveFailed"));
    } finally {
      setSaving(false);
    }
  };

  if (draft === null) {
    return (
      <Button
        type="button"
        variant="ghost"
        size="sm"
        onClick={() => setDraft(clip.referenceText ?? "")}
      >
        {t(clip.referenceText ? "benchmark.reference.edit" : "benchmark.reference.add")}
      </Button>
    );
  }

  return (
    <form
      className="space-y-2"
      onSubmit={(e) => void saveReference(e)}
      onKeyDown={(e) => {
        if (e.key === "Escape" && !e.nativeEvent.isComposing && !saving) {
          e.preventDefault();
          e.stopPropagation();
          setDraft(null);
        }
      }}
    >
      <label htmlFor={inputId} className="block text-sm font-medium">
        {t("benchmark.reference.title")}
      </label>
      <Textarea
        id={inputId}
        dir="auto"
        autoFocus
        variant="compact"
        className="block w-full"
        value={draft}
        readOnly={saving}
        onChange={(e) => setDraft(e.target.value)}
        aria-describedby={`${inputId}-hint`}
      />
      <p id={`${inputId}-hint`} className="text-xs text-[#9a9a9a]">
        {t("benchmark.reference.hint")}
      </p>
      <div className="flex gap-2">
        <Button type="submit" size="sm" disabled={saving}>
          {t("benchmark.reference.save")}
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          disabled={saving}
          onClick={() => setDraft(null)}
        >
          {t("benchmark.reference.cancel")}
        </Button>
      </div>
    </form>
  );
}

const editStyles = {
  substitutions: "bg-amber-400/25 text-amber-100",
  omissions: "bg-red-400/25 text-red-100",
  additions: "bg-sky-400/25 text-sky-100",
};
const edits = ["substitutions", "omissions", "additions"] as const;

function ComparisonText({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="space-y-1 min-w-0">
      <h4 className="text-xs font-medium text-[#b8b8b8]">{label}</h4>
      <p
        dir="auto"
        className="max-h-60 overflow-auto rounded-lg bg-black/25 p-2.5 text-sm whitespace-pre-wrap wrap-anywhere select-text"
      >
        {children}
      </p>
    </div>
  );
}

export function BenchmarkTranscriptComparison({
  reference,
  recognized,
}: {
  reference: string;
  recognized: string;
}) {
  const { t, i18n } = useTranslation();
  const [open, setOpen] = useState(false);
  const comparison = useMemo(
    () => (open ? compareTranscripts(reference, recognized) : null),
    [open, reference, recognized],
  );
  const wordErrorRate = comparison?.wordErrorRate;

  const renderParts = (parts: TranscriptPart[]) =>
    parts.map((part, index) =>
      part.edit ? (
        <mark
          key={index}
          className={`rounded-sm ${editStyles[part.edit]}`}
          title={t(`benchmark.reference.${part.edit}`)}
        >
          {part.text}
        </mark>
      ) : (
        part.text
      ),
    );
  const recognizedText = comparison ? renderParts(comparison.recognized) : recognized;

  return (
    <div className="space-y-2">
      <Button
        type="button"
        variant="secondary"
        size="sm"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
      >
        {t(open ? "benchmark.reference.hideComparison" : "benchmark.reference.compare")}
      </Button>
      {open && (
        <div className="space-y-2 rounded-lg border border-white/10 p-3">
          {comparison ? (
            <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs">
              {wordErrorRate != null && (
                <span
                  className="font-medium"
                  title={t("benchmark.reference.wordErrorRateHelp")}
                >
                  {t("benchmark.reference.wordErrorRate")}{" "}
                  {wordErrorRate.toLocaleString(i18n.resolvedLanguage, {
                    style: "percent",
                    maximumFractionDigits: 1,
                  })}
                </span>
              )}
              {edits.map((edit) => (
                <span key={edit} className={`rounded px-1 ${editStyles[edit]}`}>
                  {t(`benchmark.reference.${edit}`)} {comparison.counts[edit]}
                </span>
              ))}
            </div>
          ) : (
            <p className="text-xs text-[#b8b8b8]">{t("benchmark.reference.tooLong")}</p>
          )}
          <div className="grid gap-2 sm:grid-cols-2">
            <ComparisonText label={t("benchmark.reference.title")}>
              {comparison ? renderParts(comparison.reference) : reference}
            </ComparisonText>
            <ComparisonText label={t("benchmark.reference.recognized")}>
              {recognized.trim() ? recognizedText : t("benchmark.results.empty")}
            </ComparisonText>
          </div>
        </div>
      )}
    </div>
  );
}
