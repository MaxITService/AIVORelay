//! Minimal safe wrapper around the statically linked official Xiph libopus.
//!
//! Keeping the FFI surface here makes the TTS encoder independent of a system
//! codec while avoiding a second, separately maintained Opus implementation.

use std::ffi::{c_char, c_int, CStr};
use std::ptr::NonNull;

pub(crate) const MAX_PACKET_BYTES: usize = 4_000;

const OPUS_OK: c_int = 0;
const OPUS_APPLICATION_AUDIO: c_int = 2_049;
const OPUS_SET_BITRATE_REQUEST: c_int = 4_002;
const OPUS_SET_VBR_REQUEST: c_int = 4_006;
const OPUS_GET_LOOKAHEAD_REQUEST: c_int = 4_027;
const OPUS_SET_LSB_DEPTH_REQUEST: c_int = 4_036;

#[repr(C)]
struct OpusEncoderState {
    _private: [u8; 0],
}

#[repr(C)]
struct OpusDecoderState {
    _private: [u8; 0],
}

extern "C" {
    fn opus_decoder_create(sample_rate: c_int, channels: c_int, error: *mut c_int) -> *mut OpusDecoderState;
    fn opus_decoder_destroy(decoder: *mut OpusDecoderState);
    fn opus_decode_float(
        decoder: *mut OpusDecoderState,
        packet: *const u8,
        packet_bytes: c_int,
        pcm: *mut f32,
        frame_size: c_int,
        decode_fec: c_int,
    ) -> c_int;
    fn opus_encoder_create(
        sample_rate: c_int,
        channels: c_int,
        application: c_int,
        error: *mut c_int,
    ) -> *mut OpusEncoderState;
    fn opus_encoder_destroy(encoder: *mut OpusEncoderState);
    fn opus_encode(
        encoder: *mut OpusEncoderState,
        pcm: *const i16,
        frame_size: c_int,
        packet: *mut u8,
        max_packet_bytes: c_int,
    ) -> c_int;
    fn opus_encoder_ctl(encoder: *mut OpusEncoderState, request: c_int, ...) -> c_int;
    fn opus_strerror(error: c_int) -> *const c_char;
    fn opus_get_version_string() -> *const c_char;
}

pub(crate) struct Encoder {
    inner: NonNull<OpusEncoderState>,
    channels: usize,
}

/// Owned mono/stereo decoder, always producing 48 kHz interleaved PCM.
pub(crate) struct Decoder {
    inner: NonNull<OpusDecoderState>,
    channels: usize,
}

impl Decoder {
    pub(crate) fn new(channels: usize) -> Result<Self, String> {
        if !(1..=2).contains(&channels) {
            return Err("Opus supports mono or stereo input only".to_string());
        }
        let mut error = OPUS_OK;
        // SAFETY: channel count is checked; libopus writes only to `error` and
        // returns an owned state that is freed exactly once by Drop.
        let decoder = unsafe { opus_decoder_create(48_000, channels as c_int, &mut error) };
        check(error)?;
        let inner = NonNull::new(decoder)
            .ok_or_else(|| "libopus returned a null decoder without an error".to_string())?;
        Ok(Self { inner, channels })
    }

    pub(crate) fn decode(&mut self, packet: &[u8], pcm: &mut [f32]) -> Result<usize, String> {
        if packet.is_empty() {
            return Err("Empty Opus packet".to_string());
        }
        let packet_bytes = c_int::try_from(packet.len())
            .map_err(|_| "Opus packet is too large".to_string())?;
        let frame_size = c_int::try_from(pcm.len() / self.channels)
            .map_err(|_| "Opus PCM buffer is too large".to_string())?;
        if frame_size == 0 { return Err("Opus PCM buffer is empty".to_string()); }
        // SAFETY: the decoder is alive and exclusively borrowed. Both slices
        // remain alive, and libopus receives their checked lengths. frame_size
        // is per channel, so it cannot write beyond pcm.
        let frames = unsafe {
            opus_decode_float(self.inner.as_ptr(), packet.as_ptr(), packet_bytes,
                pcm.as_mut_ptr(), frame_size, 0)
        };
        if frames < 0 { return Err(error_message(frames)); }
        Ok(frames as usize)
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        // SAFETY: inner was returned by opus_decoder_create and is freed once.
        unsafe { opus_decoder_destroy(self.inner.as_ptr()) };
    }
}

impl Encoder {
    pub(crate) fn new(sample_rate: u32, channels: u8) -> Result<Self, String> {
        let sample_rate = c_int::try_from(sample_rate)
            .map_err(|_| "Opus sample rate is too large".to_string())?;
        let mut error = OPUS_OK;
        // SAFETY: libopus accepts these scalar values and writes only to `error`.
        // A successful call returns an owned encoder freed by `Drop` below.
        let encoder = unsafe {
            opus_encoder_create(
                sample_rate,
                channels as c_int,
                OPUS_APPLICATION_AUDIO,
                &mut error,
            )
        };
        if error != OPUS_OK {
            return Err(error_message(error));
        }
        let inner = NonNull::new(encoder)
            .ok_or_else(|| "libopus returned a null encoder without an error".to_string())?;
        Ok(Self {
            inner,
            channels: channels as usize,
        })
    }

    pub(crate) fn set_bitrate(&mut self, bitrate_bps: i32) -> Result<(), String> {
        // SAFETY: the encoder is alive, and this request takes one promoted int.
        let result = unsafe {
            opus_encoder_ctl(
                self.inner.as_ptr(),
                OPUS_SET_BITRATE_REQUEST,
                bitrate_bps as c_int,
            )
        };
        check(result)
    }

    pub(crate) fn set_vbr(&mut self, enabled: bool) -> Result<(), String> {
        // SAFETY: the encoder is alive, and this request takes one promoted int.
        let result = unsafe {
            opus_encoder_ctl(
                self.inner.as_ptr(),
                OPUS_SET_VBR_REQUEST,
                if enabled { 1 as c_int } else { 0 as c_int },
            )
        };
        check(result)
    }

    pub(crate) fn set_lsb_depth(&mut self, bits: u8) -> Result<(), String> {
        // SAFETY: the encoder is alive, and this request takes one promoted int.
        let result = unsafe {
            opus_encoder_ctl(
                self.inner.as_ptr(),
                OPUS_SET_LSB_DEPTH_REQUEST,
                bits as c_int,
            )
        };
        check(result)
    }

    pub(crate) fn lookahead(&self) -> Result<u32, String> {
        let mut samples: c_int = 0;
        // SAFETY: the encoder is alive and libopus writes one int to `samples`.
        let result = unsafe {
            opus_encoder_ctl(
                self.inner.as_ptr(),
                OPUS_GET_LOOKAHEAD_REQUEST,
                &mut samples as *mut c_int,
            )
        };
        check(result)?;
        u32::try_from(samples).map_err(|_| format!("libopus returned invalid lookahead: {samples}"))
    }

    pub(crate) fn encode_s16(
        &mut self,
        pcm: &[i16],
        frame_samples_per_channel: usize,
        packet: &mut [u8],
    ) -> Result<usize, String> {
        let required_samples = frame_samples_per_channel
            .checked_mul(self.channels)
            .ok_or_else(|| "Opus frame size overflow".to_string())?;
        if pcm.len() < required_samples {
            return Err(format!(
                "Opus frame needs {required_samples} samples, received {}",
                pcm.len()
            ));
        }
        let frame_size = c_int::try_from(frame_samples_per_channel)
            .map_err(|_| "Opus frame is too large".to_string())?;
        let packet_capacity = c_int::try_from(packet.len())
            .map_err(|_| "Opus packet buffer is too large".to_string())?;
        if packet.is_empty() {
            return Err("Opus packet buffer is empty".to_string());
        }

        // SAFETY: both slices remain alive for the call and their lengths were
        // checked above. libopus writes at most `packet_capacity` bytes.
        let encoded = unsafe {
            opus_encode(
                self.inner.as_ptr(),
                pcm.as_ptr(),
                frame_size,
                packet.as_mut_ptr(),
                packet_capacity,
            )
        };
        if encoded < 0 {
            return Err(error_message(encoded));
        }
        Ok(encoded as usize)
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        // SAFETY: `inner` came from `opus_encoder_create` and is dropped once.
        unsafe { opus_encoder_destroy(self.inner.as_ptr()) };
    }
}

pub(crate) fn version() -> String {
    // SAFETY: libopus returns a process-lifetime NUL-terminated string.
    let version = unsafe { opus_get_version_string() };
    if version.is_null() {
        return "libopus (unknown version)".to_string();
    }
    // SAFETY: the non-null pointer is owned by libopus for the process lifetime.
    unsafe { CStr::from_ptr(version) }
        .to_string_lossy()
        .into_owned()
}

fn check(result: c_int) -> Result<(), String> {
    if result == OPUS_OK {
        Ok(())
    } else {
        Err(error_message(result))
    }
}

fn error_message(error: c_int) -> String {
    // SAFETY: `opus_strerror` returns a process-lifetime NUL-terminated string
    // for every libopus status code.
    let message = unsafe { opus_strerror(error) };
    if message.is_null() {
        return format!("libopus error {error}");
    }
    // SAFETY: the non-null pointer is owned by libopus for the process lifetime.
    let message = unsafe { CStr::from_ptr(message) }.to_string_lossy();
    format!("{message} (libopus error {error})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_the_vendored_xiph_release() {
        assert_eq!(version(), "libopus 1.5.2");
    }

    #[test]
    fn encodes_and_decodes_mono_and_stereo_frames_via_vendored_libopus() {
        use std::f32::consts::{FRAC_PI_2, PI};

        const SAMPLE_RATE: u32 = 48_000;
        const FRAME_SAMPLES: usize = 960;
        const FRAME_COUNT: usize = 12;
        const TONE_HZ: f32 = 440.0;

        for channels in [1_u8, 2_u8] {
            let channel_count = channels as usize;
            let mut encoder = Encoder::new(SAMPLE_RATE, channels).unwrap();
            encoder.set_bitrate(64_000).unwrap();
            encoder.set_vbr(true).unwrap();
            encoder.set_lsb_depth(16).unwrap();
            let lookahead = encoder.lookahead().unwrap() as usize;

            let mut decoder = Decoder::new(channel_count).unwrap();
            let mut input_frame = vec![0_i16; FRAME_SAMPLES * channel_count];
            let mut packet = vec![0_u8; MAX_PACKET_BYTES];
            let mut decoded_frame = vec![0_f32; FRAME_SAMPLES * channel_count];
            let mut decoded_pcm = Vec::with_capacity(FRAME_COUNT * FRAME_SAMPLES * channel_count);

            for frame_idx in 0..FRAME_COUNT {
                for sample_idx in 0..FRAME_SAMPLES {
                    let t = (frame_idx * FRAME_SAMPLES + sample_idx) as f32 / SAMPLE_RATE as f32;
                    let phase = 2.0 * PI * TONE_HZ * t;
                    input_frame[sample_idx * channel_count] =
                        (16_000.0 * phase.sin()).round() as i16;
                    if channel_count == 2 {
                        input_frame[sample_idx * channel_count + 1] =
                            (16_000.0 * (phase + FRAC_PI_2).sin()).round() as i16;
                    }
                }

                let encoded_len = encoder
                    .encode_s16(&input_frame, FRAME_SAMPLES, &mut packet)
                    .unwrap();
                assert!(encoded_len > 0, "{channels}ch frame {frame_idx} empty packet");

                let decoded_frames = decoder
                    .decode(&packet[..encoded_len], &mut decoded_frame)
                    .unwrap();
                assert_eq!(decoded_frames, FRAME_SAMPLES);
                decoded_pcm.extend_from_slice(&decoded_frame[..decoded_frames * channel_count]);
            }

            let skip_frames = lookahead + FRAME_SAMPLES * 2;
            assert!(skip_frames < FRAME_COUNT * FRAME_SAMPLES);

            for channel in 0..channel_count {
                let samples: Vec<f32> = decoded_pcm
                    .chunks_exact(channel_count)
                    .skip(skip_frames)
                    .map(|frame| frame[channel])
                    .collect();
                assert!(samples.iter().all(|sample| sample.is_finite()));
                let peak = samples.iter().copied().map(f32::abs).fold(0.0_f32, f32::max);
                assert!(peak > 0.1, "{channels}ch ch{channel} peak {peak}");

                let crossings = samples
                    .windows(2)
                    .filter(|pair| (pair[0] >= 0.0) != (pair[1] >= 0.0))
                    .count();
                let expected = samples.len() as f32 * (TONE_HZ * 2.0) / SAMPLE_RATE as f32;
                let min_crossings = (expected * 0.75) as usize;
                let max_crossings = (expected * 1.25) as usize;
                assert!(
                    (min_crossings..=max_crossings).contains(&crossings),
                    "{channels}ch ch{channel} crossings {crossings} outside {min_crossings}..={max_crossings}"
                );
            }
        }
    }
}
