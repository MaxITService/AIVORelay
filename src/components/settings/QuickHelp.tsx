import React from "react";
import { useTranslation } from "react-i18next";
import type { SidebarSection } from "../Sidebar";
import { useNavigationStore } from "../../stores/navigationStore";

interface QuickHelpProps {
  activeSection: SidebarSection;
}

const QUICK_HELP: Partial<
  Record<SidebarSection, { copyKey: string; anchor: string }>
> = {
  general: {
    copyKey: "quickHelp.general",
    anchor: "help-transcription",
  },
  models: {
    copyKey: "quickHelp.models",
    anchor: "help-models",
  },
  advanced: {
    copyKey: "quickHelp.advanced",
    anchor: "help-advanced",
  },
  postprocessing: {
    copyKey: "quickHelp.postprocessing",
    anchor: "help-post-processing",
  },
  aiReplace: {
    copyKey: "quickHelp.aiReplace",
    anchor: "help-ai-replace",
  },
  voiceCommands: {
    copyKey: "quickHelp.voiceCommands",
    anchor: "help-voice-commands",
  },
  browserConnector: {
    copyKey: "quickHelp.browserConnector",
    anchor: "help-connector",
  },
  textReplacement: {
    copyKey: "quickHelp.textReplacement",
    anchor: "help-text-processing",
  },
  userInterface: {
    copyKey: "quickHelp.userInterface",
    anchor: "help-user-interface",
  },
  history: {
    copyKey: "quickHelp.history",
    anchor: "help-history",
  },
  audioProcessing: {
    copyKey: "quickHelp.audioProcessing",
    anchor: "help-speech-processing",
  },
  debug: {
    copyKey: "quickHelp.debug",
    anchor: "help-debug",
  },
  liveSoundTranscription: {
    copyKey: "quickHelp.liveSoundTranscription",
    anchor: "help-live-monitor",
  },
  transcribeFile: {
    copyKey: "quickHelp.transcribeFile",
    anchor: "help-transcribe-file",
  },
  textToSpeech: {
    copyKey: "quickHelp.textToSpeech",
    anchor: "help-speak-selected-text",
  },
  ttsFiles: {
    copyKey: "quickHelp.ttsFiles",
    anchor: "help-text-file-to-mp3",
  },
};

export const QuickHelp: React.FC<QuickHelpProps> = ({ activeSection }) => {
  const { t } = useTranslation();
  const openHelp = useNavigationStore((state) => state.openHelp);
  const help = QUICK_HELP[activeSection];

  if (!help) return null;

  return (
    <div className="flex w-full flex-wrap items-center justify-between gap-x-3 gap-y-1 rounded-lg border border-[#333333] bg-[#1a1a1a]/80 px-3 py-2">
      <p className="min-w-0 flex-1 text-xs leading-relaxed text-[#b8b8b8]">
        {t(help.copyKey)}
      </p>
      <a
        href={`#${help.anchor}`}
        onClick={(event) => {
          event.preventDefault();
          openHelp(help.anchor);
        }}
        className="shrink-0 rounded-md px-1 text-xs font-medium text-[#ff8ebb] underline decoration-[#ff8ebb]/50 underline-offset-2 transition-colors hover:text-[#ffd1e6] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#ff4d8d]/60"
      >
        {t("quickHelp.learnMore")}
      </a>
    </div>
  );
};
