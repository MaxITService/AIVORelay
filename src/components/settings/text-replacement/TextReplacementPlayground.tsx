import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/Button";
import { useWindowState } from "@/hooks/useWindowState";
import type { TextReplacementRule } from "./textReplacementRuleView";

interface PreviewResult {
  final_text: string;
  changed_rule_indices: number[];
  invalid_regex_indices: number[];
}

type DiffPart = { kind: "same" | "added" | "removed"; text: string };

const appendPart = (parts: DiffPart[], kind: DiffPart["kind"], text: string) => {
  const last = parts[parts.length - 1];
  if (last?.kind === kind) last.text += text;
  else parts.push({ kind, text });
};

const DIFF_CELL_BUDGET = 160_000;

const appendSequenceDiff = (parts: DiffPart[], oldUnits: string[], newUnits: string[]): boolean => {
  if (oldUnits.length * newUnits.length > DIFF_CELL_BUDGET) return false;
  const width = newUnits.length + 1;
  const lengths = new Uint32Array((oldUnits.length + 1) * width);
  for (let i = oldUnits.length - 1; i >= 0; i--) {
    for (let j = newUnits.length - 1; j >= 0; j--) {
      const cell = i * width + j;
      lengths[cell] = oldUnits[i] === newUnits[j]
        ? lengths[(i + 1) * width + j + 1] + 1
        : Math.max(lengths[(i + 1) * width + j], lengths[cell + 1]);
    }
  }
  let i = 0;
  let j = 0;
  while (i < oldUnits.length || j < newUnits.length) {
    if (i < oldUnits.length && j < newUnits.length && oldUnits[i] === newUnits[j]) {
      appendPart(parts, "same", oldUnits[i++]);
      j++;
    } else if (i < oldUnits.length && (j === newUnits.length || lengths[(i + 1) * width + j] >= lengths[i * width + j + 1])) {
      // On ties, show the removed text before its replacement.
      appendPart(parts, "removed", oldUnits[i++]);
    } else {
      appendPart(parts, "added", newUnits[j++]);
    }
  }
  return true;
};

// Compare characters for small changes, then words/spaces/punctuation for long spans.
const diffText = (before: string, after: string): DiffPart[] => {
  const oldChars = Array.from(before);
  const newChars = Array.from(after);
  let start = 0;
  while (start < oldChars.length && start < newChars.length && oldChars[start] === newChars[start]) start++;
  let oldEnd = oldChars.length;
  let newEnd = newChars.length;
  while (oldEnd > start && newEnd > start && oldChars[oldEnd - 1] === newChars[newEnd - 1]) {
    oldEnd--;
    newEnd--;
  }
  const parts: DiffPart[] = [];
  if (start) parts.push({ kind: "same", text: oldChars.slice(0, start).join("") });
  const oldMiddle = oldChars.slice(start, oldEnd);
  const newMiddle = newChars.slice(start, newEnd);
  if (oldMiddle.length && newMiddle.length && appendSequenceDiff(parts, oldMiddle, newMiddle)) {
    // Character detail is useful when the changed span is small.
  } else if (oldMiddle.length && newMiddle.length && appendSequenceDiff(
    parts,
    oldMiddle.join("").match(/\s+|[\p{L}\p{N}\p{M}]+|[^\s\p{L}\p{N}\p{M}]+/gu) ?? [],
    newMiddle.join("").match(/\s+|[\p{L}\p{N}\p{M}]+|[^\s\p{L}\p{N}\p{M}]+/gu) ?? [],
  )) {
    // Token alignment preserves unchanged text between distant edits.
  } else {
    if (oldMiddle.length) parts.push({ kind: "removed", text: oldMiddle.join("") });
    if (newMiddle.length) parts.push({ kind: "added", text: newMiddle.join("") });
  }
  if (oldEnd < oldChars.length) parts.push({ kind: "same", text: oldChars.slice(oldEnd).join("") });
  return parts;
};

export function TextReplacementPlayground({ rules, enabled, rulesBusy }: {
  rules: TextReplacementRule[];
  enabled: boolean;
  rulesBusy: boolean;
}) {
  const { t } = useTranslation();
  const [input, setInput] = useWindowState("textReplacement.playground.input", "");
  const [preview, setPreview] = useState<{ signature: string; value: PreviewResult } | null>(null);
  const [error, setError] = useState("");
  const [checking, setChecking] = useState(false);
  const requestId = useRef(0);
  const signature = useMemo(() => JSON.stringify([input, rules]), [input, rules]);
  const current = !rulesBusy && preview?.signature === signature ? preview.value : null;
  const parts = useMemo(() => current ? diffText(input, current.final_text) : [], [input, current]);

  useEffect(() => () => { requestId.current++; }, []);
  useEffect(() => {
    requestId.current++;
    setChecking(false);
    setError("");
    if (rulesBusy) setPreview(null);
  }, [signature, rulesBusy]);

  const check = async () => {
    const id = ++requestId.current;
    setPreview(null);
    setError("");
    if (new TextEncoder().encode(input).length > 32_768) {
      setChecking(false);
      setError(t("textReplacement.playgroundTooLong"));
      return;
    }
    setChecking(true);
    try {
      const value = await invoke<PreviewResult>("preview_text_replacements", {
        text: input,
        replacements: rules,
      });
      if (id === requestId.current) setPreview({ signature, value });
    } catch (cause) {
      if (id === requestId.current) setError(`${t("textReplacement.playgroundError")} ${String(cause)}`);
    } finally {
      if (id === requestId.current) setChecking(false);
    }
  };

  return (
    <div className="border-t border-white/[0.05] px-4 py-4 space-y-4">
      <div className="space-y-1">
        <h3 className="text-sm font-medium text-[#f5f5f5]">{t("textReplacement.playgroundTitle")}</h3>
        <p className="text-xs leading-relaxed text-[#a1a1aa]">{t("textReplacement.playgroundDescription")}</p>
        {!enabled && <p className="text-xs leading-relaxed text-[#e6b86b]">{t("textReplacement.playgroundDisabled")}</p>}
      </div>
      <div className="space-y-2">
        <label htmlFor="text-replacement-playground-input" className="block text-sm text-[#d4d4d8]">
          {t("textReplacement.playgroundInput")}
        </label>
        <textarea
          id="text-replacement-playground-input"
          value={input}
          onChange={(event) => { setInput(event.target.value); setError(""); }}
          rows={4}
          className="w-full resize-y rounded-md border border-[#383838] bg-[#1a1a1a] px-3 py-2 text-sm text-[#f5f5f5] outline-none focus:border-[#9b5de5]"
          placeholder={t("textReplacement.playgroundPlaceholder")}
        />
        <Button type="button" onClick={check} disabled={checking || rulesBusy || rules.length === 0}>
          {checking ? t("textReplacement.playgroundChecking") : t("textReplacement.playgroundCheck")}
        </Button>
      </div>
      {rules.length === 0 && <p className="text-sm text-[#a1a1aa]">{t("textReplacement.playgroundNoRules")}</p>}
      {error && <p role="alert" className="text-sm text-[#ff8f8f]">{error}</p>}
      <div aria-live="polite" aria-busy={checking}>
        {current && (
          <div className="space-y-4">
            <div className="space-y-2">
              <h4 className="text-sm font-medium text-[#d4d4d8]">{t("textReplacement.playgroundResult")}</h4>
              <div className="rounded-md border border-[#383838] bg-[#1a1a1a] px-3 py-3 font-mono text-sm text-[#f5f5f5] whitespace-pre-wrap break-words min-h-12">
                {parts.filter((part) => part.kind !== "removed").map((part, index) => part.kind === "added"
                  ? <mark key={index} className="rounded bg-[#327a54]/55 text-[#e7ffef]">{part.text}</mark>
                  : <span key={index}>{part.text}</span>)}
              </div>
              {current.final_text !== input && (
                <details className="group space-y-2">
                  <summary className="cursor-pointer text-sm text-[#b69ad6] hover:text-[#d5b8f4]">
                    {t("textReplacement.playgroundShowChanges")}
                  </summary>
                  <p className="pt-2 text-xs text-[#a1a1aa]">{t("textReplacement.playgroundChanges")}</p>
                  <div className="rounded-md border border-[#383838] bg-[#1a1a1a] px-3 py-3 font-mono text-sm whitespace-pre-wrap break-words text-[#d4d4d8]">
                    {parts.map((part, index) => part.kind === "removed"
                      ? <del key={index} className="rounded bg-[#833d42]/45 text-[#ffc7ca]">{part.text}</del>
                      : part.kind === "added"
                        ? <ins key={index} className="rounded bg-[#327a54]/45 text-[#e7ffef] no-underline">{part.text}</ins>
                        : <span key={index}>{part.text}</span>)}
                  </div>
                </details>
              )}
            </div>
            <div className="space-y-2">
              <h4 className="text-sm font-medium text-[#d4d4d8]">{t("textReplacement.playgroundChangedRules")}</h4>
              {current.changed_rule_indices.length === 0
                ? <p className="text-sm text-[#a1a1aa]">{t("textReplacement.playgroundNoChanges")}</p>
                : <ol className="space-y-2">
                    {current.changed_rule_indices.map((index) => {
                      const rule = rules[index];
                      return rule && <li key={`${index}-${rule.id}`} className="rounded-md border border-[#383838] bg-[#1a1a1a] px-3 py-2 text-sm text-[#d4d4d8]">
                        <span className="mr-2 text-[#a1a1aa]">#{index + 1}</span>
                        <code className="whitespace-pre-wrap break-all">{rule.from}</code>
                        <span className="mx-2 text-[#a1a1aa]">→</span>
                        <code className="whitespace-pre-wrap break-all">{rule.to || t("textReplacement.emptyValue")}</code>
                      </li>;
                    })}
                  </ol>}
              {current.invalid_regex_indices.length > 0 && (
                <p className="text-sm text-[#e6b86b]">
                  {t("textReplacement.playgroundInvalidRegex", { rules: current.invalid_regex_indices.map((index) => `#${index + 1}`).join(", ") })}
                </p>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
