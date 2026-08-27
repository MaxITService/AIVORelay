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
    in_hz: usize,
    out_hz: usize,
    in_count: usize,
    out_count: usize,
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
            in_hz,
            out_hz,
            in_count: 0,
            out_count: 0,
        }
    }

    pub fn push(&mut self, mut src: &[f32], mut emit: impl FnMut(&[f32])) {
        if self.resampler.is_none() {
            self.emit_frames(src, &mut emit);
            return;
        }
        self.in_count = self.in_count.saturating_add(src.len());

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
                    self.out_count = self.out_count.saturating_add(out[0].len());
                    self.emit_frames(&out[0], &mut emit);
                }
                self.in_buf.clear();
            }
        }
    }

    pub fn finish(&mut self, mut emit: impl FnMut(&[f32])) {
        if self.resampler.is_some() && self.in_count > 0 {
            let delay = self.resampler.as_ref().unwrap().output_delay();
            let expected_output =
                scale_frames(self.in_count, self.in_hz, self.out_hz).saturating_add(delay);

            // Rubato pads the last incomplete input chunk internally. Emit
            // only the part belonging to real input plus the filter delay.
            if !self.in_buf.is_empty() {
                let result = self
                    .resampler
                    .as_mut()
                    .unwrap()
                    .process_partial(Some(&[&self.in_buf[..]]), None);
                match result {
                    Ok(out) => {
                        let take = expected_output
                            .saturating_sub(self.out_count)
                            .min(out[0].len());
                        self.out_count = self.out_count.saturating_add(take);
                        self.emit_frames(&out[0][..take], &mut emit);
                    }
                    Err(error) => log::warn!("Failed to process final resampler input: {error}"),
                }
                self.in_buf.clear();
            }

            // The filter delays real output. Feed bounded zero chunks until
            // every expected delayed sample has emerged, trimming padding.
            if self.out_count < expected_output {
                let rounds = drain_round_limit(
                    expected_output - self.out_count,
                    delay,
                    self.in_hz,
                    self.out_hz,
                    self.chunk_in,
                );
                for _ in 0..rounds {
                    if self.out_count >= expected_output {
                        break;
                    }

                    match self
                        .resampler
                        .as_mut()
                        .unwrap()
                        .process_partial::<&[f32]>(None, None)
                    {
                        Ok(out) => {
                            let take = expected_output
                                .saturating_sub(self.out_count)
                                .min(out[0].len());
                            self.out_count = self.out_count.saturating_add(take);
                            self.emit_frames(&out[0][..take], &mut emit);
                        }
                        Err(error) => {
                            log::warn!("Failed to drain resampler delay: {error}");
                            break;
                        }
                    }
                }

                if self.out_count < expected_output {
                    log::warn!(
                        "Resampler delay drain stopped {} sample(s) early",
                        expected_output - self.out_count
                    );
                }
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
        self.in_count = 0;
        self.out_count = 0;
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

fn scale_frames(frames: usize, in_hz: usize, out_hz: usize) -> usize {
    let scaled = (frames as u128)
        .saturating_mul(out_hz as u128)
        .checked_div(in_hz as u128)
        .unwrap_or(0);
    scaled.min(usize::MAX as u128) as usize
}

fn scale_frames_ceil(frames: usize, in_hz: usize, out_hz: usize) -> usize {
    let numerator = (frames as u128).saturating_mul(out_hz as u128);
    let denominator = in_hz as u128;
    let scaled = numerator
        .saturating_add(denominator.saturating_sub(1))
        .checked_div(denominator)
        .unwrap_or(0);
    scaled.min(usize::MAX as u128) as usize
}

fn drain_round_limit(
    remaining_output: usize,
    output_delay: usize,
    in_hz: usize,
    out_hz: usize,
    chunk_in: usize,
) -> usize {
    // FftFixedIn's filter span is twice its reported output delay. Convert
    // both the missing output and that span back to input frames. This covers
    // unusual rates where one or more zero-input calls emit no samples.
    let missing_input = scale_frames_ceil(remaining_output, out_hz, in_hz);
    let filter_input = scale_frames_ceil(output_delay.saturating_mul(2), out_hz, in_hz);
    missing_input
        .saturating_add(filter_input)
        .div_ceil(chunk_in)
        .saturating_add(1)
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

    fn assert_tail_burst_flushed(in_hz: usize, input_len: usize, expected_out: usize) {
        let mut resampler = FrameResampler::new(in_hz, 16_000, Duration::from_millis(30));
        let mut input = vec![0.0f32; input_len];
        input[input_len - 200..].fill(0.5);

        let output = collect_recording(&mut resampler, &input);
        let max_abs = output.iter().map(|sample| sample.abs()).fold(0.0, f32::max);

        assert!(
            max_abs > 0.3,
            "tail burst was lost in the resampler, max_abs={max_abs}"
        );
        assert_eq!(output.len(), expected_out);
    }

    #[test]
    fn finish_flushes_resampler_delay() {
        assert_tail_burst_flushed(48_000, 4 * RESAMPLER_CHUNK_SIZE, 1920);
    }

    #[test]
    fn finish_flushes_resampler_delay_at_44100_hz() {
        assert_tail_burst_flushed(44_100, 4 * RESAMPLER_CHUNK_SIZE, 1920);
    }

    #[test]
    fn finish_flushes_unaligned_tail() {
        assert_tail_burst_flushed(48_000, 4 * RESAMPLER_CHUNK_SIZE + 300, 1920);
    }

    #[test]
    fn finish_trims_padding_when_upsampling_short_input() {
        let mut resampler = FrameResampler::new(8_000, 16_000, Duration::from_millis(30));
        let output = collect_recording(&mut resampler, &[0.5; 100]);

        // 200 converted samples plus 1024 samples of filter delay, padded to
        // complete 480-sample frames. Processing a whole zero-padded input
        // chunk would incorrectly emit 2400 samples here.
        assert_eq!(output.len(), 1440);
    }

    #[test]
    fn finish_without_input_emits_nothing() {
        let mut resampler = FrameResampler::new(48_000, 16_000, Duration::from_millis(30));
        let mut output = Vec::new();

        resampler.finish(|frame| output.extend_from_slice(frame));

        assert!(output.is_empty());
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
