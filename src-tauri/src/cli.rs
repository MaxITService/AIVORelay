use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

fn parse_positive_usize(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| "value must be a positive integer".to_string())?;
    if parsed == 0 {
        Err("value must be at least 1".to_string())
    } else {
        Ok(parsed)
    }
}

#[derive(Parser, Debug, Clone, Default)]
#[command(
    name = "aivorelay",
    about = "AivoRelay - Speech to Text and Text to Speech"
)]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Option<CliCommand>,

    /// Toggle transcription on/off (sent to running instance).
    #[arg(long)]
    pub toggle_transcription: bool,

    /// Toggle transcription with post-processing on/off (sent to running instance).
    #[arg(long)]
    pub toggle_post_process: bool,

    /// Cancel the current operation (sent to running instance).
    #[arg(long)]
    pub cancel: bool,

    /// Enable debug mode with verbose logging.
    #[arg(long)]
    pub debug: bool,

    /// Transcribe this WAV (16 kHz mono) headlessly and exit. Runs the same
    /// batch transcription path as the app: no mic, no VAD, no download.
    #[arg(short = 'f', long, value_name = "WAV")]
    pub transcribe_file: Option<PathBuf>,

    /// Convert a text/Markdown file to audio, or a common audio file to
    /// text/Markdown, using the matching saved app configuration.
    ///
    /// This is intentionally separate from the legacy --transcribe-file
    /// benchmark command.
    #[arg(
        long,
        value_name = "FILE",
        conflicts_with_all = [
            "toggle_transcription",
            "toggle_post_process",
            "cancel",
            "transcribe_file",
            "list_devices",
            "model",
            "device_index",
            "repeat"
        ]
    )]
    pub convert_file: Option<PathBuf>,

    /// Output path for --convert-file. Its extension selects MP3/WAV for TTS
    /// or TXT/MD for transcription. When omitted, the saved TTS format or
    /// Markdown is used next to the input file.
    #[arg(short = 'o', long, value_name = "FILE", requires = "convert_file")]
    pub output: Option<PathBuf>,

    /// Use a saved, named TTS instruction-prompt preset for --convert-file.
    /// OpenAI TTS only.
    #[arg(long, value_name = "NAME", requires = "convert_file")]
    pub tts_prompt: Option<String>,

    /// Use these OpenAI TTS instructions for this conversion only. This is a
    /// literal argument: AivoRelay never evaluates it as shell code.
    #[arg(long, value_name = "TEXT", requires = "convert_file")]
    pub tts_instructions: Option<String>,

    /// Read OpenAI TTS instructions from a UTF-8 file for this conversion.
    /// Takes precedence over --tts-instructions and --tts-prompt.
    #[arg(long, value_name = "FILE", requires = "convert_file")]
    pub tts_instructions_file: Option<PathBuf>,

    /// Model id to load for --transcribe-file (default: selected app model).
    #[arg(long)]
    pub model: Option<String>,

    /// Hard-select the compute device for --transcribe-file by --list-devices
    /// index. 0 = CPU, 1.. = specific GPU. Whisper.cpp models only.
    #[arg(long, value_name = "N")]
    pub device_index: Option<usize>,

    /// List selectable whisper compute devices and exit.
    #[arg(long)]
    pub list_devices: bool,

    /// Repeat transcription N times; best_ms reports the fastest run.
    #[arg(long, value_name = "N")]
    pub repeat: Option<usize>,

    /// Emit headless operation results as JSON.
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum CliCommand {
    /// Inspect, export, regenerate, or delete retained Text-to-Speech history.
    #[command(name = "tts-history")]
    TtsHistory(TtsHistoryArgs),
}

#[derive(Args, Debug, Clone)]
pub struct TtsHistoryArgs {
    #[command(subcommand)]
    pub command: TtsHistoryCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum TtsHistoryCommand {
    /// List retained TTS results, newest first.
    List(TtsHistoryListArgs),
    /// Show one retained TTS result.
    Show(TtsHistoryShowArgs),
    /// Export the retained audio copy without making an API request.
    Export(TtsHistoryExportArgs),
    /// Make a new paid TTS request and append it as a comparison variant.
    Regenerate(TtsHistoryRegenerateArgs),
    /// Delete one retained result and its managed audio copy.
    Delete(TtsHistoryDeleteArgs),
}

#[derive(Args, Debug, Clone, Default)]
pub struct TtsHistoryListArgs {
    /// Maximum number of entries to return. Omit for all entries.
    #[arg(long, value_name = "N", value_parser = parse_positive_usize)]
    pub limit: Option<usize>,

    /// Return only variants sharing this exact history group ID.
    #[arg(long, value_name = "GROUP_ID")]
    pub group: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct TtsHistoryShowArgs {
    /// Numeric TTS history result ID.
    pub id: i64,
}

#[derive(Args, Debug, Clone)]
pub struct TtsHistoryExportArgs {
    /// Numeric TTS history result ID.
    pub id: i64,

    /// New MP3/WAV destination. Existing files are never overwritten.
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output: PathBuf,
}

#[derive(Args, Debug, Clone)]
pub struct TtsHistoryRegenerateArgs {
    /// Numeric source TTS history result ID.
    pub id: i64,

    /// Optional new MP3/WAV destination. When omitted, only the managed TTS
    /// History result is kept; its default format is MP3.
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Override the retained provider for this new variant.
    #[arg(long, value_enum)]
    pub provider: Option<CliTtsProvider>,

    /// Override the provider model for this new variant.
    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,

    /// Override the provider voice for this new variant.
    #[arg(long, value_name = "VOICE")]
    pub voice: Option<String>,

    /// Use a saved, named OpenAI TTS prompt preset.
    #[arg(long, value_name = "NAME")]
    pub tts_prompt: Option<String>,

    /// Use literal OpenAI TTS instructions for this new variant.
    #[arg(long, value_name = "TEXT")]
    pub tts_instructions: Option<String>,

    /// Read literal OpenAI TTS instructions from a UTF-8 file.
    #[arg(long, value_name = "FILE")]
    pub tts_instructions_file: Option<PathBuf>,

    /// Explicit output format. With --output, it must match the extension;
    /// without --output, it selects the managed History result format.
    #[arg(long, value_enum)]
    pub format: Option<CliTtsOutputFormat>,

    /// MP3 CBR bitrate in kb/s.
    #[arg(long, value_name = "KBPS")]
    pub bitrate: Option<u16>,

    /// Confirm the new paid API request without an interactive prompt.
    #[arg(long)]
    pub yes: bool,
}

#[derive(Args, Debug, Clone)]
pub struct TtsHistoryDeleteArgs {
    /// Numeric TTS history result ID.
    pub id: i64,

    /// Confirm destructive deletion without an interactive prompt.
    #[arg(long)]
    pub yes: bool,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliTtsProvider {
    Soniox,
    Deepgram,
    Openai,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliTtsOutputFormat {
    Mp3,
    Wav,
}

#[cfg(test)]
mod tests {
    use super::{CliArgs, CliCommand, CliTtsProvider, TtsHistoryCommand};
    use clap::Parser;

    #[test]
    fn parses_symmetric_file_conversion_and_named_tts_prompt() {
        let args = CliArgs::try_parse_from([
            "aivorelay",
            "--convert-file",
            "chapter.md",
            "--output",
            "chapter.mp3",
            "--tts-prompt",
            "Calm narrator",
        ])
        .expect("conversion arguments should parse");

        assert_eq!(args.convert_file.unwrap().to_string_lossy(), "chapter.md");
        assert_eq!(args.output.unwrap().to_string_lossy(), "chapter.mp3");
        assert_eq!(args.tts_prompt.as_deref(), Some("Calm narrator"));
    }

    #[test]
    fn output_requires_convert_file() {
        assert!(
            CliArgs::try_parse_from(["aivorelay", "--output", "result.md"]).is_err(),
            "--output without --convert-file must be rejected"
        );
    }

    #[test]
    fn new_conversion_conflicts_with_legacy_benchmark() {
        assert!(CliArgs::try_parse_from([
            "aivorelay",
            "--convert-file",
            "meeting.mp3",
            "--transcribe-file",
            "benchmark.wav",
        ])
        .is_err());
    }

    #[test]
    fn parses_every_tts_history_subcommand() {
        for command in [
            vec!["aivorelay", "tts-history", "list", "--limit", "10"],
            vec!["aivorelay", "tts-history", "show", "42"],
            vec![
                "aivorelay",
                "tts-history",
                "export",
                "42",
                "--output",
                "copy.mp3",
            ],
            vec![
                "aivorelay",
                "tts-history",
                "regenerate",
                "42",
                "--output",
                "new.mp3",
                "--yes",
            ],
            vec!["aivorelay", "tts-history", "delete", "42", "--yes"],
        ] {
            CliArgs::try_parse_from(command).expect("history subcommand should parse");
        }
    }

    #[test]
    fn history_json_is_global_and_regeneration_overrides_are_typed() {
        let args = CliArgs::try_parse_from([
            "aivorelay",
            "tts-history",
            "regenerate",
            "7",
            "--output",
            "variant.wav",
            "--provider",
            "openai",
            "--voice",
            "marin",
            "--tts-instructions",
            "Read “UTF-8” literally.",
            "--format",
            "wav",
            "--json",
            "--yes",
        ])
        .expect("history regeneration should parse");
        assert!(args.json);
        let Some(CliCommand::TtsHistory(history)) = args.command else {
            panic!("expected tts-history command");
        };
        let TtsHistoryCommand::Regenerate(regenerate) = history.command else {
            panic!("expected regenerate command");
        };
        assert_eq!(regenerate.provider, Some(CliTtsProvider::Openai));
        assert_eq!(
            regenerate.tts_instructions.as_deref(),
            Some("Read “UTF-8” literally.")
        );
    }

    #[test]
    fn history_regeneration_allows_managed_only_output() {
        let args =
            CliArgs::try_parse_from(["aivorelay", "tts-history", "regenerate", "7", "--yes"])
                .expect("managed-only history regeneration should parse");
        let Some(CliCommand::TtsHistory(history)) = args.command else {
            panic!("expected tts-history command");
        };
        let TtsHistoryCommand::Regenerate(regenerate) = history.command else {
            panic!("expected regenerate command");
        };
        assert!(regenerate.output.is_none());
    }

    #[test]
    fn history_parser_rejects_invalid_provider_format_and_zero_limit() {
        assert!(CliArgs::try_parse_from([
            "aivorelay",
            "tts-history",
            "regenerate",
            "1",
            "--output",
            "new.mp3",
            "--provider",
            "unknown",
        ])
        .is_err());
        assert!(CliArgs::try_parse_from([
            "aivorelay",
            "tts-history",
            "regenerate",
            "1",
            "--output",
            "new.mp3",
            "--format",
            "flac",
        ])
        .is_err());
        assert!(
            CliArgs::try_parse_from(["aivorelay", "tts-history", "list", "--limit", "0",]).is_err()
        );
    }
}
