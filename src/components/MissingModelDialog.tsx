import React, { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { AlertTriangle } from "lucide-react";
import { Button } from "./ui/Button";

interface MissingModelDialogProps {
  missingModel: string;
  replacementModel: string;
  onConfirm: (alwaysSwitch: boolean) => void;
}

/**
 * Shown once after startup when the saved local model is unknown to this build
 * (typically after switching between a dev and a release build). The replacement
 * is already active; "OK" only acknowledges it and may remember the choice.
 */
export const MissingModelDialog: React.FC<MissingModelDialogProps> = ({
  missingModel,
  replacementModel,
  onConfirm,
}) => {
  const { t } = useTranslation();
  const titleId = useId();
  const descriptionId = useId();
  const [alwaysSwitch, setAlwaysSwitch] = useState(false);
  const dialogRef = useRef<HTMLDivElement>(null);

  // Restore focus to whatever had it before the dialog opened.
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    return () => previous?.focus?.();
  }, []);

  const handleKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === "Escape") {
      event.stopPropagation();
      onConfirm(alwaysSwitch);
      return;
    }
    if (event.key !== "Tab") return;
    const focusable = dialogRef.current?.querySelectorAll<HTMLElement>(
      "input, button",
    );
    if (!focusable || focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  };

  return (
    <div
      className="fixed inset-0 z-[100] flex items-center justify-center"
      onKeyDown={handleKeyDown}
    >
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" />
      <div
        ref={dialogRef}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
        className="relative z-10 w-full max-w-md mx-4 bg-gradient-to-br from-yellow-500/10 to-orange-500/5 border-2 border-yellow-500/50 rounded-xl shadow-2xl backdrop-blur-md"
      >
        <div className="p-6">
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 rounded-full bg-black/30 text-yellow-400">
              <AlertTriangle className="w-6 h-6" />
            </div>
            <h2 id={titleId} className="text-lg font-semibold text-yellow-300">
              {t("models.missingSelectionTitle", {
                defaultValue: "Model not available in this version",
              })}
            </h2>
          </div>
          <p
            id={descriptionId}
            className="text-text/80 text-sm leading-relaxed mb-5"
          >
            {t("models.missingSelectionDialogBody", {
              defaultValue:
                "The saved model {{model}} does not exist in this version. Switched to your last used model: {{replacement}}.",
              model: missingModel,
              replacement: replacementModel,
            })}
          </p>
          <label className="flex items-center gap-2 mb-5 text-sm text-text/90 cursor-pointer select-none">
            <input
              type="checkbox"
              className="w-4 h-4 accent-[#9b5de5]"
              checked={alwaysSwitch}
              onChange={(event) => setAlwaysSwitch(event.target.checked)}
            />
            {t("models.alwaysSwitch", {
              defaultValue: "Always switch automatically",
            })}
          </label>
          <Button
            autoFocus
            size="lg"
            className="w-full"
            onClick={() => onConfirm(alwaysSwitch)}
          >
            {t("common.ok", { defaultValue: "OK" })}
          </Button>
        </div>
      </div>
    </div>
  );
};
