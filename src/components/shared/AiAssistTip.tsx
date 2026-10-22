import React, { useCallback, useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, Copy, Sparkles } from "lucide-react";
import { Button } from "../ui/Button";

const AIVORELAY_SOURCE_URL = "https://github.com/MaxITService/AIVORelay";

interface AiAssistTipProps {
  className?: string;
}

// Shown in Help and on the first onboarding screen.
export const AiAssistTip: React.FC<AiAssistTipProps> = ({ className = "" }) => {
  const { t } = useTranslation();
  const titleId = useId();
  const [copyStatus, setCopyStatus] = useState<"idle" | "copied" | "error">(
    "idle",
  );
  const copyResetTimerRef = useRef<number | null>(null);

  useEffect(
    () => () => {
      if (copyResetTimerRef.current !== null) {
        window.clearTimeout(copyResetTimerRef.current);
      }
    },
    [],
  );

  const handleCopy = useCallback(async () => {
    const copyText = `${t("help.aiAssist.exampleIntro")}\n\n${AIVORELAY_SOURCE_URL}\n${t("help.aiAssist.questionLead")}\n\n`;

    try {
      await navigator.clipboard.writeText(copyText);
      setCopyStatus("copied");
    } catch (error) {
      console.error("Failed to copy the Help AI example:", error);
      setCopyStatus("error");
    }

    if (copyResetTimerRef.current !== null) {
      window.clearTimeout(copyResetTimerRef.current);
    }
    copyResetTimerRef.current = window.setTimeout(() => {
      setCopyStatus("idle");
      copyResetTimerRef.current = null;
    }, 2000);
  }, [t]);

  return (
    <section
      aria-labelledby={titleId}
      className={`rounded-xl border border-[#ff4d8d]/30 bg-[#1a1a1a] p-4 ${className}`}
    >
      <div className="flex min-w-0 items-start gap-3">
        <Sparkles
          aria-hidden="true"
          className="mt-0.5 h-4 w-4 shrink-0 text-[#ff8ebb]"
        />
        <div className="min-w-0">
          <p className="text-xs font-semibold uppercase tracking-[0.12em] text-[#ff8ebb]">
            {t("help.aiAssist.label")}
          </p>
          <h2
            id={titleId}
            className="mt-1 text-sm font-semibold text-[#f5f5f5]"
          >
            {t("help.aiAssist.title")}
          </h2>
          <p className="mt-1 text-xs leading-relaxed text-[#b8b8b8]">
            {t("help.aiAssist.description")}
          </p>
        </div>
      </div>

      <div className="mt-3 grid grid-cols-[minmax(0,1fr)_auto] overflow-hidden rounded-lg border border-[#363636] bg-[#111111]">
        <div className="min-w-0 px-3 py-3 font-mono text-xs leading-5">
          <p className="text-[#d8d8d8]">{t("help.aiAssist.exampleIntro")}</p>
          <a
            href={AIVORELAY_SOURCE_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="mt-1 block break-all text-[#ff8ebb] underline decoration-[#ff4d8d]/55 underline-offset-4 hover:text-[#ffc0d5] focus-visible:rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#ff4d8d]/60"
          >
            {AIVORELAY_SOURCE_URL}
          </a>
          <p className="mt-3 text-[#d8d8d8]">
            {t("help.aiAssist.questionLead")}
          </p>
          <p className="mt-3 font-semibold text-red-400">
            {t("help.aiAssist.questionPlaceholder")}
          </p>
        </div>
        <div className="flex items-center border-l border-[#333333] bg-[#151515] px-3">
          <Button
            type="button"
            variant="secondary"
            size="sm"
            onClick={() => void handleCopy()}
            className="inline-flex min-w-[6.75rem] shrink-0 items-center justify-center gap-1.5 whitespace-nowrap"
          >
            {copyStatus === "copied" ? (
              <Check
                aria-hidden="true"
                className="h-3.5 w-3.5 text-green-400"
              />
            ) : (
              <Copy aria-hidden="true" className="h-3.5 w-3.5" />
            )}
            {t(`help.aiAssist.${copyStatus === "idle" ? "copy" : copyStatus}`)}
          </Button>
        </div>
      </div>
      <p className="mt-2.5 max-w-[72ch] text-[11px] leading-4 text-[#969696]">
        {t("help.aiAssist.copyHint")}
      </p>
    </section>
  );
};
