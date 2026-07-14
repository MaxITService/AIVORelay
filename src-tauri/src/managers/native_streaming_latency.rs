use super::model::NativeStreamingLatencyKind;
use crate::settings::NativeStreamingLatencyPreset;
use log::warn;
use transcribe_cpp::{
    sys, ExtSlot, Model, ParakeetBufferedStreamOptions, ParakeetStreamOptions, StreamExtension,
};

fn extension_for_kind(
    kind: NativeStreamingLatencyKind,
    preset: NativeStreamingLatencyPreset,
) -> Option<(u32, StreamExtension)> {
    if preset == NativeStreamingLatencyPreset::Accurate {
        return None;
    }

    match kind {
        NativeStreamingLatencyKind::ParakeetBuffered => {
            let (chunk_ms, right_ms) = match preset {
                NativeStreamingLatencyPreset::Fastest => (160, 160),
                NativeStreamingLatencyPreset::Fast => (160, 320),
                NativeStreamingLatencyPreset::Balanced => (560, 560),
                NativeStreamingLatencyPreset::Accurate => unreachable!(),
            };
            Some((
                sys::TRANSCRIBE_EXT_KIND_PARAKEET_BUFFERED_STREAM,
                StreamExtension::ParakeetBuffered(ParakeetBufferedStreamOptions {
                    left_ms: Some(5600),
                    chunk_ms: Some(chunk_ms),
                    right_ms: Some(right_ms),
                }),
            ))
        }
        NativeStreamingLatencyKind::Nemotron35CacheAware
        | NativeStreamingLatencyKind::NemotronSpeechCacheAware => {
            let att_context_right = match (kind, preset) {
                (_, NativeStreamingLatencyPreset::Fastest) => 0,
                (
                    NativeStreamingLatencyKind::Nemotron35CacheAware,
                    NativeStreamingLatencyPreset::Fast,
                ) => 3,
                (
                    NativeStreamingLatencyKind::NemotronSpeechCacheAware,
                    NativeStreamingLatencyPreset::Fast,
                ) => 1,
                (_, NativeStreamingLatencyPreset::Balanced) => 6,
                (_, NativeStreamingLatencyPreset::Accurate) => unreachable!(),
                (NativeStreamingLatencyKind::ParakeetBuffered, _) => unreachable!(),
            };
            Some((
                sys::TRANSCRIBE_EXT_KIND_PARAKEET_STREAM,
                StreamExtension::ParakeetStream(ParakeetStreamOptions {
                    att_context_right: Some(att_context_right),
                }),
            ))
        }
    }
}

pub fn stream_extension(
    model: &Model,
    model_id: &str,
    kind: Option<NativeStreamingLatencyKind>,
    preset: NativeStreamingLatencyPreset,
) -> Option<StreamExtension> {
    let kind = kind?;
    let (extension_kind, extension) = extension_for_kind(kind, preset)?;
    if model.accepts_ext(ExtSlot::Stream, extension_kind) {
        Some(extension)
    } else {
        warn!(
            "Native streaming latency preset {:?} ignored for model '{}': runtime rejected extension kind {:#x}",
            preset, model_id, extension_kind
        );
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accurate_uses_the_runtime_default() {
        assert!(extension_for_kind(
            NativeStreamingLatencyKind::ParakeetBuffered,
            NativeStreamingLatencyPreset::Accurate,
        )
        .is_none());
    }

    #[test]
    fn fast_tier_is_model_specific_for_nemotron() {
        let (_, nemotron_35) = extension_for_kind(
            NativeStreamingLatencyKind::Nemotron35CacheAware,
            NativeStreamingLatencyPreset::Fast,
        )
        .unwrap();
        let (_, nemotron_speech) = extension_for_kind(
            NativeStreamingLatencyKind::NemotronSpeechCacheAware,
            NativeStreamingLatencyPreset::Fast,
        )
        .unwrap();

        assert_eq!(
            nemotron_35,
            StreamExtension::ParakeetStream(ParakeetStreamOptions {
                att_context_right: Some(3),
            })
        );
        assert_eq!(
            nemotron_speech,
            StreamExtension::ParakeetStream(ParakeetStreamOptions {
                att_context_right: Some(1),
            })
        );
    }
}
