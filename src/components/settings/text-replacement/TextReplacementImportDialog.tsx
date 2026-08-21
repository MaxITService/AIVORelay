import React, { useEffect, useId, useRef } from "react";
import { Loader2, Upload, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/Button";
import { ToggleSwitch } from "@/components/ui/ToggleSwitch";
import type { TextReplacementImportMode } from "./textReplacementRuleTransfer";

interface TextReplacementImportDialogProps {
  isOpen: boolean;
  importedRuleCount: number;
  currentRuleCount: number;
  mode: TextReplacementImportMode;
  overwriteConflicts: boolean;
  isApplying: boolean;
  onModeChange: (mode: TextReplacementImportMode) => void;
  onOverwriteConflictsChange: (checked: boolean) => void;
  onCancel: () => void;
  onConfirm: () => void;
}

const FOCUSABLE_SELECTOR =
  'button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';

const getFocusableElements = (container: HTMLElement): HTMLElement[] =>
  Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR));

export const TextReplacementImportDialog = ({
  isOpen,
  importedRuleCount,
  currentRuleCount,
  mode,
  overwriteConflicts,
  isApplying,
  onModeChange,
  onOverwriteConflictsChange,
  onCancel,
  onConfirm,
}: TextReplacementImportDialogProps) => {
  const { t } = useTranslation();
  const dialogRef = useRef<HTMLDivElement>(null);
  const onCancelRef = useRef(onCancel);
  const isApplyingRef = useRef(isApplying);
  const titleId = useId();
  const descriptionId = useId();
  const modeLabelId = useId();

  useEffect(() => {
    onCancelRef.current = onCancel;
  }, [onCancel]);

  useEffect(() => {
    isApplyingRef.current = isApplying;
  }, [isApplying]);

  useEffect(() => {
    if (!isOpen) return;

    const previousFocus =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    const focusTimer = window.setTimeout(() => {
      const initialFocus = dialogRef.current?.querySelector<HTMLElement>(
        "[data-import-dialog-initial-focus]",
      );
      const firstFocusable = dialogRef.current
        ? getFocusableElements(dialogRef.current)[0]
        : null;
      (initialFocus ?? firstFocusable)?.focus();
    }, 0);

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        if (!isApplyingRef.current) onCancelRef.current();
        return;
      }
      if (event.key !== "Tab" || !dialogRef.current) return;

      const focusableElements = getFocusableElements(dialogRef.current);
      if (focusableElements.length === 0) {
        event.preventDefault();
        dialogRef.current.focus();
        return;
      }

      const first = focusableElements[0];
      const last = focusableElements[focusableElements.length - 1];
      if (!dialogRef.current.contains(document.activeElement)) {
        event.preventDefault();
        (event.shiftKey ? last : first).focus();
      } else if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.clearTimeout(focusTimer);
      window.removeEventListener("keydown", handleKeyDown);
      previousFocus?.focus();
    };
  }, [isOpen]);

  if (!isOpen) return null;

  const closeDialog = () => {
    if (!isApplying) onCancel();
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) closeDialog();
      }}
    >
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-sm"
        onMouseDown={closeDialog}
        aria-hidden="true"
      />
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-busy={isApplying}
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
        tabIndex={-1}
        className="relative z-10 max-h-[calc(100vh-2rem)] w-full max-w-lg overflow-y-auto rounded-lg border border-[#3c3c3c] bg-[#151515] shadow-2xl"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="flex items-start justify-between gap-3 border-b border-white/[0.06] p-4">
          <div className="min-w-0">
            <h2 id={titleId} className="text-base font-semibold text-[#f5f5f5]">
              {t("textReplacement.importDialogTitle", "Import replacement rules")}
            </h2>
            <p
              id={descriptionId}
              className="mt-1 text-sm leading-relaxed text-[#a0a0a0]"
            >
              {t(
                "textReplacement.importDialogDescription",
                "Choose whether to replace the current rules or merge the imported rules with them.",
              )}
            </p>
          </div>
          <button
            type="button"
            onClick={closeDialog}
            disabled={isApplying}
            aria-label={t("textReplacement.importDialogCancel", "Cancel")}
            title={t("textReplacement.importDialogCancel", "Cancel")}
            className="shrink-0 rounded-md p-1 text-[#808080] transition-colors hover:bg-white/[0.06] hover:text-[#f5f5f5] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#9b5de5]/60 disabled:cursor-not-allowed"
          >
            <X className="h-4 w-4" aria-hidden="true" />
          </button>
        </div>

        <div className="space-y-4 p-4">
          <p className="text-xs text-[#8f8f8f]">
            {t("textReplacement.importDialogRuleCounts", {
              imported: importedRuleCount,
              current: currentRuleCount,
            })}
          </p>

          <div className="space-y-2">
            <span
              id={modeLabelId}
              className="block text-xs font-medium text-[#8f8f8f]"
            >
              {t("textReplacement.importDialogMode", "Import strategy")}
            </span>
            <div
              role="group"
              aria-labelledby={modeLabelId}
              className="grid grid-cols-2 overflow-hidden rounded-md border border-[#3c3c3c] bg-[#181818]"
            >
              <button
                type="button"
                aria-pressed={mode === "replace"}
                disabled={isApplying}
                onClick={() => onModeChange("replace")}
                className={`min-h-9 px-3 py-2 text-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[#9b5de5]/70 disabled:cursor-not-allowed disabled:opacity-50 ${
                  mode === "replace"
                    ? "bg-[#9b5de5]/20 text-[#d0b2ff]"
                    : "text-[#a0a0a0] hover:bg-white/[0.04] hover:text-[#e0e0e0]"
                }`}
              >
                {t("textReplacement.importDialogReplace", "Replace current")}
              </button>
              <button
                type="button"
                data-import-dialog-initial-focus="true"
                aria-pressed={mode === "merge"}
                disabled={isApplying}
                onClick={() => onModeChange("merge")}
                className={`min-h-9 border-l border-[#3c3c3c] px-3 py-2 text-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[#9b5de5]/70 disabled:cursor-not-allowed disabled:opacity-50 ${
                  mode === "merge"
                    ? "bg-[#9b5de5]/20 text-[#d0b2ff]"
                    : "text-[#a0a0a0] hover:bg-white/[0.04] hover:text-[#e0e0e0]"
                }`}
              >
                {t("textReplacement.importDialogMerge", "Merge")}
              </button>
            </div>
          </div>

          <p className="text-sm leading-relaxed text-[#a0a0a0]">
            {mode === "replace"
              ? t(
                  "textReplacement.importDialogReplaceDescription",
                  "Remove the current rules and use the imported file in its order.",
                )
              : t(
                  "textReplacement.importDialogMergeDescription",
                  "Keep the current rules and append imported rules that do not conflict.",
                )}
          </p>

          {mode === "merge" && (
            <ToggleSwitch
              checked={overwriteConflicts}
              onChange={onOverwriteConflictsChange}
              disabled={isApplying}
              isUpdating={isApplying}
              label={t(
                "textReplacement.importDialogOverwriteConflicts",
                "Overwrite conflicts",
              )}
              description={t(
                "textReplacement.importDialogOverwriteDescription",
                "A conflict means the Find text, case-sensitivity, and regex mode match; the imported replacement can replace that rule.",
              )}
              descriptionMode="inline"
              grouped
              ariaLabel={t(
                "textReplacement.importDialogOverwriteConflicts",
                "Overwrite conflicts",
              )}
            />
          )}
        </div>

        <div className="flex flex-wrap justify-end gap-2 border-t border-white/[0.06] p-4">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={closeDialog}
            disabled={isApplying}
            className="focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#9b5de5]/60"
          >
            {t("textReplacement.importDialogCancel", "Cancel")}
          </Button>
          <Button
            type="button"
            variant="primary"
            size="sm"
            onClick={onConfirm}
            disabled={isApplying}
            className="inline-flex min-w-0 items-center justify-center gap-1.5 whitespace-nowrap focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#9b5de5]/60"
          >
            {isApplying ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" aria-hidden="true" />
            ) : (
              <Upload className="h-3.5 w-3.5" aria-hidden="true" />
            )}
            {t("textReplacement.importDialogConfirm", "Import rules")}
          </Button>
        </div>
      </div>
    </div>
  );
};
