//! Policy and runtime support for selecting a spoken language before transcription.

pub mod policy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageDetectionMode {
    WhenNeeded,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageSource {
    Selected,
    Model,
    Detected,
    Fallback,
}
