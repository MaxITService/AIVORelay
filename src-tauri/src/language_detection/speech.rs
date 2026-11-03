//! The speech the language detector hears: silence cut out by VAD, so a
//! window of N seconds is N seconds of speech.

use anyhow::Result;

use crate::audio_toolkit::vad::{VadFrame, VoiceActivityDetector};

/// The first `max_samples` of speech in `audio`, as `vad` keeps it.
/// Check cancellation between frames, including when silence never fills the window.
pub fn speech_head(
    vad: &mut dyn VoiceActivityDetector,
    audio: &[f32],
    max_samples: usize,
    check_cancelled: impl Fn() -> Result<()>,
) -> Result<Vec<f32>> {
    let mut speech = Vec::with_capacity(max_samples.min(audio.len()));
    for frame in audio.chunks_exact(vad.frame_samples()) {
        check_cancelled()?;
        if let VadFrame::Speech(kept) = vad.push_frame(frame)? {
            speech.extend_from_slice(kept);
            if speech.len() >= max_samples {
                break;
            }
        }
    }
    speech.truncate(max_samples);
    Ok(speech)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Frames whose first sample is positive are speech.
    struct SignVad;

    impl VoiceActivityDetector for SignVad {
        fn push_frame<'a>(&'a mut self, frame: &'a [f32]) -> Result<VadFrame<'a>> {
            Ok(if frame[0] > 0.0 {
                VadFrame::Speech(frame)
            } else {
                VadFrame::Noise
            })
        }

        fn frame_samples(&self) -> usize {
            2
        }
    }

    #[test]
    fn keeps_only_speech_up_to_the_window() {
        let audio = [0.0, 0.0, 1.0, 2.0, 0.0, 0.0, 3.0, 4.0, 5.0, 6.0];
        assert_eq!(
            speech_head(&mut SignVad, &audio, 3, || Ok(())).unwrap(),
            vec![1.0, 2.0, 3.0]
        );
        assert_eq!(
            speech_head(&mut SignVad, &audio, 100, || Ok(())).unwrap(),
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]
        );
        assert!(speech_head(&mut SignVad, &[0.0; 6], 4, || Ok(())).unwrap().is_empty());
    }

    #[test]
    fn cancellation_stops_scanning_silence_before_the_speech_window_is_filled() {
        let checks = std::cell::Cell::new(0);
        let result = speech_head(&mut SignVad, &[0.0; 100], 10, || {
            checks.set(checks.get() + 1);
            if checks.get() == 4 {
                anyhow::bail!("File transcription was cancelled");
            }
            Ok(())
        });
        assert_eq!(result.unwrap_err().to_string(), "File transcription was cancelled");
        assert_eq!(checks.get(), 4);
    }
}
