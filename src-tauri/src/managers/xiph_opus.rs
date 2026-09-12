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

extern "C" {
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
}
