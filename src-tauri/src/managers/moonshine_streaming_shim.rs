use transcribe_cpp::{CommitPolicy, StreamOptions};

const MOONSHINE_STREAMING_ARCHITECTURE: &str = "moonshine_streaming";

/// Describes whether a streaming text consumer can replace an earlier
/// hypothesis or has already made the text irreversible outside the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommittedTextSink {
    ReplaceablePreview,
    IrreversibleAppendOnly,
}

/// Applies Moonshine's stream policy without leaking its re-attending model
/// semantics into preview, clipboard, or history callers.
pub fn configure_stream_options(
    architecture: &str,
    sink: CommittedTextSink,
    mut options: StreamOptions,
) -> StreamOptions {
    if architecture == MOONSHINE_STREAMING_ARCHITECTURE
        && sink == CommittedTextSink::IrreversibleAppendOnly
    {
        // Moonshine can revise text that previously satisfied stable-prefix
        // agreement. An append-only consumer cannot safely receive such text,
        // so expose no committed prefix until the authoritative final decode.
        options.commit_policy = CommitPolicy::OnFinalize;
    }

    options
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moonshine_defers_commits_for_irreversible_output() {
        let options = configure_stream_options(
            MOONSHINE_STREAMING_ARCHITECTURE,
            CommittedTextSink::IrreversibleAppendOnly,
            StreamOptions::default(),
        );

        assert_eq!(options.commit_policy, CommitPolicy::OnFinalize);
    }

    #[test]
    fn moonshine_keeps_auto_policy_for_replaceable_preview() {
        let options = configure_stream_options(
            MOONSHINE_STREAMING_ARCHITECTURE,
            CommittedTextSink::ReplaceablePreview,
            StreamOptions::default(),
        );

        assert_eq!(options.commit_policy, CommitPolicy::Auto);
    }

    #[test]
    fn other_architectures_keep_their_existing_policy() {
        let options = configure_stream_options(
            "voxtral_realtime",
            CommittedTextSink::IrreversibleAppendOnly,
            StreamOptions {
                commit_policy: CommitPolicy::StablePrefix,
                stable_prefix_agreement_n: 32,
                ..Default::default()
            },
        );

        assert_eq!(options.commit_policy, CommitPolicy::StablePrefix);
        assert_eq!(options.stable_prefix_agreement_n, 32);
    }
}
