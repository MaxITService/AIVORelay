import React, { useCallback } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink, FlaskConical } from "lucide-react";
import { useTranslation } from "react-i18next";

const TTS_ISSUES_URL = "https://github.com/MaxITService/AIVORelay/issues";

export const TtsBetaBanner: React.FC = () => {
  const { t } = useTranslation();

  const handleOpenIssues = useCallback(async () => {
    try {
      await openUrl(TTS_ISSUES_URL);
    } catch (error) {
      console.error("Failed to open the AivoRelay issues page:", error);
    }
  }, []);

  return (
    <div
      role="status"
      className="flex items-start gap-3 rounded-lg border border-violet-400/35 bg-violet-500/10 px-4 py-3 text-violet-100"
    >
      <FlaskConical
        aria-hidden="true"
        className="mt-0.5 h-5 w-5 shrink-0 text-violet-300"
      />
      <div className="min-w-0 space-y-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="rounded bg-violet-400/20 px-2 py-0.5 text-xs font-bold tracking-wider text-violet-100">
            {t("textToSpeech.beta.label")}
          </span>
          <p className="text-sm font-semibold">
            {t("textToSpeech.beta.title")}
          </p>
        </div>
        <p className="text-sm text-violet-100/85">
          {t("textToSpeech.beta.description")} {" "}
          <button
            type="button"
            onClick={() => void handleOpenIssues()}
            className="inline-flex items-center gap-1 font-medium text-violet-200 underline decoration-violet-300/60 underline-offset-2 transition-colors hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-violet-300/70"
          >
            {t("textToSpeech.beta.reportIssue")}
            <ExternalLink aria-hidden="true" className="h-3.5 w-3.5" />
          </button>
        </p>
      </div>
    </div>
  );
};
