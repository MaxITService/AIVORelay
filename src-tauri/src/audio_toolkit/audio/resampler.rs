use rubato::{FftFixedIn, Resampler};
use std::time::Duration;

// Make this a constant you can tweak
const RESAMPLER_CHUNK_SIZE: usize = 1024;

pub struct FrameResampler {
    resampler: Option<FftFixedIn<f32>>,
    chunk_in: usize,
    in_buf: Vec<f32>,
    frame_samples: usize,
    pending: Vec<f32>,
}

impl FrameResampler {
    pub fn new(in_hz: usize, out_hz: usize, frame_dur: Duration) -> Self {
        let frame_samples = ((out_hz as f64 * frame_dur.as_secs_f64()).round()) as usize;
        assert!(frame_samples > 0, "frame duration too short");

        // Use fixed chunk size instead of GCD-based
        let chunk_in = RESAMPLER_CHUNK_SIZE;

        let resampler = (in_hz != out_hz).then(|| {
            FftFixedIn::<f32>::new(in_hz, out_hz, chunk_in, 1, 1)
                .expect("Failed to create resampler")
        });

        Self {
            resampler,
            chunk_in,
            in_buf: Vec::with_capacity(chunk_in),
            frame_samples,
            pending: Vec::with_capacity(frame_samples),
        }
    }

    pub fn push(&mut self, mut src: &[f32], mut emit: impl FnMut(&[f32])) {
        if self.resampler.is_none() {
            self.emit_frames(src, &mut emit);
            return;
        }

        while !src.is_empty() {
            let space = self.chunk_in - self.in_buf.len();
            let take = space.min(src.len());
            self.in_buf.extend_from_slice(&src[..take]);
            src = &src[take..];

            if self.in_buf.len() == self.chunk_in {
                // let start = std::time::Instant::now();
                if let Ok(out) = self
                    .resampler
                    .as_mut()
                    .unwrap()
                    .process(&[&self.in_buf[..]], None)
                {
                    // let duration = start.elapsed();
                    // log::debug!("Resampler took: {:?}", duration);
                    self.emit_frames(&out[0], &mut emit);
                }
                self.in_buf.clear();
            }
        }
    }

    pub fn finish(&mut self, mut emit: impl FnMut(&[f32])) {
        // Process any remaining input samples
        if let Some(ref mut resampler) = self.resampler {
            if !self.in_buf.is_empty() {
                // Pad with zeros to reach chunk size
                self.in_buf.resize(self.chunk_in, 0.0);
                if let Ok(out) = resampler.process(&[&self.in_buf[..]], None) {
                    self.emit_frames(&out[0], &mut emit);
                }
                self.in_buf.clear();
            }
        }

        // Emit any remaining pending frame (padded with zeros)
        if !self.pending.is_empty() {
            self.pending.resize(self.frame_samples, 0.0);
            emit(&self.pending);
            self.pending.clear();
        }
    }

    /// Clear all buffered audio and resampler history between recordings.
    pub fn reset(&mut self) {
        self.in_buf.clear();
        self.pending.clear();
        if let Some(ref mut resampler) = self.resampler {
            resampler.reset();
        }
    }

    fn emit_frames(&mut self, mut data: &[f32], emit: &mut impl FnMut(&[f32])) {
        while !data.is_empty() {
            let space = self.frame_samples - self.pending.len();
            let take = space.min(data.len());
            self.pending.extend_from_slice(&data[..take]);
            data = &data[take..];

            if self.pending.len() == self.frame_samples {
                emit(&self.pending);
                self.pending.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect_recording(resampler: &mut FrameResampler, input: &[f32]) -> Vec<f32> {
        let mut output = Vec::new();
        resampler.push(input, |frame| output.extend_from_slice(frame));
        resampler.finish(|frame| output.extend_from_slice(frame));
        output
    }

    #[test]
    fn reset_clears_wrapper_buffers() {
        let mut resampling = FrameResampler::new(48_000, 16_000, Duration::from_millis(30));
        resampling.push(&[1.0; 500], |_| panic!("partial chunk emitted"));
        assert_eq!(resampling.in_buf.len(), 500);

        resampling.reset();
        assert!(resampling.in_buf.is_empty());

        let mut passthrough = FrameResampler::new(16_000, 16_000, Duration::from_millis(30));
        passthrough.push(&[1.0; 200], |_| panic!("partial frame emitted"));
        assert_eq!(passthrough.pending.len(), 200);

        passthrough.reset();
        assert!(passthrough.pending.is_empty());
    }

    #[test]
    fn reset_makes_reused_resampler_match_fresh_resampler() {
        let mut reused = FrameResampler::new(48_000, 16_000, Duration::from_millis(30));
        let previous_recording = vec![1.0; RESAMPLER_CHUNK_SIZE * 4];
        assert!(!collect_recording(&mut reused, &previous_recording).is_empty());

        reused.reset();

        let next_recording = vec![0.0; RESAMPLER_CHUNK_SIZE * 4];
        let reused_output = collect_recording(&mut reused, &next_recording);

        let mut fresh = FrameResampler::new(48_000, 16_000, Duration::from_millis(30));
        let fresh_output = collect_recording(&mut fresh, &next_recording);

        assert_eq!(reused_output.len(), fresh_output.len());
        assert!(
            reused_output
                .iter()
                .zip(&fresh_output)
                .all(|(reused, fresh)| (reused - fresh).abs() <= f32::EPSILON),
            "reset resampler retained audio from the previous recording"
        );
    }

    #[test]
    fn finish_does_not_leak_tail_into_next_session() {
        let mut resampler = FrameResampler::new(48_000, 16_000, Duration::from_millis(30));

        resampler.push(&[0.5; 100], |_| {});
        resampler.finish(|_| {});

        let mut emitted = 0usize;
        resampler.push(&[0.25; RESAMPLER_CHUNK_SIZE], |frame| {
            emitted += frame.len()
        });

        assert_eq!(
            emitted, 0,
            "stale resampler tail from finish() leaked into the next session"
        );
    }
}
