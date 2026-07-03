use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone, Default)]
#[command(name = "aivorelay", about = "AivoRelay - Speech to Text")]
pub struct CliArgs {
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

    /// Emit --transcribe-file results as JSON.
    #[arg(long)]
    pub json: bool,
}
