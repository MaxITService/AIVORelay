import type { SidebarSection } from "../../Sidebar";

export interface HelpSubsectionDefinition {
  id: string;
  anchor: string;
  titleKey: string;
  summaryKey: string;
  warningKey?: string;
  destination: SidebarSection;
  destinationLabelKey: string;
}

export interface HelpSectionDefinition {
  id: string;
  anchor: string;
  titleKey: string;
  summaryKey: string;
  destination: SidebarSection;
  destinationLabelKey: string;
  subsections?: readonly HelpSubsectionDefinition[];
}

export const HELP_SECTIONS: readonly HelpSectionDefinition[] = [
  {
    id: "transcription",
    anchor: "help-transcription",
    titleKey: "help.sections.transcription.title",
    summaryKey: "help.sections.transcription.summary",
    destination: "general",
    destinationLabelKey: "sidebar.general",
    subsections: [
      {
        id: "models",
        anchor: "help-models",
        titleKey: "help.sections.transcription.models.title",
        summaryKey: "help.sections.transcription.models.summary",
        warningKey: "help.sections.transcription.models.warning",
        destination: "models",
        destinationLabelKey: "sidebar.models",
      },
      {
        id: "speechProcessing",
        anchor: "help-speech-processing",
        titleKey: "help.sections.transcription.speechProcessing.title",
        summaryKey: "help.sections.transcription.speechProcessing.summary",
        destination: "audioProcessing",
        destinationLabelKey: "sidebar.audioProcessing",
      },
      {
        id: "advanced",
        anchor: "help-advanced",
        titleKey: "help.sections.transcription.advanced.title",
        summaryKey: "help.sections.transcription.advanced.summary",
        destination: "advanced",
        destinationLabelKey: "sidebar.advanced",
      },
    ],
  },
  {
    id: "postProcessing",
    anchor: "help-post-processing",
    titleKey: "help.sections.postProcessing.title",
    summaryKey: "help.sections.postProcessing.summary",
    destination: "postprocessing",
    destinationLabelKey: "sidebar.postProcessing",
  },
  {
    id: "aiReplace",
    anchor: "help-ai-replace",
    titleKey: "help.sections.aiReplace.title",
    summaryKey: "help.sections.aiReplace.summary",
    destination: "aiReplace",
    destinationLabelKey: "sidebar.aiReplace",
  },
  {
    id: "transcribeFile",
    anchor: "help-transcribe-file",
    titleKey: "help.sections.transcribeFile.title",
    summaryKey: "help.sections.transcribeFile.summary",
    destination: "transcribeFile",
    destinationLabelKey: "sidebar.transcribeFile",
  },
  {
    id: "liveMonitor",
    anchor: "help-live-monitor",
    titleKey: "help.sections.liveMonitor.title",
    summaryKey: "help.sections.liveMonitor.summary",
    destination: "liveSoundTranscription",
    destinationLabelKey: "sidebar.liveSoundTranscription",
  },
  {
    id: "speakSelectedText",
    anchor: "help-speak-selected-text",
    titleKey: "help.sections.speakSelectedText.title",
    summaryKey: "help.sections.speakSelectedText.summary",
    destination: "textToSpeech",
    destinationLabelKey: "sidebar.textToSpeech",
  },
  {
    id: "textFileToMp3",
    anchor: "help-text-file-to-mp3",
    titleKey: "help.sections.textFileToMp3.title",
    summaryKey: "help.sections.textFileToMp3.summary",
    destination: "ttsFiles",
    destinationLabelKey: "sidebar.ttsFiles",
  },
  {
    id: "voiceCommands",
    anchor: "help-voice-commands",
    titleKey: "help.sections.voiceCommands.title",
    summaryKey: "help.sections.voiceCommands.summary",
    destination: "debug",
    destinationLabelKey: "sidebar.debug",
  },
  {
    id: "connector",
    anchor: "help-connector",
    titleKey: "help.sections.connector.title",
    summaryKey: "help.sections.connector.summary",
    destination: "browserConnector",
    destinationLabelKey: "sidebar.browserConnector",
  },
  {
    id: "textProcessing",
    anchor: "help-text-processing",
    titleKey: "help.sections.textProcessing.title",
    summaryKey: "help.sections.textProcessing.summary",
    destination: "textReplacement",
    destinationLabelKey: "sidebar.textReplacement",
  },
  {
    id: "history",
    anchor: "help-history",
    titleKey: "help.sections.history.title",
    summaryKey: "help.sections.history.summary",
    destination: "history",
    destinationLabelKey: "sidebar.history",
  },
  {
    id: "userInterface",
    anchor: "help-user-interface",
    titleKey: "help.sections.userInterface.title",
    summaryKey: "help.sections.userInterface.summary",
    destination: "userInterface",
    destinationLabelKey: "sidebar.userInterface",
  },
  {
    id: "debug",
    anchor: "help-debug",
    titleKey: "help.sections.debug.title",
    summaryKey: "help.sections.debug.summary",
    destination: "debug",
    destinationLabelKey: "sidebar.debug",
  },
] as const;
