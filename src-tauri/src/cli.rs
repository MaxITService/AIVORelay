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
    version,
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

    /// Convert one or more text/Markdown files to audio, or one common audio
    /// file to text/Markdown, using the matching saved app configuration.
    ///
    /// This is intentionally separate from the legacy --transcribe-file
    /// benchmark command.
    #[arg(
        long,
        value_name = "FILE",
        num_args = 1..,
        action = clap::ArgAction::Append,
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
    pub convert_file: Vec<PathBuf>,

    /// Output path for one --convert-file input, or an output directory when
    /// multiple TXT/MD inputs are supplied. Its extension selects MP3/WAV for
    /// one TTS input or TXT/MD for transcription. When omitted, output is
    /// created next to each input using the saved format.
    #[arg(short = 'o', long, value_name = "FILE", requires = "convert_file")]
    pub output: Option<PathBuf>,

    /// Override the saved TTS provider for this TXT/MD conversion only.
    #[arg(long, value_enum, requires = "convert_file")]
    pub tts_provider: Option<CliTtsProvider>,

    /// Override the selected provider's saved voice for this conversion only.
    /// With Windows, omit this flag to use the current OS default voice.
    #[arg(long, value_name = "VOICE", requires = "convert_file")]
    pub tts_voice: Option<String>,

    /// Override the selected provider's model for this conversion only.
    #[arg(long, value_name = "MODEL", requires = "convert_file")]
    pub tts_model: Option<String>,

    /// Override the selected provider's language for this conversion only.
    #[arg(long, value_name = "LANGUAGE", requires = "convert_file")]
    pub tts_language: Option<String>,

    /// Override speech speed for this conversion only. The accepted range is
    /// provider-specific and is validated before synthesis.
    #[arg(long, value_name = "MULTIPLIER", requires = "convert_file")]
    pub tts_speed: Option<f32>,

    /// Select which already-stored credential to use for this conversion.
    /// Cloud providers only; no secret is accepted on the command line.
    #[arg(long, value_enum, requires = "convert_file")]
    pub tts_key_source: Option<CliTtsKeySource>,

    /// Select MP3 or WAV when --output is omitted. With --output, this must
    /// match the destination extension.
    #[arg(long, value_enum, requires = "convert_file")]
    pub tts_format: Option<CliTtsOutputFormat>,

    /// Override the final MP3 CBR bitrate in kb/s.
    #[arg(long, value_name = "KBPS", requires = "convert_file")]
    pub tts_bitrate: Option<u16>,

    /// Override the semantic file-conversion chunk target in Unicode
    /// characters. The provider hard limit is enforced.
    #[arg(long, value_name = "CHARS", requires = "convert_file")]
    pub tts_chunk_chars: Option<u32>,

    /// Override the number of retries after the first provider attempt.
    #[arg(long, value_name = "N", requires = "convert_file")]
    pub tts_retries: Option<u8>,

    /// Override the initial exponential retry delay in milliseconds.
    #[arg(long, value_name = "MS", requires = "convert_file")]
    pub tts_retry_delay_ms: Option<u32>,

    /// Override the silence inserted between ordinary chunks.
    #[arg(long, value_name = "MS", requires = "convert_file")]
    pub tts_chunk_pause_ms: Option<u32>,

    /// Override the silence inserted at paragraph boundaries.
    #[arg(long, value_name = "MS", requires = "convert_file")]
    pub tts_paragraph_pause_ms: Option<u32>,

    /// Enable or disable saved TTS preprocessing rules for this conversion.
    #[arg(long, value_name = "BOOL", requires = "convert_file")]
    pub tts_preprocessing: Option<bool>,

    /// Replace saved preprocessing rules with a UTF-8 JSON array of
    /// TextReplacement objects for this conversion only.
    #[arg(long, value_name = "FILE", requires = "convert_file")]
    pub tts_replacements_file: Option<PathBuf>,

    /// Enable or disable LLM cleanup for this file conversion. Other
    /// --tts-llm-* overrides imply true unless this is explicitly false.
    #[arg(long, value_name = "BOOL", requires = "convert_file")]
    pub tts_llm_preprocessing: Option<bool>,

    /// Use a saved, named TTS File Operations cleanup prompt.
    #[arg(long, value_name = "NAME", requires = "convert_file")]
    pub tts_llm_prompt: Option<String>,

    /// Use literal TTS File Operations LLM cleanup instructions.
    #[arg(long, value_name = "TEXT", requires = "convert_file")]
    pub tts_llm_instructions: Option<String>,

    /// Read literal TTS File Operations LLM cleanup instructions from UTF-8.
    /// Takes precedence over --tts-llm-instructions and --tts-llm-prompt.
    #[arg(long, value_name = "FILE", requires = "convert_file")]
    pub tts_llm_instructions_file: Option<PathBuf>,

    /// Override the saved TTS cleanup LLM provider ID.
    #[arg(long, value_name = "PROVIDER_ID", requires = "convert_file")]
    pub tts_llm_provider: Option<String>,

    /// Override the saved TTS cleanup LLM model ID.
    #[arg(long, value_name = "MODEL", requires = "convert_file")]
    pub tts_llm_model: Option<String>,

    /// Select which already-stored LLM credential to use. Secrets are never
    /// accepted on the command line.
    #[arg(long, value_enum, requires = "convert_file")]
    pub tts_llm_key_source: Option<CliTtsKeySource>,

    /// Override the OpenAI-compatible base URL. Supported only when the
    /// effective TTS cleanup provider is custom.
    #[arg(long, value_name = "URL", requires = "convert_file")]
    pub tts_llm_base_url: Option<String>,

    /// Allow insecure HTTP for a custom local TTS cleanup endpoint.
    #[arg(long, value_name = "BOOL", requires = "convert_file")]
    pub tts_llm_allow_insecure_http: Option<bool>,

    /// Enable or disable reasoning controls for TTS cleanup.
    #[arg(long, value_name = "BOOL", requires = "convert_file")]
    pub tts_llm_reasoning: Option<bool>,

    /// Override the TTS cleanup reasoning budget.
    #[arg(long, value_name = "TOKENS", requires = "convert_file")]
    pub tts_llm_reasoning_budget: Option<u32>,

    /// Override semantic LLM cleanup chunk size in Unicode characters.
    #[arg(long, value_name = "CHARS", requires = "convert_file")]
    pub tts_llm_chunk_chars: Option<u32>,

    /// Override LLM cleanup retries after the first request.
    #[arg(long, value_name = "N", requires = "convert_file")]
    pub tts_llm_retries: Option<u8>,

    /// Override initial LLM cleanup retry delay in milliseconds.
    #[arg(long, value_name = "MS", requires = "convert_file")]
    pub tts_llm_retry_delay_ms: Option<u32>,

    /// Override per-request LLM cleanup timeout in seconds.
    #[arg(long, value_name = "SECONDS", requires = "convert_file")]
    pub tts_llm_timeout_seconds: Option<u32>,

    /// Override the minimum free-disk reserve for this conversion.
    #[arg(long, value_name = "MB", requires = "convert_file")]
    pub tts_disk_reserve_mb: Option<u32>,

    /// Enable or disable TTS History capture for this conversion only.
    #[arg(long, value_name = "BOOL", requires = "convert_file")]
    pub tts_history: Option<bool>,

    /// Use a saved, named TTS instruction-prompt preset for --convert-file.
    /// OpenAI TTS only.
    #[arg(long, value_name = "NAME", requires = "convert_file")]
    pub tts_prompt: Option<String>,

    /// Use these TTS voice instructions for this conversion only. This is a
    /// literal argument: AivoRelay never evaluates it as shell code.
    #[arg(long, value_name = "TEXT", requires = "convert_file")]
    pub tts_instructions: Option<String>,

    /// Read TTS voice instructions from a UTF-8 file for this conversion.
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

impl CliArgs {
    pub(crate) fn has_tts_file_conversion_args(&self) -> bool {
        self.tts_provider.is_some()
            || self.tts_voice.is_some()
            || self.tts_model.is_some()
            || self.tts_language.is_some()
            || self.tts_speed.is_some()
            || self.tts_key_source.is_some()
            || self.tts_format.is_some()
            || self.tts_bitrate.is_some()
            || self.tts_chunk_chars.is_some()
            || self.tts_retries.is_some()
            || self.tts_retry_delay_ms.is_some()
            || self.tts_chunk_pause_ms.is_some()
            || self.tts_paragraph_pause_ms.is_some()
            || self.tts_preprocessing.is_some()
            || self.tts_replacements_file.is_some()
            || self.tts_llm_preprocessing.is_some()
            || self.tts_llm_prompt.is_some()
            || self.tts_llm_instructions.is_some()
            || self.tts_llm_instructions_file.is_some()
            || self.tts_llm_provider.is_some()
            || self.tts_llm_model.is_some()
            || self.tts_llm_key_source.is_some()
            || self.tts_llm_base_url.is_some()
            || self.tts_llm_allow_insecure_http.is_some()
            || self.tts_llm_reasoning.is_some()
            || self.tts_llm_reasoning_budget.is_some()
            || self.tts_llm_chunk_chars.is_some()
            || self.tts_llm_retries.is_some()
            || self.tts_llm_retry_delay_ms.is_some()
            || self.tts_llm_timeout_seconds.is_some()
            || self.tts_disk_reserve_mb.is_some()
            || self.tts_history.is_some()
            || self.tts_prompt.is_some()
            || self.tts_instructions.is_some()
            || self.tts_instructions_file.is_some()
    }
}

#[derive(Subcommand, Debug, Clone)]
pub enum CliCommand {
    /// Inspect, export, regenerate, or delete retained Text-to-Speech history.
    #[command(name = "tts-history")]
    TtsHistory(TtsHistoryArgs),
    /// Inspect, install, test, or delete an optional local TTS runtime.
    #[command(name = "tts-local")]
    TtsLocal(TtsLocalArgs),
}

#[derive(Args, Debug, Clone)]
pub struct TtsLocalArgs {
    /// Select the managed local engine.
    #[arg(long, value_enum, default_value_t = CliLocalTtsEngine::Qwen, global = true)]
    pub engine: CliLocalTtsEngine,

    #[command(subcommand)]
    pub command: TtsLocalCommand,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliLocalTtsEngine {
    Qwen,
    Kokoro,
}

#[derive(Subcommand, Debug, Clone)]
pub enum TtsLocalCommand {
    /// Show local model/runtime availability and exact pinned revision.
    Status,
    /// Download and install the app-managed local model and runtime.
    Install(TtsLocalConfirmationArgs),
    /// Delete the app-managed local model and runtime.
    Delete(TtsLocalConfirmationArgs),
    /// Generate a real local MP3/WAV without changing saved provider settings.
    Test(TtsLocalTestArgs),
}

#[derive(Args, Debug, Clone, Default)]
pub struct TtsLocalConfirmationArgs {
    /// Confirm the multi-gigabyte install or destructive deletion.
    #[arg(long)]
    pub yes: bool,
}

#[derive(Args, Debug, Clone)]
pub struct TtsLocalTestArgs {
    /// Text to synthesize. Defaults to a short engine-compatible validation text.
    #[arg(long, value_name = "TEXT")]
    pub text: Option<String>,

    /// New .mp3 or .wav output path. Existing files are never overwritten.
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output: PathBuf,

    /// Engine-specific official voice name.
    #[arg(long, value_name = "VOICE")]
    pub voice: Option<String>,

    /// Engine-specific language name.
    #[arg(long, value_name = "LANGUAGE")]
    pub language: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct TtsHistoryArgs {
    /// Select the independent TTS history to inspect or modify.
    #[arg(long, value_enum, default_value_t = CliTtsHistoryScope::File, global = true)]
    pub scope: CliTtsHistoryScope,

    #[command(subcommand)]
    pub command: TtsHistoryCommand,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliTtsHistoryScope {
    Interactive,
    File,
}

#[derive(Subcommand, Debug, Clone)]
pub enum TtsHistoryCommand {
    /// List retained TTS results, newest first.
    List(TtsHistoryListArgs),
    /// Show one retained TTS result.
    Show(TtsHistoryShowArgs),
    /// Export the retained audio copy without making an API request.
    Export(TtsHistoryExportArgs),
    /// Make a new TTS result and append it as a comparison variant.
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

    /// Enable or disable LLM cleanup for this regenerated variant.
    #[arg(long, value_name = "BOOL")]
    pub tts_llm_preprocessing: Option<bool>,

    /// Select a saved cleanup prompt from the entry scope.
    #[arg(long, value_name = "NAME")]
    pub tts_llm_prompt: Option<String>,

    /// Use literal LLM cleanup instructions for this variant.
    #[arg(long, value_name = "TEXT")]
    pub tts_llm_instructions: Option<String>,

    /// Read literal LLM cleanup instructions from a UTF-8 file.
    #[arg(long, value_name = "FILE")]
    pub tts_llm_instructions_file: Option<PathBuf>,

    /// Override the TTS cleanup provider ID.
    #[arg(long, value_name = "PROVIDER_ID")]
    pub tts_llm_provider: Option<String>,

    /// Override the TTS cleanup model ID.
    #[arg(long, value_name = "MODEL")]
    pub tts_llm_model: Option<String>,

    /// Select the shared or separate already-stored cleanup credential.
    #[arg(long, value_enum)]
    pub tts_llm_key_source: Option<CliTtsKeySource>,

    /// Override the custom provider base URL.
    #[arg(long, value_name = "URL")]
    pub tts_llm_base_url: Option<String>,

    /// Allow insecure HTTP for a trusted custom local endpoint.
    #[arg(long, value_name = "BOOL")]
    pub tts_llm_allow_insecure_http: Option<bool>,

    /// Enable or disable cleanup-model reasoning controls.
    #[arg(long, value_name = "BOOL")]
    pub tts_llm_reasoning: Option<bool>,

    /// Override cleanup reasoning budget.
    #[arg(long, value_name = "TOKENS")]
    pub tts_llm_reasoning_budget: Option<u32>,

    /// Override cleanup chunk size in Unicode characters.
    #[arg(long, value_name = "CHARS")]
    pub tts_llm_chunk_chars: Option<u32>,

    /// Override cleanup retries after the first request.
    #[arg(long, value_name = "N")]
    pub tts_llm_retries: Option<u8>,

    /// Override cleanup retry delay in milliseconds.
    #[arg(long, value_name = "MS")]
    pub tts_llm_retry_delay_ms: Option<u32>,

    /// Override cleanup request timeout in seconds.
    #[arg(long, value_name = "SECONDS")]
    pub tts_llm_timeout_seconds: Option<u32>,

    /// Explicit output format. With --output, it must match the extension;
    /// without --output, it selects the managed History result format.
    #[arg(long, value_enum)]
    pub format: Option<CliTtsOutputFormat>,

    /// MP3 CBR bitrate in kb/s.
    #[arg(long, value_name = "KBPS")]
    pub bitrate: Option<u16>,

    /// Confirm a new paid cloud API request without an interactive prompt.
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
    Edge,
    LocalQwen,
    LocalKokoro,
    Windows,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliTtsKeySource {
    Shared,
    Separate,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliTtsOutputFormat {
    Mp3,
    Wav,
}

#[cfg(test)]
mod tests {
    use super::{
        CliArgs, CliCommand, CliTtsHistoryScope, CliTtsKeySource, CliTtsOutputFormat,
        CliTtsProvider, TtsHistoryCommand, TtsLocalCommand,
    };
    use clap::{error::ErrorKind, Parser};

    #[test]
    fn exposes_package_version() {
        let error = CliArgs::try_parse_from(["aivorelay", "--version"])
            .expect_err("--version should be handled by clap before app startup");

        assert_eq!(error.kind(), ErrorKind::DisplayVersion);
        assert!(error.to_string().contains(env!("CARGO_PKG_VERSION")));
    }

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

        assert_eq!(args.convert_file.len(), 1);
        assert_eq!(args.convert_file[0].to_string_lossy(), "chapter.md");
        assert_eq!(args.output.unwrap().to_string_lossy(), "chapter.mp3");
        assert_eq!(args.tts_prompt.as_deref(), Some("Calm narrator"));
    }

    #[test]
    fn parses_multiple_tts_input_files_in_one_argument_group() {
        let args = CliArgs::try_parse_from([
            "aivorelay",
            "--convert-file",
            "chapter-1.md",
            "chapter-2.txt",
            "--output",
            "audio",
        ])
        .expect("multiple TTS input paths should parse");

        assert_eq!(args.convert_file.len(), 2);
        assert_eq!(args.convert_file[0].to_string_lossy(), "chapter-1.md");
        assert_eq!(args.convert_file[1].to_string_lossy(), "chapter-2.txt");
        assert_eq!(args.output.unwrap().to_string_lossy(), "audio");
    }

    #[test]
    fn parses_comprehensive_temporary_tts_overrides() {
        let args = CliArgs::try_parse_from([
            "aivorelay",
            "--convert-file",
            "chapter.md",
            "--tts-provider",
            "soniox",
            "--tts-model",
            "sonic-preview",
            "--tts-voice",
            "voice-id",
            "--tts-language",
            "ru",
            "--tts-speed",
            "1.2",
            "--tts-key-source",
            "separate",
            "--tts-format",
            "mp3",
            "--tts-bitrate",
            "192",
            "--tts-chunk-chars",
            "1400",
            "--tts-retries",
            "4",
            "--tts-retry-delay-ms",
            "750",
            "--tts-chunk-pause-ms",
            "80",
            "--tts-paragraph-pause-ms",
            "300",
            "--tts-preprocessing",
            "true",
            "--tts-replacements-file",
            "rules.json",
            "--tts-llm-preprocessing",
            "true",
            "--tts-llm-instructions",
            "Remove page numbers.",
            "--tts-llm-provider",
            "custom",
            "--tts-llm-model",
            "cleanup-model",
            "--tts-llm-key-source",
            "separate",
            "--tts-llm-base-url",
            "https://example.test/v1",
            "--tts-llm-allow-insecure-http",
            "false",
            "--tts-llm-reasoning",
            "true",
            "--tts-llm-reasoning-budget",
            "4096",
            "--tts-llm-chunk-chars",
            "12000",
            "--tts-llm-retries",
            "3",
            "--tts-llm-retry-delay-ms",
            "900",
            "--tts-llm-timeout-seconds",
            "120",
            "--tts-disk-reserve-mb",
            "1024",
            "--tts-history",
            "false",
        ])
        .expect("TTS provider overrides should parse");

        assert_eq!(args.tts_provider, Some(CliTtsProvider::Soniox));
        assert_eq!(args.tts_model.as_deref(), Some("sonic-preview"));
        assert_eq!(args.tts_voice.as_deref(), Some("voice-id"));
        assert_eq!(args.tts_language.as_deref(), Some("ru"));
        assert_eq!(args.tts_speed, Some(1.2));
        assert_eq!(args.tts_key_source, Some(CliTtsKeySource::Separate));
        assert_eq!(args.tts_format, Some(CliTtsOutputFormat::Mp3));
        assert_eq!(args.tts_bitrate, Some(192));
        assert_eq!(args.tts_chunk_chars, Some(1400));
        assert_eq!(args.tts_retries, Some(4));
        assert_eq!(args.tts_retry_delay_ms, Some(750));
        assert_eq!(args.tts_chunk_pause_ms, Some(80));
        assert_eq!(args.tts_paragraph_pause_ms, Some(300));
        assert_eq!(args.tts_preprocessing, Some(true));
        assert_eq!(
            args.tts_replacements_file
                .as_deref()
                .map(|path| path.to_string_lossy().into_owned()),
            Some("rules.json".to_string())
        );
        assert_eq!(args.tts_llm_preprocessing, Some(true));
        assert_eq!(
            args.tts_llm_instructions.as_deref(),
            Some("Remove page numbers.")
        );
        assert_eq!(args.tts_llm_provider.as_deref(), Some("custom"));
        assert_eq!(args.tts_llm_model.as_deref(), Some("cleanup-model"));
        assert_eq!(args.tts_llm_key_source, Some(CliTtsKeySource::Separate));
        assert_eq!(
            args.tts_llm_base_url.as_deref(),
            Some("https://example.test/v1")
        );
        assert_eq!(args.tts_llm_allow_insecure_http, Some(false));
        assert_eq!(args.tts_llm_reasoning, Some(true));
        assert_eq!(args.tts_llm_reasoning_budget, Some(4096));
        assert_eq!(args.tts_llm_chunk_chars, Some(12000));
        assert_eq!(args.tts_llm_retries, Some(3));
        assert_eq!(args.tts_llm_retry_delay_ms, Some(900));
        assert_eq!(args.tts_llm_timeout_seconds, Some(120));
        assert_eq!(args.tts_disk_reserve_mb, Some(1024));
        assert_eq!(args.tts_history, Some(false));
        assert!(args.has_tts_file_conversion_args());
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
            vec![
                "aivorelay",
                "tts-history",
                "list",
                "--scope",
                "interactive",
                "--limit",
                "10",
            ],
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
    fn parses_local_tts_lifecycle_commands() {
        for command in [
            vec!["aivorelay", "tts-local", "status"],
            vec!["aivorelay", "tts-local", "install", "--yes"],
            vec!["aivorelay", "tts-local", "delete", "--yes"],
            vec![
                "aivorelay",
                "tts-local",
                "test",
                "--output",
                "local-test.mp3",
            ],
        ] {
            CliArgs::try_parse_from(command).expect("local TTS command should parse");
        }
        let args = CliArgs::try_parse_from(["aivorelay", "tts-local", "status"])
            .expect("local status should parse");
        let Some(CliCommand::TtsLocal(local)) = args.command else {
            panic!("expected tts-local command");
        };
        assert!(matches!(local.command, TtsLocalCommand::Status));
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
        assert_eq!(history.scope, CliTtsHistoryScope::File);
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
        assert_eq!(history.scope, CliTtsHistoryScope::File);
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
