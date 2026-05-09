use nnnoiseless::DenoiseState;
use rubato::{FftFixedIn, Resampler};

use crate::audio_toolkit::constants;

const RNNOISE_SAMPLE_RATE: usize = 48_000;
const RNNOISE_FRAME_SIZE: usize = DenoiseState::FRAME_SIZE;
const I16_SCALE: f32 = i16::MAX as f32;

pub struct NoiseSuppressor {
    upsampler: FftFixedIn<f32>,
    downsampler: FftFixedIn<f32>,
    denoise: Box<DenoiseState<'static>>,
    input_frame_size: usize,
}

impl NoiseSuppressor {
    pub fn new_16khz() -> Result<Self, String> {
        let input_frame_size = (constants::WHISPER_SAMPLE_RATE as usize * 30) / 1000;
        let output_frame_size = RNNOISE_FRAME_SIZE * 3;

        let upsampler = FftFixedIn::<f32>::new(
            constants::WHISPER_SAMPLE_RATE as usize,
            RNNOISE_SAMPLE_RATE,
            input_frame_size,
            1,
            1,
        )
        .map_err(|e| format!("Failed to create RNNoise upsampler: {e}"))?;

        let downsampler = FftFixedIn::<f32>::new(
            RNNOISE_SAMPLE_RATE,
            constants::WHISPER_SAMPLE_RATE as usize,
            output_frame_size,
            1,
            1,
        )
        .map_err(|e| format!("Failed to create RNNoise downsampler: {e}"))?;

        Ok(Self {
            upsampler,
            downsampler,
            denoise: DenoiseState::new(),
            input_frame_size,
        })
    }

    pub fn process_16khz_frame(&mut self, samples: &[f32]) -> Vec<f32> {
        if samples.len() != self.input_frame_size {
            return samples.to_vec();
        }

        let upsampled = match self.upsampler.process(&[samples], None) {
            Ok(channels) => channels.into_iter().next().unwrap_or_default(),
            Err(err) => {
                log::warn!("RNNoise upsample failed: {err}");
                return samples.to_vec();
            }
        };

        if upsampled.len() < RNNOISE_FRAME_SIZE {
            return samples.to_vec();
        }

        let mut denoised_48khz = Vec::with_capacity(upsampled.len());
        let mut input_frame = [0.0f32; RNNOISE_FRAME_SIZE];
        let mut output_frame = [0.0f32; RNNOISE_FRAME_SIZE];

        for chunk in upsampled.chunks_exact(RNNOISE_FRAME_SIZE) {
            for (dst, src) in input_frame.iter_mut().zip(chunk.iter()) {
                *dst = (*src * I16_SCALE).clamp(i16::MIN as f32, i16::MAX as f32);
            }

            self.denoise.process_frame(&mut output_frame, &input_frame);

            denoised_48khz.extend(
                output_frame
                    .iter()
                    .map(|sample| (*sample / I16_SCALE).clamp(-1.0, 1.0)),
            );
        }

        if denoised_48khz.is_empty() {
            return samples.to_vec();
        }

        match self.downsampler.process(&[&denoised_48khz], None) {
            Ok(channels) => {
                let mut output = channels.into_iter().next().unwrap_or_default();
                if output.len() > samples.len() {
                    output.truncate(samples.len());
                } else if output.len() < samples.len() {
                    output.resize(samples.len(), 0.0);
                }
                output
            }
            Err(err) => {
                log::warn!("RNNoise downsample failed: {err}");
                samples.to_vec()
            }
        }
    }
}
