import React, { useEffect, useMemo, useState } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";
import { useTranslation } from "react-i18next";
import { TellMeMore } from "../ui/TellMeMore";

const SONIOX_CONTEXT_MAX_CHARS = 10_000;

interface SonioxContextEditorProps {
  generalJson: string;
  text: string;
  terms: string[];
  disabled?: boolean;
  onCommitGeneralJson: (value: string) => Promise<void>;
  onCommitText: (value: string) => Promise<void>;
  onCommitTerms: (value: string[]) => Promise<void>;
  onDraftGeneralJsonChange?: (value: string) => void;
  onDraftTextChange?: (value: string) => void;
  onDraftTermsChange?: (value: string[]) => void;
}

interface GeneralValidationResult {
  valid: boolean;
  empty: boolean;
  error: string | null;
  count: number;
}

function parseGeneralJson(raw: string): GeneralValidationResult {
  const trimmed = raw.trim();
  if (!trimmed) {
    return { valid: true, empty: true, error: null, count: 0 };
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch (error) {
    return {
      valid: false,
      empty: false,
      error:
        error instanceof Error ? error.message : "Invalid JSON syntax.",
      count: 0,
    };
  }

  if (!Array.isArray(parsed)) {
    return {
      valid: false,
      empty: false,
      error: "Must be a JSON array of { key, value } objects.",
      count: 0,
    };
  }

  for (let i = 0; i < parsed.length; i += 1) {
    const item = parsed[i];
    if (!item || typeof item !== "object" || Array.isArray(item)) {
      return {
        valid: false,
        empty: false,
        error: `Item ${i} must be an object with "key" and "value".`,
        count: 0,
      };
    }

    const record = item as Record<string, unknown>;
    const key = record.key;
    const value = record.value;
    const keys = Object.keys(record);
    if (!keys.every((k) => k === "key" || k === "value")) {
      return {
        valid: false,
        empty: false,
        error: `Item ${i} has unsupported properties. Use only "key" and "value".`,
        count: 0,
      };
    }
    if (typeof key !== "string" || key.trim().length === 0) {
      return {
        valid: false,
        empty: false,
        error: `Item ${i} has an empty or invalid "key".`,
        count: 0,
      };
    }
    if (typeof value !== "string" || value.trim().length === 0) {
      return {
        valid: false,
        empty: false,
        error: `Item ${i} has an empty or invalid "value".`,
        count: 0,
      };
    }
  }

  return { valid: true, empty: parsed.length === 0, error: null, count: parsed.length };
}

function normalizeTerms(value: string): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const line of value.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || seen.has(trimmed)) continue;
    seen.add(trimmed);
    out.push(trimmed);
  }
  return out;
}

function computeContextSize(
  generalJson: string,
  contextText: string,
  termsText: string,
): number {
  const generalValidation = parseGeneralJson(generalJson);
  if (!generalValidation.valid) {
    return 0;
  }

  const parsedGeneral =
    generalValidation.empty ? [] : (JSON.parse(generalJson.trim()) as unknown[]);
  const payload = {
    ...(parsedGeneral.length > 0 ? { general: parsedGeneral } : {}),
    ...(contextText.trim() ? { text: contextText.trim() } : {}),
    ...(normalizeTerms(termsText).length > 0
      ? { terms: normalizeTerms(termsText) }
      : {}),
  };

  if (Object.keys(payload).length === 0) {
    return 0;
  }

  return JSON.stringify(payload).length;
}

function statusClassName(status: "empty" | "valid" | "invalid"): string {
  if (status === "valid") return "text-green-400 border-green-500/40";
  if (status === "invalid") return "text-red-400 border-red-500/40";
  return "text-mid-gray border-mid-gray/40";
}

export const SonioxContextEditor: React.FC<SonioxContextEditorProps> = ({
  generalJson,
  text,
  terms,
  disabled = false,
  onCommitGeneralJson,
  onCommitText,
  onCommitTerms,
  onDraftGeneralJsonChange,
  onDraftTextChange,
  onDraftTermsChange,
}) => {
  const { t } = useTranslation();
  const [generalDraft, setGeneralDraft] = useState(generalJson || "");
  const [textDraft, setTextDraft] = useState(text || "");
  const [termsDraft, setTermsDraft] = useState((terms || []).join("\n"));
  const [generalError, setGeneralError] = useState<string | null>(null);
  const [textError, setTextError] = useState<string | null>(null);
  const [termsError, setTermsError] = useState<string | null>(null);
  const [openGeneral, setOpenGeneral] = useState(false);
  const [openText, setOpenText] = useState(false);
  const [openTerms, setOpenTerms] = useState(false);

  useEffect(() => {
    setGeneralDraft(generalJson || "");
  }, [generalJson]);
  useEffect(() => {
    setTextDraft(text || "");
  }, [text]);
  useEffect(() => {
    setTermsDraft((terms || []).join("\n"));
  }, [terms]);

  const generalValidation = useMemo(
    () => parseGeneralJson(generalDraft),
    [generalDraft],
  );
  const normalizedTerms = useMemo(() => normalizeTerms(termsDraft), [termsDraft]);
  const contextSize = useMemo(
    () => computeContextSize(generalDraft, textDraft, termsDraft),
    [generalDraft, textDraft, termsDraft],
  );
  const contextSizeError =
    contextSize > SONIOX_CONTEXT_MAX_CHARS
      ? t(
          "settings.transcriptionProfiles.sonioxContext.tooLarge",
          "Soniox context is too large. Keep total context under 10,000 characters.",
        )
      : null;

  const generalStatus: "empty" | "valid" | "invalid" = generalValidation.empty
    ? "empty"
    : generalValidation.valid
      ? "valid"
      : "invalid";
  const textStatus: "empty" | "valid" = textDraft.trim() ? "valid" : "empty";
  const termsStatus: "empty" | "valid" = normalizedTerms.length > 0 ? "valid" : "empty";

  useEffect(() => {
    if (!generalValidation.valid && !generalValidation.empty) {
      setOpenGeneral(true);
    }
  }, [generalValidation.valid, generalValidation.empty]);

  const commitGeneral = async () => {
    if (disabled) return;
    setGeneralError(null);
    if (!generalValidation.valid) {
      setGeneralError(
        generalValidation.error ||
          t(
            "settings.transcriptionProfiles.sonioxContext.generalInvalid",
            "Invalid JSON format.",
          ),
      );
      return;
    }
    if (contextSizeError) {
      setGeneralError(contextSizeError);
      return;
    }
    const next = generalDraft.trim();
    if (next === (generalJson || "").trim()) return;
    try {
      await onCommitGeneralJson(next);
    } catch (error) {
      setGeneralError(
        error instanceof Error
          ? error.message
          : t(
              "settings.transcriptionProfiles.sonioxContext.saveFailed",
              "Failed to save value.",
            ),
      );
    }
  };

  const commitText = async () => {
    if (disabled) return;
    setTextError(null);
    if (contextSizeError) {
      setTextError(contextSizeError);
      return;
    }
    const next = textDraft.trim();
    if (next === (text || "").trim()) return;
    try {
      await onCommitText(next);
    } catch (error) {
      setTextError(
        error instanceof Error
          ? error.message
          : t(
              "settings.transcriptionProfiles.sonioxContext.saveFailed",
              "Failed to save value.",
            ),
      );
    }
  };

  const commitTerms = async () => {
    if (disabled) return;
    setTermsError(null);
    if (contextSizeError) {
      setTermsError(contextSizeError);
      return;
    }
    const previous = normalizeTerms((terms || []).join("\n"));
    if (
      previous.length === normalizedTerms.length &&
      previous.every((value, idx) => value === normalizedTerms[idx])
    ) {
      return;
    }
    try {
      await onCommitTerms(normalizedTerms);
    } catch (error) {
      setTermsError(
        error instanceof Error
          ? error.message
          : t(
              "settings.transcriptionProfiles.sonioxContext.saveFailed",
              "Failed to save value.",
            ),
      );
    }
  };

  return (
    <div className="space-y-2 pt-2 border-t border-mid-gray/10">
      <div className="flex items-baseline justify-between gap-2">
        <div className="text-xs text-mid-gray">
          {t(
            "settings.transcriptionProfiles.sonioxContext.description",
            "Soniox context improves recognition for domain terms and names. Fields stay saved when you switch providers.",
          )}
        </div>
        <a
          href="https://soniox.com/docs/stt/concepts/context"
          target="_blank"
          rel="noreferrer"
          className="text-xs text-accent hover:underline whitespace-nowrap shrink-0"
        >
          {t("settings.transcriptionProfiles.sonioxContext.docsLink", "Context docs")}
        </a>
      </div>
      {contextSize > 0 && (
        <div
          className={`text-xs ${contextSizeError ? "text-red-400" : "text-mid-gray"}`}
        >
          {contextSize} / {SONIOX_CONTEXT_MAX_CHARS}
          <span className="ml-2 text-mid-gray/60">
            {t("settings.transcriptionProfiles.sonioxContext.sizeLimit", "Maximum ~8,000 tokens (~10,000 characters). If exceeded, the API returns an error — trim or summarize first.")}
          </span>
        </div>
      )}

      <div className="rounded border border-mid-gray/20">
        <button
          type="button"
          onClick={() => setOpenGeneral((prev) => !prev)}
          className="w-full flex items-center justify-between px-3 py-2 text-left hover:bg-mid-gray/10"
        >
          <div className="flex items-center gap-2">
            <span className="text-xs font-semibold text-text/70">
              {t("settings.transcriptionProfiles.sonioxContext.generalTitle", "Context General (JSON)")}
            </span>
            <span
              className={`text-[10px] px-1.5 py-0.5 rounded border ${statusClassName(generalStatus)}`}
            >
              {generalStatus === "empty"
                ? t("settings.transcriptionProfiles.sonioxContext.empty", "Empty")
                : generalStatus === "valid"
                  ? t("settings.transcriptionProfiles.sonioxContext.valid", "Valid")
                  : t("settings.transcriptionProfiles.sonioxContext.invalid", "Invalid")}
            </span>
            {!generalValidation.empty && generalValidation.valid && (
              <span className="text-[10px] text-mid-gray">
                {generalValidation.count} {t("settings.transcriptionProfiles.sonioxContext.items", "items")}
              </span>
            )}
          </div>
          {openGeneral ? (
            <ChevronUp className="w-4 h-4 text-mid-gray" />
          ) : (
            <ChevronDown className="w-4 h-4 text-mid-gray" />
          )}
        </button>
        {openGeneral && (
          <div className="px-3 pb-3 space-y-2 border-t border-mid-gray/20">
            <textarea
              value={generalDraft}
              onChange={(e) => {
                const nextValue = e.target.value;
                setGeneralDraft(nextValue);
                onDraftGeneralJsonChange?.(nextValue);
              }}
              onBlur={commitGeneral}
              rows={6}
              disabled={disabled}
              placeholder={`[\n  { "key": "domain", "value": "Healthcare" },\n  { "key": "topic", "value": "Diabetes consultation" }\n]`}
              className="mt-2 w-full px-3 py-2 text-sm bg-[#1e1e1e]/80 border border-[#3c3c3c] rounded-md resize-y text-[#e8e8e8] placeholder-[#6b6b6b]"
            />
            {(generalError || (!generalValidation.valid && !generalValidation.empty)) && (
              <p className="text-xs text-red-400">
                {generalError || generalValidation.error}
              </p>
            )}
            <TellMeMore
              title={t(
                "settings.transcriptionProfiles.sonioxContext.helpGeneralTitle",
                "General context — docs & AI prompt",
              )}
            >
              <p className="text-xs text-text/80">
                {t(
                  "settings.transcriptionProfiles.sonioxContext.helpGeneralDocIntro",
                  "General information provides baseline context that guides the AI model. It helps adapt vocabulary to the correct domain, improving transcription and translation quality.",
                )}
              </p>
              <p className="mt-1 text-xs text-text/70">
                {t(
                  "settings.transcriptionProfiles.sonioxContext.helpGeneralDocContent",
                  "Structured key-value pairs describing conversation domain, topic, intent, and other relevant metadata such as participants' names, organization, setting, location, etc.",
                )}
              </p>
              <p className="mt-1 text-xs text-amber-400/80">
                {t(
                  "settings.transcriptionProfiles.sonioxContext.helpGeneralDocTip",
                  "Tip: keep it short — ideally no more than 10 key-value pairs.",
                )}
              </p>
              <pre className="mt-2 text-[11px] bg-mid-gray/20 p-2 rounded overflow-x-auto">
{`[
  { "key": "domain", "value": "Healthcare" },
  { "key": "topic", "value": "Diabetes management consultation" },
  { "key": "doctor", "value": "Dr. Martha Smith" },
  { "key": "patient", "value": "Mr. David Miller" },
  { "key": "organization", "value": "St John's Hospital" }
]`}
              </pre>
              <p className="mt-2 text-xs text-text/60">
                {t(
                  "settings.transcriptionProfiles.sonioxContext.helpGeneralAskAi",
                  "Ask AI: Generate Soniox context.general JSON array for a customer support call. Return only JSON with key/value pairs.",
                )}
              </p>
            </TellMeMore>
          </div>
        )}
      </div>

      <div className="rounded border border-mid-gray/20">
        <button
          type="button"
          onClick={() => setOpenText((prev) => !prev)}
          className="w-full flex items-center justify-between px-3 py-2 text-left hover:bg-mid-gray/10"
        >
          <div className="flex items-center gap-2">
            <span className="text-xs font-semibold text-text/70">
              {t("settings.transcriptionProfiles.sonioxContext.textTitle", "Context Text")}
            </span>
            <span
              className={`text-[10px] px-1.5 py-0.5 rounded border ${statusClassName(textStatus)}`}
            >
              {textStatus === "empty"
                ? t("settings.transcriptionProfiles.sonioxContext.empty", "Empty")
                : t("settings.transcriptionProfiles.sonioxContext.ready", "Ready")}
            </span>
            {textDraft.trim() && (
              <span className="text-[10px] text-mid-gray">
                {textDraft.trim().length} {t("settings.transcriptionProfiles.sonioxContext.chars", "chars")}
              </span>
            )}
          </div>
          {openText ? (
            <ChevronUp className="w-4 h-4 text-mid-gray" />
          ) : (
            <ChevronDown className="w-4 h-4 text-mid-gray" />
          )}
        </button>
        {openText && (
          <div className="px-3 pb-3 space-y-2 border-t border-mid-gray/20">
            <textarea
              value={textDraft}
              onChange={(e) => {
                const nextValue = e.target.value;
                setTextDraft(nextValue);
                onDraftTextChange?.(nextValue);
              }}
              onBlur={commitText}
              rows={4}
              disabled={disabled}
              placeholder={t(
                "settings.transcriptionProfiles.sonioxContext.textPlaceholder",
                "Background notes, names, prior context, product details, etc.",
              )}
              className="mt-2 w-full px-3 py-2 text-sm bg-[#1e1e1e]/80 border border-[#3c3c3c] rounded-md resize-y text-[#e8e8e8] placeholder-[#6b6b6b]"
            />
            {textError && <p className="text-xs text-red-400">{textError}</p>}
            <TellMeMore
              title={t(
                "settings.transcriptionProfiles.sonioxContext.helpTextTitle",
                "Context Text — what to put here",
              )}
            >
              <p className="text-xs text-text/80">
                {t(
                  "settings.transcriptionProfiles.sonioxContext.helpText",
                  "Add short background notes, participant names, product names, or meeting context that may appear in speech.",
                )}
              </p>
              <p className="mt-1 text-xs text-text/70">
                {t(
                  "settings.transcriptionProfiles.sonioxContext.helpTextDocIntro",
                  "Provide longer unstructured text that expands on general information. Examples:",
                )}
              </p>
              <ul className="mt-1 list-disc pl-4 text-xs text-text/70 space-y-0.5">
                <li>{t("settings.transcriptionProfiles.sonioxContext.helpTextDocItem1", "History of prior interactions with a customer.")}</li>
                <li>{t("settings.transcriptionProfiles.sonioxContext.helpTextDocItem2", "Reference documents or background summaries.")}</li>
                <li>{t("settings.transcriptionProfiles.sonioxContext.helpTextDocItem3", "Meeting notes or prior conversation context.")}</li>
              </ul>
            </TellMeMore>
          </div>
        )}
      </div>

      <div className="rounded border border-mid-gray/20">
        <button
          type="button"
          onClick={() => setOpenTerms((prev) => !prev)}
          className="w-full flex items-center justify-between px-3 py-2 text-left hover:bg-mid-gray/10"
        >
          <div className="flex items-center gap-2">
            <span className="text-xs font-semibold text-text/70">
              {t("settings.transcriptionProfiles.sonioxContext.termsTitle", "Context Terms")}
            </span>
            <span
              className={`text-[10px] px-1.5 py-0.5 rounded border ${statusClassName(termsStatus)}`}
            >
              {termsStatus === "empty"
                ? t("settings.transcriptionProfiles.sonioxContext.empty", "Empty")
                : t("settings.transcriptionProfiles.sonioxContext.ready", "Ready")}
            </span>
            {normalizedTerms.length > 0 && (
              <span className="text-[10px] text-mid-gray">
                {normalizedTerms.length} {t("settings.transcriptionProfiles.sonioxContext.terms", "terms")}
              </span>
            )}
          </div>
          {openTerms ? (
            <ChevronUp className="w-4 h-4 text-mid-gray" />
          ) : (
            <ChevronDown className="w-4 h-4 text-mid-gray" />
          )}
        </button>
        {openTerms && (
          <div className="px-3 pb-3 space-y-2 border-t border-mid-gray/20">
            <textarea
              value={termsDraft}
              onChange={(e) => {
                const nextValue = e.target.value;
                setTermsDraft(nextValue);
                onDraftTermsChange?.(normalizeTerms(nextValue));
              }}
              onBlur={commitTerms}
              rows={5}
              disabled={disabled}
              placeholder={`Celebrex\nPrilosec\nAcme Cloud Pro`}
              className="mt-2 w-full px-3 py-2 text-sm bg-[#1e1e1e]/80 border border-[#3c3c3c] rounded-md resize-y text-[#e8e8e8] placeholder-[#6b6b6b]"
            />
            <p className="text-xs text-mid-gray">
              {t(
                "settings.transcriptionProfiles.sonioxContext.termsHint",
                "One term per line. Empty lines are ignored and duplicates are removed.",
              )}
            </p>
            {termsError && <p className="text-xs text-red-400">{termsError}</p>}
            <TellMeMore
              title={t(
                "settings.transcriptionProfiles.sonioxContext.helpTermsTitle",
                "Transcription Terms — docs & examples",
              )}
            >
              <p className="text-xs text-text/80">
                {t(
                  "settings.transcriptionProfiles.sonioxContext.helpTermsDocIntro",
                  "Improve transcription accuracy of important or uncommon words and phrases that you expect in the audio:",
                )}
              </p>
              <ul className="mt-1 list-disc pl-4 text-xs text-text/70 space-y-0.5">
                <li>{t("settings.transcriptionProfiles.sonioxContext.helpTermsDocItem1", "Domain or industry-specific terminology.")}</li>
                <li>{t("settings.transcriptionProfiles.sonioxContext.helpTermsDocItem2", "Brand or product names.")}</li>
                <li>{t("settings.transcriptionProfiles.sonioxContext.helpTermsDocItem3", "Rare, uncommon, or invented words.")}</li>
              </ul>
              <pre className="mt-2 text-[11px] bg-mid-gray/20 p-2 rounded overflow-x-auto">
{`Celebrex
Prilosec
Amoxicillin Clavulanate Potassium
Acme Cloud Pro`}
              </pre>
            </TellMeMore>
          </div>
        )}
      </div>
    </div>
  );
};
