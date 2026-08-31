import { aboutSearchEntries } from "./about/searchEntries";
import { advancedSearchEntries } from "./advanced/searchEntries";
import { aiReplaceSearchEntries } from "./ai-replace/searchEntries";
import { audioProcessingSearchEntries } from "./audio-processing/searchEntries";
import { browserConnectorSearchEntries } from "./browser-connector/searchEntries";
import { debugSearchEntries } from "./debug/searchEntries";
import { generalSearchEntries } from "./general/searchEntries";
import { helpSearchEntries } from "./help/searchEntries";
import { historySearchEntries } from "./history/searchEntries";
import { liveSoundSearchEntries } from "./live-sound-transcription/searchEntries";
import { modelsSearchEntries } from "./models/searchEntries";
import { postProcessingSearchEntries } from "./post-processing/searchEntries";
import { sendSelectedTextSearchEntries } from "./send-selected-text/searchEntries";
import { textReplacementSearchEntries } from "./text-replacement/searchEntries";
import { textToSpeechSearchEntries } from "./text-to-speech/searchEntries";
import { transcribeFileSearchEntries } from "./transcribe-file/searchEntries";
import { userInterfaceSearchEntries } from "./user-interface/searchEntries";
import { voiceCommandsSearchEntries } from "./voice-commands/searchEntries";
import type { SettingsSearchEntry } from "./settingsSearchTypes";

export const SETTINGS_SEARCH_ENTRIES = [
  ...generalSearchEntries,
  ...modelsSearchEntries,
  ...advancedSearchEntries,
  ...postProcessingSearchEntries,
  ...aiReplaceSearchEntries,
  ...sendSelectedTextSearchEntries,
  ...voiceCommandsSearchEntries,
  ...browserConnectorSearchEntries,
  ...textReplacementSearchEntries,
  ...userInterfaceSearchEntries,
  ...historySearchEntries,
  ...audioProcessingSearchEntries,
  ...debugSearchEntries,
  ...liveSoundSearchEntries,
  ...transcribeFileSearchEntries,
  ...textToSpeechSearchEntries,
  ...helpSearchEntries,
  ...aboutSearchEntries,
] as const satisfies readonly SettingsSearchEntry[];
