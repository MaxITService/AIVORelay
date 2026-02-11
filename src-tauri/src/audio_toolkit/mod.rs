pub mod audio;
pub mod constants;
pub mod text;
pub mod utils;
pub mod vad;

pub use audio::{
    encode_wav_bytes, list_input_devices, list_output_devices, save_wav_file, AudioRecorder,
    CpalDeviceInfo, StreamFrameCallback,
};
pub use text::{apply_custom_words, filter_transcription_output};
pub use utils::get_cpal_host;
pub use vad::{SileroVad, VoiceActivityDetector};
