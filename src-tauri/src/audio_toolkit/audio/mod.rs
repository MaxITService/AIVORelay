// Re-export all audio components
mod device;
mod recorder;
mod resampler;
mod utils;
mod visualizer;

pub use device::{list_input_devices, list_output_devices, CpalDeviceInfo};
pub use recorder::{AudioCaptureSource, AudioRecorder, StreamFrameCallback};
pub use resampler::FrameResampler;
pub use utils::{encode_wav_bytes, read_wav_samples, save_wav_file, verify_wav_file};
pub use visualizer::AudioVisualiser;
