export interface HelpSearchResult {
  anchor: string;
  score: number;
}

interface HelpSearchRecord {
  anchor: string;
  title: string;
  summary: string;
  destination: string;
  keywords: readonly string[];
  voiceCommandsOnly?: boolean;
}

const HELP_SEARCH_RECORDS: readonly HelpSearchRecord[] = [
  {
    anchor: "help-transcription",
    title: "Press a key. Speak. Text appears.",
    summary:
      "Set a microphone and shortcut in Speech / Mic. Choose where speech becomes text in Models: a local model works on this computer, while a cloud provider works online.",
    destination: "Speech / Mic",
    keywords: ["speech", "mic", "microphone", "shortcut", "dictation", "voice"],
  },
  {
    anchor: "help-models",
    title: "Choose a local model or cloud provider.",
    summary:
      "A local model works on this computer. A cloud provider processes the recording online.",
    destination: "Models",
    keywords: ["model", "local", "cloud", "provider", "speech to text", "STT"],
  },
  {
    anchor: "help-speech-processing",
    title: "Improve microphone audio.",
    summary:
      "Adjust input levels and microphone cleanup before transcription. This can help when speech is too quiet or background noise is distracting.",
    destination: "Speech Processing",
    keywords: ["audio", "noise", "cancellation", "boost", "input", "microphone"],
  },
  {
    anchor: "help-advanced",
    title: "Control advanced application behaviour.",
    summary:
      "Advanced contains less common controls. Use it to change startup, clipboard, or other application behaviour.",
    destination: "Advanced",
    keywords: ["advanced", "startup", "clipboard", "paste", "behaviour"],
  },
  {
    anchor: "help-post-processing",
    title: "Speak naturally. AI cleans up the result.",
    summary: "An LLM is an AI text editor. It can fix wording after speech becomes text.",
    destination: "LLM Post Processing",
    keywords: ["LLM", "AI", "clean", "cleanup", "rewrite", "post processing"],
  },
  {
    anchor: "help-ai-replace",
    title: "Select text. Say what to change. It is replaced.",
    summary:
      "Select text, then say what you want changed. AI returns replacement text for the selection.",
    destination: "AI Replace",
    keywords: ["selection", "replace", "rewrite", "instruction"],
  },
  {
    anchor: "help-transcribe-file",
    title: "Choose an audio or video file. Get text.",
    summary:
      "Choose an audio or video file. AivoRelay creates a text transcript you can save.",
    destination: "Transcribe File",
    keywords: ["file", "audio", "video", "transcript", "SRT", "VTT"],
  },
  {
    anchor: "help-live-monitor",
    title: "Play computer audio. See the words live.",
    summary:
      "Play audio from your computer. AivoRelay writes the words while the audio plays.",
    destination: "Live Monitor",
    keywords: ["live", "monitor", "computer audio", "loopback", "stream"],
  },
  {
    anchor: "help-speak-selected-text",
    title: "Select text. Press a key. Hear it spoken.",
    summary:
      "Open Speak selected text, choose a provider, and start with its recommended preset. Assign a shortcut under Actions, then select text in any app and press the shortcut to hear it aloud. You can read the clipboard, copy selected text first, or read a selection directly where Windows supports it; cloud providers send text online, while Windows voices and local models keep speech generation on this computer.",
    destination: "Speak selected text",
    keywords: [
      "text to speech",
      "TTS",
      "read aloud",
      "voice",
      "provider",
      "preset",
      "shortcut",
      "speed",
      "playback",
      "history",
      "read clipboard",
      "copy and read",
      "direct selection",
      "local",
      "cloud",
      "clipboard",
    ],
  },
  {
    anchor: "help-text-file-to-mp3",
    title: "Choose text files. Get MP3 or WAV audio.",
    summary:
      "Open Text file to mp3, choose a provider, and select a text or Markdown file. Choose MP3 or WAV, select where to save it, and start conversion; File Operations has its own settings, while synthesis presets can be shared with Speak selected text. The page also supports multiple files, folders, resumable work, folder automation, and File History; optional LLM cleanup can send text online.",
    destination: "Text file to mp3",
    keywords: [
      "text file",
      "Markdown",
      "MP3",
      "WAV",
      "audio",
      "convert",
      "provider",
      "preset",
      "batch",
      "folder",
      "automation",
      "resume",
      "history",
      "bitrate",
      "file operations",
      "synthesis preset",
      "LLM cleanup",
      "local",
      "cloud",
      "CLI",
    ],
  },
  {
    anchor: "help-voice-commands",
    title: "Speak a command. AivoRelay performs it.",
    summary:
      "Say an instruction and AivoRelay can perform the enabled action.",
    destination: "Voice Commands",
    keywords: ["voice command", "command", "action", "automation"],
    voiceCommandsOnly: true,
  },
  {
    anchor: "help-connector",
    title: "Connect to an LLM chat open in Chrome via the separate extension.",
    summary:
      "The Connector sends text from AivoRelay to a web chat, such as ChatGPT or Claude, open in Chrome through a separate extension. Open Connector to set up that connection.",
    destination: "Connector",
    keywords: ["Chrome", "extension", "browser", "ChatGPT", "Claude", "connector"],
  },
  {
    anchor: "help-text-processing",
    title: "Add corrections once. Apply them automatically.",
    summary:
      "Add a replacement such as a common spelling fix. AivoRelay applies it when matching text appears.",
    destination: "Text Processing",
    keywords: ["correction", "replacement", "rule", "text", "automatic"],
  },
  {
    anchor: "help-history",
    title: "Find your previous transcriptions.",
    summary:
      "History keeps previous transcriptions and recordings. Open it when you want to find an older result.",
    destination: "History",
    keywords: ["history", "previous", "recording", "transcription", "search"],
  },
  {
    anchor: "help-user-interface",
    title: "Choose how results appear.",
    summary:
      "Choose how the overlay and results look. These settings change what you see while using AivoRelay.",
    destination: "User Interface",
    keywords: ["UI", "interface", "overlay", "preview", "appearance"],
  },
  {
    anchor: "help-debug",
    title: "Test features and diagnose problems.",
    summary:
      "Debug contains tests, logs, and diagnostic tools. Open it when a feature does not behave as expected.",
    destination: "Debug",
    keywords: ["debug", "test", "diagnose", "logs", "troubleshoot"],
  },
];

const normalize = (value: string) =>
  value
    .toLocaleLowerCase()
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "");

export const searchHelp = (
  query: string,
  options: { includeVoiceCommands?: boolean } = {},
): HelpSearchResult[] => {
  const normalizedQuery = normalize(query.trim());
  if (!normalizedQuery) return [];

  const words = normalizedQuery.split(/\s+/).filter(Boolean);
  const includeVoiceCommands = options.includeVoiceCommands ?? false;

  return HELP_SEARCH_RECORDS.filter(
    (record) => includeVoiceCommands || !record.voiceCommandsOnly,
  )
    .map((record) => {
      const fields = [
        { value: normalize(record.title), weight: 8 },
        { value: normalize(record.summary), weight: 5 },
        { value: normalize(record.destination), weight: 7 },
        ...record.keywords.map((keyword) => ({
          value: normalize(keyword),
          weight: 3,
        })),
      ];

      let score = 0;
      for (const word of words) {
        const matchingField = fields.find((field) => field.value.includes(word));
        if (matchingField) score += matchingField.weight;
      }

      if (normalize(record.title).includes(normalizedQuery)) score += 8;
      if (normalize(record.destination) === normalizedQuery) score += 10;

      return { anchor: record.anchor, score };
    })
    .filter((result) => result.score > 0)
    .sort((a, b) => b.score - a.score)
    .slice(0, 8);
};
