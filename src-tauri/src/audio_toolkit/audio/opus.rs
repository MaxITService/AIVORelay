//! Streaming Opus input using Symphonia's containers and our vendored libopus.
//! Other codecs continue through the existing Rodio decoder.

use std::fs::File;
use std::path::Path;

use symphonia::core::{
    codecs::CODEC_TYPE_OPUS,
    errors::Error,
    formats::{FormatOptions, FormatReader},
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};

use crate::managers::xiph_opus::Decoder;

pub(crate) const SAMPLE_RATE: u32 = 48_000;
// A packet can contain 120 ms, including stereo packets larger than 5,760 samples.
const MAX_FRAMES: usize = 5_760;

pub(crate) struct OpusInput {
    format: Box<dyn FormatReader>,
    track_id: u32,
    decoder: Decoder,
    pub(crate) channels: usize,
    pub(crate) duration: Option<f64>,
    skip: usize,
    gain: f32,
    pcm: Vec<f32>,
    offset: usize,
    end: usize,
    finished: bool,
}

impl OpusInput {
    /// None means this is another codec in an otherwise supported container.
    pub(crate) fn open(path: &Path) -> Result<Option<Self>, String> {
        let extension = path.extension().and_then(|value| value.to_str())
            .unwrap_or_default().to_lowercase();
        if !matches!(extension.as_str(), "ogg" | "oga" | "opus" | "webm") {
            return Ok(None);
        }
        let file = File::open(path).map_err(|error| format!("Failed to open audio: {error}"))?;
        let mut hint = Hint::new();
        hint.with_extension(&extension);
        let options = FormatOptions { enable_gapless: true, ..Default::default() };
        let probed = symphonia::default::get_probe().format(
            &hint, MediaSourceStream::new(Box::new(file), Default::default()),
            &options, &MetadataOptions::default(),
        ).map_err(|error| format!("Failed to inspect audio: {error}"))?;
        let Some(track) = probed.format.tracks().iter()
            .find(|track| track.codec_params.codec == CODEC_TYPE_OPUS) else {
            return Ok(None);
        };
        let params = &track.codec_params;
        let header = params.extra_data.as_deref()
            .ok_or_else(|| "Opus identification header is missing".to_string())?;
        let (channels, pre_skip, gain) = parse_header(header)?;
        let duration = params.time_base.zip(params.n_frames).map(|(base, frames)| {
            let time = base.calc_time(frames);
            time.seconds as f64 + time.frac
        });
        let track_id = track.id;
        // Ogg's gapless demuxer emits delay as packet.trim_start(). Matroska
        // leaves delay unset, so its OpusHead pre-skip must be applied here.
        let skip = if params.delay.is_some() { 0 } else { pre_skip };
        Ok(Some(Self {
            format: probed.format, track_id, decoder: Decoder::new(channels)?,
            channels, duration, skip, gain, pcm: vec![0.0; MAX_FRAMES * channels],
            offset: 0, end: 0, finished: false,
        }))
    }

    fn read_packet(&mut self) -> Result<bool, String> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(Error::IoError(error)) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(false),
                Err(error) => return Err(format!("Failed to read Opus packet: {error}")),
            };
            if packet.track_id() != self.track_id { continue; }
            let frames = self.decoder.decode(&packet.data, &mut self.pcm)?;
            self.skip = self.skip.saturating_add(packet.trim_start() as usize);
            let start = self.skip.min(frames);
            self.skip -= start;
            let end = frames.saturating_sub(packet.trim_end() as usize).max(start);
            self.offset = start * self.channels;
            self.end = end * self.channels;
            if self.offset < self.end { return Ok(true); }
        }
    }
}

impl Iterator for OpusInput {
    type Item = Result<f32, String>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished { return None; }
        if self.offset == self.end {
            match self.read_packet() {
                Ok(true) => {}
                Ok(false) => { self.finished = true; return None; }
                Err(error) => { self.finished = true; return Some(Err(error)); }
            }
        }
        let sample = self.pcm[self.offset] * self.gain;
        self.offset += 1;
        Some(Ok(sample))
    }
}

fn parse_header(header: &[u8]) -> Result<(usize, usize, f32), String> {
    if header.len() < 19 || &header[..8] != b"OpusHead" || header[8] > 15 {
        return Err("Invalid Opus identification header".to_string());
    }
    let channels = header[9] as usize;
    if !(1..=2).contains(&channels) || header[18] != 0 {
        return Err("Only mono/stereo Opus with channel mapping family 0 is supported".to_string());
    }
    let pre_skip = u16::from_le_bytes([header[10], header[11]]) as usize;
    let gain_db = i16::from_le_bytes([header[16], header[17]]) as f32 / 256.0;
    Ok((channels, pre_skip, 10.0_f32.powf(gain_db / 20.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_upstream_mono_and_stereo_120ms_fixtures() {
        for (name, channels) in [("tone_440hz_mono.opus", 1), ("tone_440hz_stereo_120ms.webm", 2)] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
            let source = OpusInput::open(&path).unwrap().unwrap();
            assert_eq!(source.channels, channels);
            assert!(source.duration.is_some_and(|duration| (0.99..1.2).contains(&duration)));
            let pcm = source.collect::<Result<Vec<_>, _>>().unwrap();
            let mono: Vec<f32> = pcm.chunks_exact(channels)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32).collect();
            assert!((48_000..53_760).contains(&mono.len()), "{name}: {} frames", mono.len());
            assert!(mono.iter().all(|sample| sample.is_finite()));
            assert!(mono.iter().copied().map(f32::abs).fold(0.0, f32::max) > 0.25);
            let crossings = mono[4_800..43_200].windows(2)
                .filter(|pair| (pair[0] >= 0.0) != (pair[1] >= 0.0)).count();
            assert!((690..=720).contains(&crossings), "{name}: {crossings} crossings");
        }
    }
}
