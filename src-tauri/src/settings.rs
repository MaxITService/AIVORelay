use log::{debug, warn};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use specta::Type;
use std::collections::HashMap;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

pub const APPLE_INTELLIGENCE_PROVIDER_ID: &str = "apple_intelligence";
pub const APPLE_INTELLIGENCE_DEFAULT_MODEL_ID: &str = "Apple Intelligence";

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

// Custom deserializer to handle both old numeric format (1-5) and new string format ("trace", "debug", etc.)
impl<'de> Deserialize<'de> for LogLevel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LogLevelVisitor;

        impl<'de> Visitor<'de> for LogLevelVisitor {
            type Value = LogLevel;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or integer representing log level")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<LogLevel, E> {
                match value.to_lowercase().as_str() {
                    "trace" => Ok(LogLevel::Trace),
                    "debug" => Ok(LogLevel::Debug),
                    "info" => Ok(LogLevel::Info),
                    "warn" => Ok(LogLevel::Warn),
                    "error" => Ok(LogLevel::Error),
                    _ => Err(E::unknown_variant(
                        value,
                        &["trace", "debug", "info", "warn", "error"],
                    )),
                }
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<LogLevel, E> {
                match value {
                    1 => Ok(LogLevel::Trace),
                    2 => Ok(LogLevel::Debug),
                    3 => Ok(LogLevel::Info),
                    4 => Ok(LogLevel::Warn),
                    5 => Ok(LogLevel::Error),
                    _ => Err(E::invalid_value(de::Unexpected::Unsigned(value), &"1-5")),
                }
            }
        }

        deserializer.deserialize_any(LogLevelVisitor)
    }
}

impl From<LogLevel> for tauri_plugin_log::LogLevel {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => tauri_plugin_log::LogLevel::Trace,
            LogLevel::Debug => tauri_plugin_log::LogLevel::Debug,
            LogLevel::Info => tauri_plugin_log::LogLevel::Info,
            LogLevel::Warn => tauri_plugin_log::LogLevel::Warn,
            LogLevel::Error => tauri_plugin_log::LogLevel::Error,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct ShortcutBinding {
    pub id: String,
    pub name: String,
    pub description: String,
    pub default_binding: String,
    pub current_binding: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct LLMPrompt {
    pub id: String,
    pub name: String,
    pub prompt: String,
}

/// Per-profile LLM post-processing settings.
/// Used as a parameter struct for update_transcription_profile to reduce argument count.
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProfileLlmSettings {
    pub enabled: bool,
    pub prompt_override: Option<String>,
    pub model_override: Option<String>,
}

/// A custom transcription profile with its own language and translation settings.
/// Each profile creates a separate shortcut binding (e.g., "transcribe_profile_abc123").
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct TranscriptionProfile {
    /// Unique identifier (e.g., "profile_1704067200000")
    pub id: String,
    /// User-friendly name (e.g., "French to English", "Spanish Native")
    pub name: String,
    /// Language code for speech recognition (e.g., "fr", "es", "auto")
    pub language: String,
    /// Whether to translate the transcription to English
    pub translate_to_english: bool,
    /// Optional description shown in UI
    #[serde(default)]
    pub description: String,
    /// Optional system prompt for STT models (context hints, terminology, etc.)
    /// Character limits are enforced based on the active model (e.g., Whisper: 896 chars)
    #[serde(default)]
    pub system_prompt: String,
    /// Whether to override the global per-model STT prompt with this profile's system_prompt.
    /// When true, uses system_prompt (even if empty) instead of global transcription_prompts.
    /// When false, falls back to global per-model prompt.
    #[serde(default)]
    pub stt_prompt_override_enabled: bool,
    /// Whether this profile participates in the cycle shortcut rotation
    #[serde(default = "default_true")]
    pub include_in_cycle: bool,
    /// Push-to-talk mode for this profile (hold key to record vs toggle)
    #[serde(default = "default_true")]
    pub push_to_talk: bool,
    // ==================== LLM Post-Processing Settings ====================
    /// Whether LLM post-processing is enabled for this profile
    /// Inherits from global post_process_enabled when profile is created
    #[serde(default)]
    pub llm_post_process_enabled: bool,
    /// Override the global LLM system prompt for this profile
    /// If Some, uses this text instead of the global selected prompt
    #[serde(default)]
    pub llm_prompt_override: Option<String>,
    /// Override the global LLM model for this profile
    /// If Some, uses this model instead of the global model for the current provider
    #[serde(default)]
    pub llm_model_override: Option<String>,
}

impl TranscriptionProfile {
    /// Resolves the STT prompt based on profile override settings.
    /// Returns the profile's system_prompt if override is enabled, otherwise None
    /// (caller should fall back to global prompt).
    pub fn resolve_prompt(&self) -> Option<String> {
        if self.stt_prompt_override_enabled {
            let trimmed = self.system_prompt.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(self.system_prompt.clone())
            }
        } else {
            None
        }
    }
}

/// Resolves the STT prompt to use for transcription.
/// - If profile exists and has override enabled: uses profile's prompt (or None if empty)
/// - Otherwise: uses the global per-model prompt from transcription_prompts
pub fn resolve_stt_prompt(
    profile: Option<&TranscriptionProfile>,
    transcription_prompts: &HashMap<String, String>,
    model_id: &str,
) -> Option<String> {
    if let Some(p) = profile {
        if p.stt_prompt_override_enabled {
            // Profile overrides global prompt - use profile's prompt (even if empty)
            return p.resolve_prompt();
        }
    }
    // No profile or no override - fall back to global per-model prompt
    transcription_prompts
        .get(model_id)
        .filter(|p| !p.trim().is_empty())
        .cloned()
}

/// PowerShell execution policy for voice commands.
/// Controls script execution permissions.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPolicy {
    /// Use system default policy (no -ExecutionPolicy flag)
    Default,
    /// Bypass all restrictions (recommended for scripts)
    Bypass,
    /// No restrictions on local scripts, remote scripts require signature
    Unrestricted,
    /// Remote scripts require signature
    RemoteSigned,
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        ExecutionPolicy::Bypass
    }
}

/// Global default settings for voice command execution.
/// These settings are used for new commands and LLM fallback.
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct VoiceCommandDefaults {
    /// Silent execution (hidden window, non-interactive, output captured)
    #[serde(default = "default_true")]
    pub silent: bool,
    /// Skip profile loading (-NoProfile flag)
    #[serde(default)]
    pub no_profile: bool,
    /// Use PowerShell 7 (pwsh) instead of Windows PowerShell 5.1
    #[serde(default)]
    pub use_pwsh: bool,
    /// Execution policy for scripts
    #[serde(default)]
    pub execution_policy: ExecutionPolicy,
}

impl Default for VoiceCommandDefaults {
    fn default() -> Self {
        Self {
            silent: true,
            no_profile: false,
            use_pwsh: false,
            execution_policy: ExecutionPolicy::default(),
        }
    }
}

/// A voice command that triggers a script when the user speaks a matching phrase.
/// Used by the Voice Command Center feature for hands-free automation.
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct VoiceCommand {
    /// Unique identifier (e.g., "vc_1704067200000")
    pub id: String,
    /// User-friendly name shown in UI (e.g., "Lock Computer")
    pub name: String,
    /// The trigger phrase to match (e.g., "lock computer", "open browser")
    pub trigger_phrase: String,
    /// The script/command to execute (e.g., "rundll32.exe user32.dll,LockWorkStation")
    pub script: String,
    /// Similarity threshold for fuzzy matching (0.0-1.0, default 0.8)
    #[serde(default = "default_voice_command_threshold")]
    pub similarity_threshold: f64,
    /// Whether this command is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    // ==================== Execution Options ====================
    /// Silent execution (hidden window, non-interactive)
    #[serde(default = "default_true")]
    pub silent: bool,
    /// Skip profile loading (-NoProfile flag)
    #[serde(default)]
    pub no_profile: bool,
    /// Use PowerShell 7 (pwsh) instead of Windows PowerShell 5.1
    #[serde(default)]
    pub use_pwsh: bool,
    /// Execution policy (None = inherit from defaults)
    #[serde(default)]
    pub execution_policy: Option<ExecutionPolicy>,
    /// Working directory for this command (None = current directory)
    #[serde(default)]
    pub working_directory: Option<String>,
}

/// Resolved execution options for a voice command.
/// Used when actually executing the command.
#[derive(Debug, Clone)]
pub struct ResolvedExecutionOptions {
    pub silent: bool,
    pub no_profile: bool,
    pub use_pwsh: bool,
    pub execution_policy: ExecutionPolicy,
    pub working_directory: Option<String>,
}

impl VoiceCommand {
    /// Resolves execution options by merging command-level settings with global defaults.
    /// Command-level settings take priority over defaults.
    pub fn resolve_execution_options(
        &self,
        defaults: &VoiceCommandDefaults,
    ) -> ResolvedExecutionOptions {
        ResolvedExecutionOptions {
            silent: self.silent,
            no_profile: self.no_profile,
            use_pwsh: self.use_pwsh,
            // Use command's execution_policy if set, otherwise inherit from defaults
            execution_policy: self.execution_policy.unwrap_or(defaults.execution_policy),
            working_directory: self.working_directory.clone(),
        }
    }
}

impl VoiceCommandDefaults {
    /// Creates ResolvedExecutionOptions from defaults (for LLM fallback commands).
    pub fn to_resolved_options(&self) -> ResolvedExecutionOptions {
        ResolvedExecutionOptions {
            silent: self.silent,
            no_profile: self.no_profile,
            use_pwsh: self.use_pwsh,
            execution_policy: self.execution_policy,
            working_directory: None,
        }
    }
}

/// A text replacement rule that substitutes one text pattern with another.
/// Supports escape sequences for special characters (e.g., \n for newline).
/// Used to automatically fix common misheard phrases or apply consistent formatting.
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct TextReplacement {
    /// Unique identifier (e.g., "tr_1704067200000")
    pub id: String,
    /// The text pattern to search for (supports escape sequences: \n, \r\n, \t, \\)
    /// If is_regex is true, this is treated as a regular expression pattern.
    pub from: String,
    /// The replacement text (supports escape sequences: \n, \r\n, \t, \\)
    /// For regex replacements, supports $1, $2, etc. for capture groups.
    pub to: String,
    /// Whether this replacement rule is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Whether the match should be case-sensitive (default: true)
    #[serde(default = "default_true")]
    pub case_sensitive: bool,
    /// Whether the 'from' field is a regular expression (default: false)
    #[serde(default)]
    pub is_regex: bool,
}

impl TextReplacement {
    /// Processes escape sequences in a string.
    /// Converts: \\n -> \n, \\r\\n -> \r\n, \\t -> \t, \\\\ -> \\
    fn process_escapes(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.peek() {
                    Some('n') => {
                        result.push('\n');
                        chars.next();
                    }
                    Some('r') => {
                        chars.next();
                        // Check for \r\n sequence
                        if chars.peek() == Some(&'\\') {
                            let mut temp = chars.clone();
                            temp.next();
                            if temp.peek() == Some(&'n') {
                                result.push_str("\r\n");
                                chars.next(); // consume \
                                chars.next(); // consume n
                            } else {
                                result.push('\r');
                            }
                        } else {
                            result.push('\r');
                        }
                    }
                    Some('t') => {
                        result.push('\t');
                        chars.next();
                    }
                    Some('\\') => {
                        result.push('\\');
                        chars.next();
                    }
                    Some('u') => {
                        // Unicode escape: \u{XXXX} where XXXX is 1-6 hex digits
                        chars.next(); // consume 'u'
                        if chars.peek() == Some(&'{') {
                            chars.next(); // consume '{'
                            let mut hex_str = String::new();
                            while let Some(&ch) = chars.peek() {
                                if ch == '}' {
                                    chars.next(); // consume '}'
                                    break;
                                }
                                if ch.is_ascii_hexdigit() && hex_str.len() < 6 {
                                    hex_str.push(ch);
                                    chars.next();
                                } else {
                                    // Invalid character in hex sequence, abort
                                    break;
                                }
                            }
                            if let Ok(code_point) = u32::from_str_radix(&hex_str, 16) {
                                if let Some(unicode_char) = char::from_u32(code_point) {
                                    result.push(unicode_char);
                                } else {
                                    // Invalid code point, keep original sequence
                                    result.push_str("\\u{");
                                    result.push_str(&hex_str);
                                    result.push('}');
                                }
                            } else {
                                // Failed to parse hex, keep original
                                result.push_str("\\u{");
                                result.push_str(&hex_str);
                                result.push('}');
                            }
                        } else {
                            // No opening brace, keep \u as literal
                            result.push('\\');
                            result.push('u');
                        }
                    }
                    _ => {
                        // Keep the backslash if not a recognized escape
                        result.push(c);
                    }
                }
            } else {
                result.push(c);
            }
        }
        result
    }

    /// Applies this replacement rule to the given text.
    /// Returns the text with all occurrences of `from` replaced with `to`.
    pub fn apply(&self, text: &str) -> String {
        if !self.enabled || self.from.is_empty() {
            return text.to_string();
        }

        let to_processed = Self::process_escapes(&self.to);

        if self.is_regex {
            // Regex mode
            let pattern = if self.case_sensitive {
                self.from.clone()
            } else {
                format!("(?i){}", self.from)
            };

            match regex::Regex::new(&pattern) {
                Ok(re) => re.replace_all(text, to_processed.as_str()).to_string(),
                Err(e) => {
                    log::warn!(
                        "Invalid regex pattern '{}' in text replacement: {}",
                        self.from,
                        e
                    );
                    text.to_string()
                }
            }
        } else {
            // Plain text mode
            let from_processed = Self::process_escapes(&self.from);

            if self.case_sensitive {
                text.replace(&from_processed, &to_processed)
            } else {
                // Case-insensitive plain text replacement with Unicode-safe indexing
                replace_case_insensitive_literal(text, &from_processed, &to_processed)
            }
        }
    }
}

fn replace_case_insensitive_literal(text: &str, from: &str, to: &str) -> String {
    if from.is_empty() {
        return text.to_string();
    }

    let pattern = format!("(?i){}", regex::escape(from));
    match regex::Regex::new(&pattern) {
        Ok(re) => re.replace_all(text, regex::NoExpand(to)).into_owned(),
        Err(err) => {
            warn!(
                "Failed to build case-insensitive replacement regex for '{}': {}",
                from, err
            );
            text.to_string()
        }
    }
}

/// Applies all enabled text replacement rules to the given text.
pub fn apply_text_replacements(text: &str, replacements: &[TextReplacement]) -> String {
    let mut result = text.to_string();
    for replacement in replacements {
        if replacement.enabled {
            result = replacement.apply(&result);
        }
    }
    result
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct PostProcessProvider {
    pub id: String,
    pub label: String,
    pub base_url: String,
    #[serde(default)]
    pub allow_base_url_edit: bool,
    #[serde(default)]
    pub models_endpoint: Option<String>,
}

/// Which feature is requesting LLM access.
/// Used to resolve the correct provider/key/model configuration.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum LlmFeature {
    /// Post-processing of transcriptions
    PostProcessing,
    /// AI Replace selection feature
    AiReplace,
    /// Voice Command LLM fallback
    VoiceCommand,
}

/// Resolved LLM configuration for a specific feature.
/// Contains all information needed to make an LLM API call.
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct LlmConfig {
    pub provider_id: String,
    pub api_key: String,
    pub model: String,
    pub base_url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionProvider {
    Local,
    #[serde(rename = "remote_openai_compatible")]
    RemoteOpenAiCompatible,
    #[serde(rename = "remote_soniox")]
    RemoteSoniox,
}

pub const SONIOX_DEFAULT_MODEL: &str = "stt-rt-v4";
pub const SONIOX_DEFAULT_MAX_ENDPOINT_DELAY_MS: u32 = 2000;
pub const SONIOX_DEFAULT_LIVE_FINALIZE_TIMEOUT_MS: u32 = 500;

/// Shortcut engine selection for Windows.
/// Controls which mechanism is used to listen for global hotkeys.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutEngine {
    /// Use tauri-plugin-global-shortcut (high performance, limited key support)
    /// Does NOT support: Caps Lock, Num Lock, Scroll Lock, modifier-only shortcuts
    Tauri,
    /// Use rdev low-level hooks (all keys supported, higher CPU usage)
    /// Supports ALL keys including Caps Lock, Num Lock, and modifier-only shortcuts
    Rdev,
}

impl Default for ShortcutEngine {
    fn default() -> Self {
        // Default to Tauri for all platforms (better performance)
        // Users who need Caps Lock, Num Lock, or modifier-only shortcuts
        // can switch to rdev in Settings → Debug → Experimental Features
        ShortcutEngine::Tauri
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum RemoteSttDebugMode {
    Normal,
    Verbose,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct RemoteSttSettings {
    pub base_url: String,
    pub model_id: String,
    #[serde(default = "default_remote_stt_debug_capture")]
    pub debug_capture: bool,
    #[serde(default = "default_remote_stt_debug_mode")]
    pub debug_mode: RemoteSttDebugMode,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "lowercase")]
pub enum OverlayPosition {
    None,
    Top,
    Bottom,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ScreenshotCaptureMethod {
    ExternalProgram,
    Native,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum NativeRegionCaptureMode {
    /// Most performant: transparent picker over the live desktop.
    LiveDesktop,
    /// Legacy: capture a full screenshot first and use it as the picker background.
    ScreenshotBackground,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ModelUnloadTimeout {
    Never,
    Immediately,
    Min2,
    Min5,
    Min10,
    Min15,
    Hour1,
    Sec5, // Debug mode only
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum PasteMethod {
    CtrlV,
    Direct,
    None,
    ShiftInsert,
    CtrlShiftV,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardHandling {
    DontModify,
    CopyToClipboard,
    /// Experimental: Try to restore all clipboard formats including images, HTML, files (Windows-only)
    RestoreAdvanced,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum AutoSubmitKey {
    Enter,
    CtrlEnter,
    CmdEnter,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum RecordingRetentionPeriod {
    Never,
    PreserveLimit,
    Days3,
    Weeks2,
    Months3,
}

impl Default for ModelUnloadTimeout {
    fn default() -> Self {
        ModelUnloadTimeout::Never
    }
}

impl Default for PasteMethod {
    fn default() -> Self {
        // Default to CtrlV for macOS and Windows, Direct for Linux
        #[cfg(target_os = "linux")]
        return PasteMethod::Direct;
        #[cfg(not(target_os = "linux"))]
        return PasteMethod::CtrlV;
    }
}

impl Default for ClipboardHandling {
    fn default() -> Self {
        ClipboardHandling::DontModify
    }
}

impl Default for AutoSubmitKey {
    fn default() -> Self {
        AutoSubmitKey::Enter
    }
}

impl ModelUnloadTimeout {
    pub fn to_minutes(self) -> Option<u64> {
        match self {
            ModelUnloadTimeout::Never => None,
            ModelUnloadTimeout::Immediately => Some(0), // Special case for immediate unloading
            ModelUnloadTimeout::Min2 => Some(2),
            ModelUnloadTimeout::Min5 => Some(5),
            ModelUnloadTimeout::Min10 => Some(10),
            ModelUnloadTimeout::Min15 => Some(15),
            ModelUnloadTimeout::Hour1 => Some(60),
            ModelUnloadTimeout::Sec5 => Some(0), // Special case for debug - handled separately
        }
    }

    pub fn to_seconds(self) -> Option<u64> {
        match self {
            ModelUnloadTimeout::Never => None,
            ModelUnloadTimeout::Immediately => Some(0), // Special case for immediate unloading
            ModelUnloadTimeout::Sec5 => Some(5),
            _ => self.to_minutes().map(|m| m * 60),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum SoundTheme {
    Marimba,
    Pop,
    Custom,
}

impl SoundTheme {
    fn as_str(&self) -> &'static str {
        match self {
            SoundTheme::Marimba => "marimba",
            SoundTheme::Pop => "pop",
            SoundTheme::Custom => "custom",
        }
    }

    pub fn to_start_path(&self) -> String {
        format!("resources/{}_start.wav", self.as_str())
    }

    pub fn to_stop_path(&self) -> String {
        format!("resources/{}_stop.wav", self.as_str())
    }
}

/* still handy for composing the initial JSON in the store ------------- */
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct AppSettings {
    pub bindings: HashMap<String, ShortcutBinding>,
    pub push_to_talk: bool,
    pub audio_feedback: bool,
    #[serde(default = "default_audio_feedback_volume")]
    pub audio_feedback_volume: f32,
    #[serde(default = "default_sound_theme")]
    pub sound_theme: SoundTheme,
    #[serde(default = "default_start_hidden")]
    pub start_hidden: bool,
    #[serde(default = "default_autostart_enabled")]
    pub autostart_enabled: bool,
    #[serde(default = "default_show_tray_icon")]
    pub show_tray_icon: bool,
    #[serde(default = "default_update_checks_enabled")]
    pub update_checks_enabled: bool,
    #[serde(default = "default_model")]
    pub selected_model: String,
    #[serde(default = "default_transcription_provider")]
    pub transcription_provider: TranscriptionProvider,
    #[serde(default = "default_remote_stt_settings")]
    pub remote_stt: RemoteSttSettings,
    #[serde(default = "default_soniox_model")]
    pub soniox_model: String,
    #[serde(default = "default_soniox_timeout_seconds")]
    pub soniox_timeout_seconds: u32,
    #[serde(default = "default_soniox_live_enabled")]
    pub soniox_live_enabled: bool,
    #[serde(default = "default_soniox_language_hints")]
    pub soniox_language_hints: Vec<String>,
    #[serde(default = "default_false")]
    pub soniox_use_profile_language_hint_only: bool,
    #[serde(default = "default_false")]
    pub soniox_language_hints_strict: bool,
    #[serde(default = "default_true")]
    pub soniox_enable_endpoint_detection: bool,
    #[serde(default = "default_soniox_max_endpoint_delay_ms")]
    pub soniox_max_endpoint_delay_ms: u32,
    #[serde(default = "default_true")]
    pub soniox_enable_language_identification: bool,
    #[serde(default = "default_true")]
    pub soniox_enable_speaker_diarization: bool,
    #[serde(default = "default_soniox_keepalive_interval_seconds")]
    pub soniox_keepalive_interval_seconds: u32,
    #[serde(default = "default_soniox_live_finalize_timeout_ms")]
    pub soniox_live_finalize_timeout_ms: u32,
    #[serde(default = "default_false")]
    pub soniox_live_instant_stop: bool,
    #[serde(default = "default_false")]
    pub soniox_realtime_fuzzy_correction_enabled: bool,
    #[serde(default = "default_false")]
    pub soniox_realtime_keep_safety_buffer_enabled: bool,
    #[serde(default = "default_always_on_microphone")]
    pub always_on_microphone: bool,
    #[serde(default)]
    pub selected_microphone: Option<String>,
    #[serde(default)]
    pub clamshell_microphone: Option<String>,
    #[serde(default)]
    pub selected_output_device: Option<String>,
    #[serde(default = "default_translate_to_english")]
    pub translate_to_english: bool,
    #[serde(default = "default_selected_language")]
    pub selected_language: String,
    #[serde(default = "default_overlay_position")]
    pub overlay_position: OverlayPosition,
    #[serde(default = "default_debug_mode")]
    pub debug_mode: bool,
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
    #[serde(default)]
    pub custom_words: Vec<String>,
    #[serde(default = "default_custom_words_enabled")]
    pub custom_words_enabled: bool,
    #[serde(default = "default_custom_words_ngram_enabled")]
    pub custom_words_ngram_enabled: bool,
    #[serde(default)]
    pub model_unload_timeout: ModelUnloadTimeout,
    #[serde(default = "default_word_correction_threshold")]
    pub word_correction_threshold: f64,
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,
    #[serde(default = "default_recording_retention_period")]
    pub recording_retention_period: RecordingRetentionPeriod,
    #[serde(default)]
    pub paste_method: PasteMethod,
    #[serde(default = "default_paste_delay_ms")]
    pub paste_delay_ms: u64,
    /// Convert LF to CRLF before clipboard paste (fixes newlines on Windows)
    #[serde(default = "default_true")]
    pub convert_lf_to_crlf: bool,
    #[serde(default)]
    pub clipboard_handling: ClipboardHandling,
    #[serde(default = "default_auto_submit")]
    pub auto_submit: bool,
    #[serde(default)]
    pub auto_submit_key: AutoSubmitKey,
    #[serde(default = "default_post_process_enabled")]
    pub post_process_enabled: bool,
    #[serde(default = "default_post_process_provider_id")]
    pub post_process_provider_id: String,
    #[serde(default = "default_post_process_providers")]
    pub post_process_providers: Vec<PostProcessProvider>,
    #[serde(default = "default_post_process_api_keys")]
    pub post_process_api_keys: HashMap<String, String>,
    #[serde(default = "default_post_process_models")]
    pub post_process_models: HashMap<String, String>,
    #[serde(default = "default_post_process_prompts")]
    pub post_process_prompts: Vec<LLMPrompt>,
    #[serde(default)]
    pub post_process_selected_prompt_id: Option<String>,
    /// Whether ${short_prev_transcript} is enabled in LLM prompt templates.
    #[serde(default = "default_true")]
    pub llm_context_prev_transcript_enabled: bool,
    /// Max words retained for ${short_prev_transcript}.
    #[serde(default = "default_llm_context_prev_transcript_max_words")]
    pub llm_context_prev_transcript_max_words: usize,
    /// Expiry window (seconds) for ${short_prev_transcript}.
    #[serde(default = "default_llm_context_prev_transcript_expiry_seconds")]
    pub llm_context_prev_transcript_expiry_seconds: u64,
    #[serde(default = "default_ai_replace_system_prompt")]
    pub ai_replace_system_prompt: String,
    #[serde(default = "default_ai_replace_user_prompt")]
    pub ai_replace_user_prompt: String,
    #[serde(default = "default_ai_replace_max_chars")]
    pub ai_replace_max_chars: usize,
    #[serde(default = "default_ai_replace_allow_no_selection")]
    pub ai_replace_allow_no_selection: bool,
    #[serde(default = "default_ai_replace_no_selection_system_prompt")]
    pub ai_replace_no_selection_system_prompt: String,
    #[serde(default = "default_ai_replace_allow_quick_tap")]
    pub ai_replace_allow_quick_tap: bool,
    #[serde(default = "default_ai_replace_quick_tap_threshold_ms")]
    pub ai_replace_quick_tap_threshold_ms: u32,
    #[serde(default = "default_ai_replace_quick_tap_system_prompt")]
    pub ai_replace_quick_tap_system_prompt: String,
    /// AI Replace LLM provider ID (separate from post-processing)
    #[serde(default)]
    pub ai_replace_provider_id: Option<String>,
    /// AI Replace API keys per provider
    #[serde(default)]
    pub ai_replace_api_keys: HashMap<String, String>,
    /// AI Replace models per provider
    #[serde(default)]
    pub ai_replace_models: HashMap<String, String>,
    #[serde(default = "default_send_to_extension_with_selection_system_prompt")]
    pub send_to_extension_with_selection_system_prompt: String,
    #[serde(default = "default_send_to_extension_with_selection_user_prompt")]
    pub send_to_extension_with_selection_user_prompt: String,
    /// Whether the "Send Transcription to Extension" action is enabled (risky feature)
    #[serde(default)]
    pub send_to_extension_enabled: bool,
    #[serde(default = "default_true")]
    pub send_to_extension_push_to_talk: bool,
    /// Whether the "Send Transcription + Selection to Extension" action is enabled (risky feature)
    #[serde(default)]
    pub send_to_extension_with_selection_enabled: bool,
    #[serde(default = "default_true")]
    pub send_to_extension_with_selection_push_to_talk: bool,
    #[serde(default = "default_true")]
    pub send_to_extension_with_selection_allow_no_voice: bool,
    #[serde(default = "default_quick_tap_threshold_ms")]
    pub send_to_extension_with_selection_quick_tap_threshold_ms: u32,
    #[serde(default)]
    pub send_to_extension_with_selection_no_voice_system_prompt: String,
    #[serde(default = "default_true")]
    pub ai_replace_selection_push_to_talk: bool,
    #[serde(default)]
    pub mute_while_recording: bool,
    #[serde(default)]
    pub append_trailing_space: bool,
    #[serde(default = "default_filter_silence")]
    pub filter_silence: bool,
    #[serde(default = "default_connector_port")]
    pub connector_port: u16,
    #[serde(default = "default_connector_auto_open_enabled")]
    pub connector_auto_open_enabled: bool,
    #[serde(default = "default_connector_auto_open_url")]
    pub connector_auto_open_url: String,
    #[serde(default = "default_screenshot_capture_method")]
    pub screenshot_capture_method: ScreenshotCaptureMethod,
    #[serde(default = "default_native_region_capture_mode")]
    pub native_region_capture_mode: NativeRegionCaptureMode,
    #[serde(default = "default_screenshot_capture_command")]
    pub screenshot_capture_command: String,
    #[serde(default = "default_screenshot_folder")]
    pub screenshot_folder: String,
    #[serde(default = "default_true")]
    pub screenshot_require_recent: bool,
    #[serde(default = "default_screenshot_timeout_seconds")]
    pub screenshot_timeout_seconds: u32,
    #[serde(default)]
    pub screenshot_include_subfolders: bool,
    #[serde(default = "default_true")]
    pub screenshot_allow_no_voice: bool,
    #[serde(default = "default_quick_tap_threshold_ms")]
    pub screenshot_quick_tap_threshold_ms: u32,
    #[serde(default = "default_screenshot_no_voice_default_prompt")]
    pub screenshot_no_voice_default_prompt: String,
    /// Whether the "Send Transcription + Screenshot to Extension" action is enabled (risky feature)
    #[serde(default)]
    pub send_screenshot_to_extension_enabled: bool,
    #[serde(default = "default_true")]
    pub send_screenshot_to_extension_push_to_talk: bool,
    #[serde(default = "default_app_language")]
    pub app_language: String,
    #[serde(default = "default_connector_password")]
    pub connector_password: String,
    /// Whether the user explicitly set the connector password (disables auto-generation)
    #[serde(default)]
    pub connector_password_user_set: bool,
    /// Pending password awaiting acknowledgement from extension (two-phase commit)
    #[serde(default)]
    pub connector_pending_password: Option<String>,
    /// Per-model transcription prompts (model_id -> prompt text)
    /// For Whisper: context/terms prompt. For Parakeet: comma-separated boost words.
    #[serde(default)]
    pub transcription_prompts: HashMap<String, String>,
    /// Custom transcription profiles with per-profile language/translation settings.
    /// Each profile creates a dynamic shortcut binding.
    #[serde(default)]
    pub transcription_profiles: Vec<TranscriptionProfile>,
    /// ID of the currently active profile. "default" means use global settings.
    /// When the main "Transcribe" shortcut is pressed, this profile's settings are used.
    #[serde(default = "default_active_profile_id")]
    pub active_profile_id: String,
    /// Whether to show an overlay notification when switching profiles
    #[serde(default = "default_true")]
    pub profile_switch_overlay_enabled: bool,
    // ==================== Voice Command Center ====================
    /// Whether the Voice Command feature is enabled
    #[serde(default)]
    pub voice_command_enabled: bool,
    /// Push-to-talk mode for voice commands
    #[serde(default = "default_true")]
    pub voice_command_push_to_talk: bool,
    /// Predefined voice commands (trigger phrase -> script)
    #[serde(default)]
    pub voice_commands: Vec<VoiceCommand>,
    /// Default similarity threshold for fuzzy matching (0.0-1.0)
    #[serde(default = "default_voice_command_threshold")]
    pub voice_command_default_threshold: f64,
    /// Whether to use LLM fallback when no predefined command matches
    #[serde(default = "default_true")]
    pub voice_command_llm_fallback: bool,
    /// System prompt for LLM command generation
    #[serde(default = "default_voice_command_system_prompt")]
    pub voice_command_system_prompt: String,
    /// Default execution options for new voice commands and LLM fallback
    #[serde(default)]
    pub voice_command_defaults: VoiceCommandDefaults,
    // DEPRECATED: voice_command_template - kept for migration only
    #[serde(default)]
    pub voice_command_template: String,
    // DEPRECATED: voice_command_keep_window_open - kept for migration only
    #[serde(default)]
    pub voice_command_keep_window_open: bool,
    /// Whether to auto-run predefined commands after countdown (not LLM-generated)
    #[serde(default)]
    pub voice_command_auto_run: bool,
    /// Countdown seconds before auto-running predefined commands (1-10)
    #[serde(default = "default_voice_command_auto_run_seconds")]
    pub voice_command_auto_run_seconds: u32,
    // ==================== Extended Thinking / Reasoning ====================
    /// Whether to enable extended thinking (reasoning tokens) for post-processing LLM calls
    #[serde(default)]
    pub post_process_reasoning_enabled: bool,
    /// Token budget for post-processing extended thinking (min: 1024, default: 2048)
    #[serde(default = "default_reasoning_budget")]
    pub post_process_reasoning_budget: u32,
    /// Whether to enable extended thinking for AI Replace LLM calls
    #[serde(default)]
    pub ai_replace_reasoning_enabled: bool,
    /// Token budget for AI Replace extended thinking (min: 1024, default: 2048)
    #[serde(default = "default_reasoning_budget")]
    pub ai_replace_reasoning_budget: u32,
    // ==================== Voice Command LLM Settings ====================
    /// Voice Command LLM provider ID (separate from post-processing)
    #[serde(default)]
    pub voice_command_provider_id: Option<String>,
    /// Voice Command API keys per provider
    #[serde(default)]
    pub voice_command_api_keys: HashMap<String, String>,
    /// Voice Command models per provider
    #[serde(default)]
    pub voice_command_models: HashMap<String, String>,
    /// Whether to enable extended thinking for Voice Command LLM fallback
    #[serde(default)]
    pub voice_command_reasoning_enabled: bool,
    /// Token budget for Voice Command extended thinking (min: 1024, default: 2048)
    #[serde(default = "default_reasoning_budget")]
    pub voice_command_reasoning_budget: u32,
    // ==================== Voice Command Fuzzy Matching ====================
    /// Whether to use Levenshtein distance for character-level matching
    #[serde(default = "default_true")]
    pub voice_command_use_levenshtein: bool,
    /// Per-word Levenshtein threshold (0.0-1.0, lower = more tolerant of typos)
    #[serde(default = "default_voice_command_levenshtein_threshold")]
    pub voice_command_levenshtein_threshold: f64,
    /// Whether to use phonetic (Soundex) matching
    #[serde(default = "default_true")]
    pub voice_command_use_phonetic: bool,
    /// Phonetic match boost multiplier (0.0-1.0)
    #[serde(default = "default_voice_command_phonetic_boost")]
    pub voice_command_phonetic_boost: f64,
    /// Word similarity threshold - minimum score for a word pair to be considered matching
    #[serde(default = "default_voice_command_word_similarity_threshold")]
    pub voice_command_word_similarity_threshold: f64,
    // ==================== Beta Feature Flags ====================
    /// Whether Voice Commands beta feature is enabled in the UI (Debug menu toggle)
    #[serde(default = "default_true")]
    pub beta_voice_commands_enabled: bool,
    /// Whether to show the bottom always-on-top toggle row in the floating voice button window
    #[serde(default)]
    pub voice_button_show_aot_toggle: bool,
    /// Whether clicking the close "x" once should close the floating voice button window
    #[serde(default)]
    pub voice_button_single_click_close: bool,
    // ==================== Text Replacement ====================
    /// Whether text replacement feature is enabled globally
    #[serde(default)]
    pub text_replacements_enabled: bool,
    /// List of text replacement rules
    #[serde(default)]
    pub text_replacements: Vec<TextReplacement>,
    /// Whether to apply text replacements BEFORE LLM post-processing (default: after)
    /// When true: STT → Text Replacement → LLM → Output
    /// When false (default): STT → LLM → Text Replacement → Output
    #[serde(default)]
    pub text_replacements_before_llm: bool,
    /// Enable passive key monitoring to decapitalize the next inserted chunk after manual edits.
    #[serde(default)]
    pub text_replacement_decapitalize_after_edit_key_enabled: bool,
    /// Keyboard key (or shortcut) to monitor for manual edits. Default: backspace.
    #[serde(default = "default_text_replacement_decapitalize_after_edit_key")]
    pub text_replacement_decapitalize_after_edit_key: String,
    /// How long the decapitalize trigger remains active after the monitored key is pressed.
    #[serde(default = "default_text_replacement_decapitalize_timeout_ms")]
    pub text_replacement_decapitalize_timeout_ms: u32,
    /// For standard (non-realtime) STT only: how long after stop-recording
    /// we keep monitoring for the edit key to arm decapitalization on the next output.
    #[serde(default = "default_text_replacement_decapitalize_standard_post_recording_monitor_ms")]
    pub text_replacement_decapitalize_standard_post_recording_monitor_ms: u32,
    // ==================== Audio Processing ====================
    /// Whether to filter filler words (uh, um, hmm, etc.) from transcriptions
    #[serde(default)]
    pub filler_word_filter_enabled: bool,
    /// Whether to strip invisible Unicode characters (zero-width spaces, BOM) from LLM output
    #[serde(default = "default_true")]
    pub zero_width_filter_enabled: bool,
    /// Whether to trim leading/trailing whitespace from transcription outputs.
    #[serde(default = "default_true")]
    pub trim_transcription_output_enabled: bool,
    /// VAD (Voice Activity Detection) threshold for speech detection (0.1-0.9)
    /// Lower = more sensitive (captures quieter speech but may include noise)
    /// Higher = less sensitive (cleaner input but may cut off quiet speech)
    #[serde(default = "default_vad_threshold")]
    pub vad_threshold: f32,
    // ==================== Shortcut Engine (Windows only) ====================
    /// Which shortcut engine to use for global hotkeys (Windows only)
    /// - "tauri": High performance, but doesn't support Caps Lock, Num Lock, modifier-only shortcuts
    /// - "rdev": Supports all keys, but uses more CPU (processes every keystroke)
    #[serde(default)]
    pub shortcut_engine: ShortcutEngine,
    // ==================== UI State ====================
    /// Whether the hotkey sidebar is pinned open
    #[serde(default)]
    pub sidebar_pinned: bool,
    /// Width of the hotkey sidebar in pixels
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: u32,
}

fn default_sidebar_width() -> u32 {
    350
}

fn default_model() -> String {
    "".to_string()
}

fn default_transcription_provider() -> TranscriptionProvider {
    TranscriptionProvider::Local
}

fn default_remote_stt_debug_capture() -> bool {
    false
}

fn default_remote_stt_debug_mode() -> RemoteSttDebugMode {
    RemoteSttDebugMode::Normal
}

fn default_remote_stt_settings() -> RemoteSttSettings {
    RemoteSttSettings {
        base_url: "https://api.groq.com/openai/v1".to_string(),
        model_id: "whisper-large-v3-turbo".to_string(),
        debug_capture: default_remote_stt_debug_capture(),
        debug_mode: default_remote_stt_debug_mode(),
    }
}

fn default_soniox_model() -> String {
    SONIOX_DEFAULT_MODEL.to_string()
}

fn default_soniox_timeout_seconds() -> u32 {
    30
}

fn default_soniox_live_enabled() -> bool {
    true
}

fn default_soniox_language_hints() -> Vec<String> {
    vec!["en".to_string()]
}

fn default_soniox_max_endpoint_delay_ms() -> u32 {
    SONIOX_DEFAULT_MAX_ENDPOINT_DELAY_MS
}

fn default_soniox_keepalive_interval_seconds() -> u32 {
    10
}

fn default_soniox_live_finalize_timeout_ms() -> u32 {
    SONIOX_DEFAULT_LIVE_FINALIZE_TIMEOUT_MS
}

fn default_vad_threshold() -> f32 {
    0.3 // Original Handy default - more sensitive
}

fn default_always_on_microphone() -> bool {
    false
}

fn default_translate_to_english() -> bool {
    false
}

fn default_start_hidden() -> bool {
    false
}

fn default_autostart_enabled() -> bool {
    false
}

fn default_update_checks_enabled() -> bool {
    true
}

fn default_selected_language() -> String {
    "auto".to_string()
}

fn default_overlay_position() -> OverlayPosition {
    #[cfg(target_os = "linux")]
    return OverlayPosition::None;
    #[cfg(not(target_os = "linux"))]
    return OverlayPosition::Bottom;
}

fn default_debug_mode() -> bool {
    false
}

fn default_log_level() -> LogLevel {
    LogLevel::Error
}

fn default_word_correction_threshold() -> f64 {
    0.18
}

fn default_paste_delay_ms() -> u64 {
    60
}

fn default_auto_submit() -> bool {
    false
}

fn default_custom_words_enabled() -> bool {
    true
}

fn default_custom_words_ngram_enabled() -> bool {
    true
}

fn default_history_limit() -> usize {
    5
}

fn default_recording_retention_period() -> RecordingRetentionPeriod {
    RecordingRetentionPeriod::PreserveLimit
}

fn default_audio_feedback_volume() -> f32 {
    1.0
}

fn default_sound_theme() -> SoundTheme {
    SoundTheme::Marimba
}

fn default_post_process_enabled() -> bool {
    false
}

fn default_llm_context_prev_transcript_max_words() -> usize {
    200
}

fn default_llm_context_prev_transcript_expiry_seconds() -> u64 {
    300
}

fn default_app_language() -> String {
    tauri_plugin_os::locale()
        .and_then(|l| l.split(['-', '_']).next().map(String::from))
        .unwrap_or_else(|| "en".to_string())
}

fn default_show_tray_icon() -> bool {
    true
}

fn default_filter_silence() -> bool {
    true
}

fn default_connector_port() -> u16 {
    38243
}

fn default_connector_auto_open_enabled() -> bool {
    false
}

fn default_connector_auto_open_url() -> String {
    "".to_string()
}

fn default_screenshot_capture_method() -> ScreenshotCaptureMethod {
    ScreenshotCaptureMethod::Native
}

fn default_native_region_capture_mode() -> NativeRegionCaptureMode {
    NativeRegionCaptureMode::LiveDesktop
}

fn default_screenshot_capture_command() -> String {
    r#"& "C:\Program Files\ShareX\ShareX.exe" -RectangleRegion"#.to_string()
}

fn default_screenshot_folder() -> String {
    // Use %USERPROFILE%\Documents\ShareX\Screenshots as default (ShareX default location)
    // This will be expanded at runtime
    std::env::var("USERPROFILE")
        .map(|home| format!("{}\\Documents\\ShareX\\Screenshots", home))
        .unwrap_or_else(|_| "Documents\\ShareX\\Screenshots".to_string())
}

fn default_screenshot_timeout_seconds() -> u32 {
    5
}

fn default_screenshot_no_voice_default_prompt() -> String {
    "Look at this picture and provide a helpful response.".to_string()
}

fn default_quick_tap_threshold_ms() -> u32 {
    500
}

fn default_text_replacement_decapitalize_after_edit_key() -> String {
    "backspace".to_string()
}

fn default_text_replacement_decapitalize_timeout_ms() -> u32 {
    5000
}

fn default_text_replacement_decapitalize_standard_post_recording_monitor_ms() -> u32 {
    5000
}

fn default_voice_command_threshold() -> f64 {
    0.75
}

fn default_voice_command_auto_run_seconds() -> u32 {
    4
}

fn default_voice_command_levenshtein_threshold() -> f64 {
    0.3 // 30% of word length can be edits (typos)
}

fn default_voice_command_phonetic_boost() -> f64 {
    0.5 // Phonetic matches get 50% boost
}

fn default_voice_command_word_similarity_threshold() -> f64 {
    0.7 // Words must be 70% similar to match
}

fn default_voice_command_system_prompt() -> String {
    r#"You are a Windows command generator. The user will describe what they want to do, and you must generate a SINGLE PowerShell one-liner command that accomplishes it.

Rules:
1. Return ONLY the command, nothing else - no explanations, no markdown, no code blocks
2. The command must be a valid PowerShell one-liner that can run directly
3. Use Start-Process for launching applications
4. Use common Windows paths and commands
5. If the request is unclear or dangerous (like deleting system files), return: UNSAFE_REQUEST
6. Keep commands simple and safe

Example inputs and outputs:
- "open notepad" → Start-Process notepad
- "open chrome" → Start-Process chrome
- "lock the computer" → rundll32.exe user32.dll,LockWorkStation
- "open word and excel" → Start-Process winword; Start-Process excel
- "show my documents folder" → Start-Process explorer -ArgumentList "$env:USERPROFILE\Documents""#.to_string()
}

/// Default connector password - used for initial mutual authentication
pub fn default_connector_password() -> String {
    "fklejqwhfiu342lhk3".to_string()
}

/// Default reasoning token budget for Extended Thinking (OpenRouter)
fn default_reasoning_budget() -> u32 {
    2048
}

/// Default active profile ID - "default" means use global transcription settings
fn default_active_profile_id() -> String {
    "default".to_string()
}

fn default_post_process_provider_id() -> String {
    "openai".to_string()
}

fn default_ai_replace_system_prompt() -> String {
    "You are a text transformation engine.\nReturn ONLY the final transformed text that is ready to be pasted directly into another application.\nDo not include explanations, commentary, labels, headings, lists, markdown, code fences, or any surrounding quotes.\nPreserve the original language and keep the original formatting (line breaks, punctuation, and spacing) unless the instruction explicitly asks to change it.\nMake the smallest change that satisfies the instruction.\nIf the instruction conflicts with the text or is unclear, prefer minimal edits and do not invent new facts.".to_string()
}

fn default_ai_replace_user_prompt() -> String {
    "INSTRUCTION:\n${instruction}\n\nTEXT:\n${output}".to_string()
}

fn default_ai_replace_max_chars() -> usize {
    20000
}

fn default_ai_replace_allow_no_selection() -> bool {
    true
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_ai_replace_no_selection_system_prompt() -> String {
    "You are a helpful assistant.\nAnswer the user's instruction directly and concisely.\nDo not include any preamble (like 'Here is the answer') or postscript.\nJust provide the content requested.".to_string()
}

fn default_ai_replace_allow_quick_tap() -> bool {
    true
}

fn default_ai_replace_quick_tap_threshold_ms() -> u32 {
    500
}

fn default_ai_replace_quick_tap_system_prompt() -> String {
    "You are a text improvement engine.\nImprove the provided text while preserving its original meaning and intent.\nFix any grammar, spelling, or punctuation errors.\nEnhance clarity and readability where possible.\nReturn ONLY the improved text without any explanations or commentary.\nPreserve the original language and formatting unless fixing errors requires changes.".to_string()
}

fn default_send_to_extension_with_selection_system_prompt() -> String {
    String::new()
}

fn default_send_to_extension_with_selection_user_prompt() -> String {
    default_ai_replace_user_prompt()
}

fn default_post_process_providers() -> Vec<PostProcessProvider> {
    // mut is required on macOS where we push Apple Intelligence provider
    #[allow(unused_mut)]
    let mut providers = vec![
        PostProcessProvider {
            id: "openai".to_string(),
            label: "OpenAI".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            allow_base_url_edit: false,
            models_endpoint: Some("/models".to_string()),
        },
        PostProcessProvider {
            id: "openrouter".to_string(),
            label: "OpenRouter".to_string(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            allow_base_url_edit: false,
            models_endpoint: Some("/models".to_string()),
        },
        PostProcessProvider {
            id: "anthropic".to_string(),
            label: "Anthropic".to_string(),
            base_url: "https://api.anthropic.com/v1".to_string(),
            allow_base_url_edit: false,
            models_endpoint: Some("/models".to_string()),
        },
        PostProcessProvider {
            id: "groq".to_string(),
            label: "Groq".to_string(),
            base_url: "https://api.groq.com/openai/v1".to_string(),
            allow_base_url_edit: false,
            models_endpoint: Some("/models".to_string()),
        },
        PostProcessProvider {
            id: "cerebras".to_string(),
            label: "Cerebras".to_string(),
            base_url: "https://api.cerebras.ai/v1".to_string(),
            allow_base_url_edit: false,
            models_endpoint: Some("/models".to_string()),
        },
    ];

    // Note: We always include Apple Intelligence on macOS ARM64 without checking availability
    // at startup. The availability check is deferred to when the user actually tries to use it
    // (in actions.rs). This prevents crashes on macOS 26.x beta where accessing
    // SystemLanguageModel.default during early app initialization causes SIGABRT.
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        providers.push(PostProcessProvider {
            id: APPLE_INTELLIGENCE_PROVIDER_ID.to_string(),
            label: "Apple Intelligence".to_string(),
            base_url: "apple-intelligence://local".to_string(),
            allow_base_url_edit: false,
            models_endpoint: None,
        });
    }

    // Custom provider always comes last
    providers.push(PostProcessProvider {
        id: "custom".to_string(),
        label: "Custom".to_string(),
        base_url: "http://localhost:11434/v1".to_string(),
        allow_base_url_edit: true,
        models_endpoint: Some("/models".to_string()),
    });

    providers
}

fn default_post_process_api_keys() -> HashMap<String, String> {
    let mut map = HashMap::new();
    for provider in default_post_process_providers() {
        map.insert(provider.id, String::new());
    }
    map
}

fn default_model_for_provider(provider_id: &str) -> String {
    if provider_id == APPLE_INTELLIGENCE_PROVIDER_ID {
        return APPLE_INTELLIGENCE_DEFAULT_MODEL_ID.to_string();
    }
    String::new()
}

fn default_post_process_models() -> HashMap<String, String> {
    let mut map = HashMap::new();
    for provider in default_post_process_providers() {
        map.insert(
            provider.id.clone(),
            default_model_for_provider(&provider.id),
        );
    }
    map
}

fn default_post_process_prompts() -> Vec<LLMPrompt> {
    vec![LLMPrompt {
        id: "default_improve_transcriptions".to_string(),
        name: "Improve Transcriptions".to_string(),
        prompt: "Clean this transcript:\n1. Fix spelling, capitalization, and punctuation errors\n2. Convert number words to digits (twenty-five → 25, ten percent → 10%, five dollars → $5)\n3. Replace spoken punctuation with symbols (period → ., comma → ,, question mark → ?)\n4. Remove filler words (um, uh, like as filler)\n5. Keep the language in the original version (if it was french, keep it in french for example)\n\nPreserve exact meaning and word order. Do not paraphrase or reorder content.\n\nReturn only the cleaned transcript.\n\nTranscript:\n${output}".to_string(),
    }]
}

fn ensure_post_process_defaults(settings: &mut AppSettings) -> bool {
    let mut changed = false;
    for provider in default_post_process_providers() {
        if settings
            .post_process_providers
            .iter()
            .all(|existing| existing.id != provider.id)
        {
            settings.post_process_providers.push(provider.clone());
            changed = true;
        }

        if !settings.post_process_api_keys.contains_key(&provider.id) {
            settings
                .post_process_api_keys
                .insert(provider.id.clone(), String::new());
            changed = true;
        }

        let default_model = default_model_for_provider(&provider.id);
        match settings.post_process_models.get_mut(&provider.id) {
            Some(existing) => {
                if existing.is_empty() && !default_model.is_empty() {
                    *existing = default_model.clone();
                    changed = true;
                }
            }
            None => {
                settings
                    .post_process_models
                    .insert(provider.id.clone(), default_model);
                changed = true;
            }
        }
    }

    changed
}

pub const SETTINGS_STORE_PATH: &str = "settings_store.json";

pub fn get_default_settings() -> AppSettings {
    #[cfg(target_os = "windows")]
    let default_shortcut = "ctrl+space";
    #[cfg(target_os = "macos")]
    let default_shortcut = "option+space";
    #[cfg(target_os = "linux")]
    let default_shortcut = "ctrl+space";
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let default_shortcut = "alt+space";

    #[cfg(target_os = "windows")]
    let default_send_shortcut = "ctrl+alt+space";
    #[cfg(target_os = "macos")]
    let default_send_shortcut = "option+command+space";
    #[cfg(target_os = "linux")]
    let default_send_shortcut = "ctrl+alt+space";
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let default_send_shortcut = "alt+space";

    #[cfg(target_os = "windows")]
    let default_send_selection_shortcut = "ctrl+alt+shift+space";
    #[cfg(target_os = "macos")]
    let default_send_selection_shortcut = "option+command+shift+space";
    #[cfg(target_os = "linux")]
    let default_send_selection_shortcut = "ctrl+alt+shift+space";
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let default_send_selection_shortcut = "alt+shift+space";

    let mut bindings = HashMap::new();
    bindings.insert(
        "transcribe".to_string(),
        ShortcutBinding {
            id: "transcribe".to_string(),
            name: "Transcribe".to_string(),
            description: "Converts your speech into text.".to_string(),
            default_binding: default_shortcut.to_string(),
            current_binding: default_shortcut.to_string(),
        },
    );
    bindings.insert(
        "send_to_extension".to_string(),
        ShortcutBinding {
            id: "send_to_extension".to_string(),
            name: "Send to Extension".to_string(),
            description: "Send transcription to AivoRelay Connector.".to_string(),
            default_binding: default_send_shortcut.to_string(),
            current_binding: default_send_shortcut.to_string(),
        },
    );
    bindings.insert(
        "send_to_extension_with_selection".to_string(),
        ShortcutBinding {
            id: "send_to_extension_with_selection".to_string(),
            name: "Send + Selection to Extension".to_string(),
            description: "Send transcription plus copied selection to AivoRelay Connector."
                .to_string(),
            default_binding: default_send_selection_shortcut.to_string(),
            current_binding: default_send_selection_shortcut.to_string(),
        },
    );
    #[cfg(target_os = "windows")]
    bindings.insert(
        "ai_replace_selection".to_string(),
        ShortcutBinding {
            id: "ai_replace_selection".to_string(),
            name: "AI Replace Selection".to_string(),
            description:
                "Cut selected text, speak an instruction, replace selection with AI output"
                    .to_string(),
            default_binding: "ctrl+shift+space".to_string(),
            current_binding: "ctrl+shift+space".to_string(),
        },
    );
    #[cfg(target_os = "windows")]
    bindings.insert(
        "send_screenshot_to_extension".to_string(),
        ShortcutBinding {
            id: "send_screenshot_to_extension".to_string(),
            name: "Send Screenshot to Extension".to_string(),
            description:
                "Capture screenshot with voice instruction and send to AivoRelay Connector."
                    .to_string(),
            default_binding: "".to_string(),
            current_binding: "".to_string(),
        },
    );
    bindings.insert(
        "cancel".to_string(),
        ShortcutBinding {
            id: "cancel".to_string(),
            name: "Cancel".to_string(),
            description: "Cancels the current recording.".to_string(),
            default_binding: "escape".to_string(),
            current_binding: "escape".to_string(),
        },
    );
    bindings.insert(
        "repaste_last".to_string(),
        ShortcutBinding {
            id: "repaste_last".to_string(),
            name: "Repaste Last".to_string(),
            description: "Paste the most recent transcription or AI response again.".to_string(),
            default_binding: "ctrl+shift+z".to_string(),
            current_binding: "ctrl+shift+z".to_string(),
        },
    );
    #[cfg(target_os = "windows")]
    bindings.insert(
        "voice_command".to_string(),
        ShortcutBinding {
            id: "voice_command".to_string(),
            name: "Voice Command".to_string(),
            description: "Speak to run predefined scripts or get AI-suggested PowerShell commands."
                .to_string(),
            default_binding: "".to_string(),
            current_binding: "".to_string(),
        },
    );
    #[cfg(target_os = "windows")]
    bindings.insert(
        "spawn_button".to_string(),
        ShortcutBinding {
            id: "spawn_button".to_string(),
            name: "Spawn Voice Activation Button".to_string(),
            description: "Open a floating on-screen voice activation button window.".to_string(),
            default_binding: "".to_string(),
            current_binding: "".to_string(),
        },
    );
    // Default profile shortcut (optional - uses global settings when active)
    bindings.insert(
        "transcribe_default".to_string(),
        ShortcutBinding {
            id: "transcribe_default".to_string(),
            name: "Transcribe (Default Profile)".to_string(),
            description: "Transcribe using global language settings, regardless of active profile."
                .to_string(),
            default_binding: "".to_string(),
            current_binding: "".to_string(),
        },
    );
    // Cycle through transcription profiles
    bindings.insert(
        "cycle_profile".to_string(),
        ShortcutBinding {
            id: "cycle_profile".to_string(),
            name: "Cycle Transcription Profile".to_string(),
            description: "Switch to the next transcription profile in the rotation.".to_string(),
            default_binding: "".to_string(),
            current_binding: "".to_string(),
        },
    );

    AppSettings {
        bindings,
        push_to_talk: true,
        audio_feedback: false,
        audio_feedback_volume: default_audio_feedback_volume(),
        sound_theme: default_sound_theme(),
        start_hidden: default_start_hidden(),
        autostart_enabled: default_autostart_enabled(),
        show_tray_icon: default_show_tray_icon(),
        update_checks_enabled: default_update_checks_enabled(),
        selected_model: "".to_string(),
        transcription_provider: default_transcription_provider(),
        remote_stt: default_remote_stt_settings(),
        soniox_model: default_soniox_model(),
        soniox_timeout_seconds: default_soniox_timeout_seconds(),
        soniox_live_enabled: default_soniox_live_enabled(),
        soniox_language_hints: default_soniox_language_hints(),
        soniox_use_profile_language_hint_only: default_false(),
        soniox_language_hints_strict: default_false(),
        soniox_enable_endpoint_detection: default_true(),
        soniox_max_endpoint_delay_ms: default_soniox_max_endpoint_delay_ms(),
        soniox_enable_language_identification: default_true(),
        soniox_enable_speaker_diarization: default_true(),
        soniox_keepalive_interval_seconds: default_soniox_keepalive_interval_seconds(),
        soniox_live_finalize_timeout_ms: default_soniox_live_finalize_timeout_ms(),
        soniox_live_instant_stop: default_false(),
        soniox_realtime_fuzzy_correction_enabled: default_false(),
        soniox_realtime_keep_safety_buffer_enabled: default_false(),
        always_on_microphone: false,
        selected_microphone: None,
        clamshell_microphone: None,
        selected_output_device: None,
        translate_to_english: false,
        selected_language: "auto".to_string(),
        overlay_position: default_overlay_position(),
        debug_mode: false,
        log_level: default_log_level(),
        custom_words: Vec::new(),
        custom_words_enabled: default_custom_words_enabled(),
        custom_words_ngram_enabled: default_custom_words_ngram_enabled(),
        model_unload_timeout: ModelUnloadTimeout::Never,
        word_correction_threshold: default_word_correction_threshold(),
        history_limit: default_history_limit(),
        recording_retention_period: default_recording_retention_period(),
        paste_method: PasteMethod::default(),
        paste_delay_ms: default_paste_delay_ms(),
        convert_lf_to_crlf: true,
        clipboard_handling: ClipboardHandling::default(),
        auto_submit: default_auto_submit(),
        auto_submit_key: AutoSubmitKey::default(),
        post_process_enabled: default_post_process_enabled(),
        post_process_provider_id: default_post_process_provider_id(),
        post_process_providers: default_post_process_providers(),
        post_process_api_keys: default_post_process_api_keys(),
        post_process_models: default_post_process_models(),
        post_process_prompts: default_post_process_prompts(),
        post_process_selected_prompt_id: None,
        llm_context_prev_transcript_enabled: true,
        llm_context_prev_transcript_max_words: default_llm_context_prev_transcript_max_words(),
        llm_context_prev_transcript_expiry_seconds:
            default_llm_context_prev_transcript_expiry_seconds(),
        ai_replace_system_prompt: default_ai_replace_system_prompt(),
        ai_replace_user_prompt: default_ai_replace_user_prompt(),
        ai_replace_max_chars: default_ai_replace_max_chars(),
        ai_replace_allow_no_selection: default_ai_replace_allow_no_selection(),
        ai_replace_no_selection_system_prompt: default_ai_replace_no_selection_system_prompt(),
        ai_replace_allow_quick_tap: default_ai_replace_allow_quick_tap(),
        ai_replace_quick_tap_threshold_ms: default_ai_replace_quick_tap_threshold_ms(),
        ai_replace_quick_tap_system_prompt: default_ai_replace_quick_tap_system_prompt(),
        ai_replace_provider_id: None,
        ai_replace_api_keys: HashMap::new(),
        ai_replace_models: HashMap::new(),
        send_to_extension_with_selection_system_prompt:
            default_send_to_extension_with_selection_system_prompt(),
        send_to_extension_with_selection_user_prompt:
            default_send_to_extension_with_selection_user_prompt(),
        send_to_extension_with_selection_allow_no_voice: true,
        send_to_extension_with_selection_quick_tap_threshold_ms: default_quick_tap_threshold_ms(),
        send_to_extension_with_selection_no_voice_system_prompt: String::new(),
        send_to_extension_enabled: false,
        send_to_extension_push_to_talk: true,
        send_to_extension_with_selection_enabled: false,
        send_to_extension_with_selection_push_to_talk: true,
        ai_replace_selection_push_to_talk: true,
        mute_while_recording: false,
        append_trailing_space: false,
        filter_silence: default_filter_silence(),
        connector_port: default_connector_port(),
        connector_auto_open_enabled: default_connector_auto_open_enabled(),
        connector_auto_open_url: default_connector_auto_open_url(),
        screenshot_capture_method: default_screenshot_capture_method(),
        native_region_capture_mode: default_native_region_capture_mode(),
        screenshot_capture_command: default_screenshot_capture_command(),
        screenshot_folder: default_screenshot_folder(),
        screenshot_require_recent: true,
        screenshot_timeout_seconds: default_screenshot_timeout_seconds(),
        screenshot_include_subfolders: true,
        screenshot_allow_no_voice: true,
        screenshot_quick_tap_threshold_ms: default_quick_tap_threshold_ms(),
        screenshot_no_voice_default_prompt: default_screenshot_no_voice_default_prompt(),
        send_screenshot_to_extension_enabled: false,
        send_screenshot_to_extension_push_to_talk: true,
        app_language: default_app_language(),
        connector_password: default_connector_password(),
        connector_password_user_set: false,
        connector_pending_password: None,
        transcription_prompts: HashMap::new(),
        transcription_profiles: Vec::new(),
        active_profile_id: default_active_profile_id(),
        profile_switch_overlay_enabled: true,
        // Voice Command Center
        voice_command_enabled: false,
        voice_command_push_to_talk: true,
        voice_commands: Vec::new(),
        voice_command_default_threshold: default_voice_command_threshold(),
        voice_command_llm_fallback: true,
        voice_command_system_prompt: default_voice_command_system_prompt(),
        voice_command_defaults: VoiceCommandDefaults::default(),
        voice_command_template: String::new(), // Deprecated, kept for migration
        voice_command_keep_window_open: false, // Deprecated, kept for migration
        voice_command_auto_run: false,
        voice_command_auto_run_seconds: default_voice_command_auto_run_seconds(),
        // Extended Thinking / Reasoning
        post_process_reasoning_enabled: false,
        post_process_reasoning_budget: default_reasoning_budget(),
        ai_replace_reasoning_enabled: false,
        ai_replace_reasoning_budget: default_reasoning_budget(),
        // Voice Command LLM Settings
        voice_command_provider_id: None,
        voice_command_api_keys: HashMap::new(),
        voice_command_models: HashMap::new(),
        voice_command_reasoning_enabled: false,
        voice_command_reasoning_budget: default_reasoning_budget(),
        // Voice Command Fuzzy Matching
        voice_command_use_levenshtein: true,
        voice_command_levenshtein_threshold: default_voice_command_levenshtein_threshold(),
        voice_command_use_phonetic: true,
        voice_command_phonetic_boost: default_voice_command_phonetic_boost(),
        voice_command_word_similarity_threshold: default_voice_command_word_similarity_threshold(),
        // Beta Feature Flags
        beta_voice_commands_enabled: false,
        voice_button_show_aot_toggle: false,
        voice_button_single_click_close: false,
        // Text Replacement
        text_replacements_enabled: false,
        text_replacements: Vec::new(),
        text_replacements_before_llm: false,
        text_replacement_decapitalize_after_edit_key_enabled: false,
        text_replacement_decapitalize_after_edit_key:
            default_text_replacement_decapitalize_after_edit_key(),
        text_replacement_decapitalize_timeout_ms:
            default_text_replacement_decapitalize_timeout_ms(),
        text_replacement_decapitalize_standard_post_recording_monitor_ms:
            default_text_replacement_decapitalize_standard_post_recording_monitor_ms(),
        // Audio Processing
        filler_word_filter_enabled: false,
        zero_width_filter_enabled: true,
        trim_transcription_output_enabled: true,
        vad_threshold: default_vad_threshold(),
        // Shortcut Engine (Windows only)
        shortcut_engine: ShortcutEngine::default(),
        // UI State
        sidebar_pinned: false,
        sidebar_width: default_sidebar_width(),
    }
}

impl AppSettings {
    pub fn active_post_process_provider(&self) -> Option<&PostProcessProvider> {
        self.post_process_providers
            .iter()
            .find(|provider| provider.id == self.post_process_provider_id)
    }

    /// Get the active LLM provider for Voice Commands.
    /// If voice_command_provider_id is set, uses that; otherwise falls back to post-processing provider.
    pub fn active_voice_command_provider(&self) -> Option<&PostProcessProvider> {
        if let Some(ref provider_id) = self.voice_command_provider_id {
            self.post_process_providers
                .iter()
                .find(|provider| &provider.id == provider_id)
        } else {
            // Fallback to post-processing provider for backwards compatibility
            self.active_post_process_provider()
        }
    }

    /// Get a transcription profile by its ID.
    pub fn transcription_profile(&self, profile_id: &str) -> Option<&TranscriptionProfile> {
        self.transcription_profiles
            .iter()
            .find(|p| p.id == profile_id)
    }

    /// Get a transcription profile by its binding ID (e.g., "transcribe_profile_abc123").
    /// Returns None if binding_id doesn't match the expected pattern.
    pub fn transcription_profile_by_binding(
        &self,
        binding_id: &str,
    ) -> Option<&TranscriptionProfile> {
        if let Some(profile_id) = binding_id.strip_prefix("transcribe_") {
            self.transcription_profile(profile_id)
        } else {
            None
        }
    }

    pub fn post_process_provider(&self, provider_id: &str) -> Option<&PostProcessProvider> {
        self.post_process_providers
            .iter()
            .find(|provider| provider.id == provider_id)
    }

    pub fn post_process_provider_mut(
        &mut self,
        provider_id: &str,
    ) -> Option<&mut PostProcessProvider> {
        self.post_process_providers
            .iter_mut()
            .find(|provider| provider.id == provider_id)
    }

    /// Get the active AI Replace LLM provider.
    /// Falls back to post-processing provider if none is set.
    pub fn active_ai_replace_provider(&self) -> Option<&PostProcessProvider> {
        if let Some(ref provider_id) = self.ai_replace_provider_id {
            self.post_process_providers
                .iter()
                .find(|p| &p.id == provider_id)
        } else {
            self.active_post_process_provider()
        }
    }

    /// Get AI Replace API key for a provider.
    /// Uses post-processing API key only when AI Replace provider is set to
    /// "same as post-processing" (ai_replace_provider_id = None).
    pub fn ai_replace_api_key(&self, provider_id: &str) -> String {
        // On Windows, use secure key storage
        #[cfg(target_os = "windows")]
        {
            // If AI Replace is configured to use the same provider as post-processing,
            // use the post-processing API key (ignore any AI Replace overrides).
            if self.ai_replace_provider_id.as_deref() != Some(provider_id) {
                return crate::secure_keys::get_post_process_api_key(provider_id);
            }

            // When AI Replace has its own provider selected, use only AI Replace key.
            return crate::secure_keys::get_ai_replace_api_key(provider_id);
        }

        // On non-Windows, use JSON settings
        #[cfg(not(target_os = "windows"))]
        {
            if self.ai_replace_provider_id.as_deref() != Some(provider_id) {
                return self
                    .post_process_api_keys
                    .get(provider_id)
                    .cloned()
                    .unwrap_or_default();
            }

            self.ai_replace_api_keys
                .get(provider_id)
                .cloned()
                .unwrap_or_default()
        }
    }

    /// Get AI Replace model for a provider.
    /// Falls back to post-processing model if not set.
    pub fn ai_replace_model(&self, provider_id: &str) -> String {
        // If AI Replace is configured to use the same provider as post-processing,
        // use the post-processing model (ignore any AI Replace overrides).
        if self.ai_replace_provider_id.as_deref() != Some(provider_id) {
            return self
                .post_process_models
                .get(provider_id)
                .cloned()
                .unwrap_or_default();
        }

        self.ai_replace_models
            .get(provider_id)
            .filter(|m| !m.is_empty())
            .cloned()
            .unwrap_or_else(|| {
                self.post_process_models
                    .get(provider_id)
                    .cloned()
                    .unwrap_or_default()
            })
    }

    /// Get the fully resolved LLM configuration for a specific feature.
    /// This is the primary entry point for getting LLM settings with proper fallback chains.
    /// On Windows, API keys are fetched from secure storage.
    pub fn llm_config_for(&self, feature: LlmFeature) -> Option<LlmConfig> {
        match feature {
            LlmFeature::PostProcessing => {
                let provider = self.active_post_process_provider()?;

                // On Windows, use secure key storage
                #[cfg(target_os = "windows")]
                let api_key = crate::secure_keys::get_post_process_api_key(&provider.id);

                // On non-Windows, use JSON settings
                #[cfg(not(target_os = "windows"))]
                let api_key = self
                    .post_process_api_keys
                    .get(&provider.id)
                    .cloned()
                    .unwrap_or_default();

                let model = self
                    .post_process_models
                    .get(&provider.id)
                    .cloned()
                    .unwrap_or_default();

                Some(LlmConfig {
                    provider_id: provider.id.clone(),
                    api_key,
                    model,
                    base_url: provider.base_url.clone(),
                })
            }
            LlmFeature::AiReplace => {
                let provider = self.active_ai_replace_provider()?;
                let api_key = self.ai_replace_api_key(&provider.id);
                let model = self.ai_replace_model(&provider.id);

                Some(LlmConfig {
                    provider_id: provider.id.clone(),
                    api_key,
                    model,
                    base_url: provider.base_url.clone(),
                })
            }
            LlmFeature::VoiceCommand => {
                let provider = self.active_voice_command_provider()?;

                // Use post-processing key only when voice command provider is set to
                // "same as post-processing" (voice_command_provider_id = None).
                #[cfg(target_os = "windows")]
                let api_key = if self.voice_command_provider_id.as_deref()
                    != Some(provider.id.as_str())
                {
                    crate::secure_keys::get_post_process_api_key(&provider.id)
                } else {
                    crate::secure_keys::get_voice_command_api_key(&provider.id).unwrap_or_default()
                };

                #[cfg(not(target_os = "windows"))]
                let api_key =
                    if self.voice_command_provider_id.as_deref() != Some(provider.id.as_str()) {
                        self.post_process_api_keys
                            .get(&provider.id)
                            .cloned()
                            .unwrap_or_default()
                    } else {
                        self.voice_command_api_keys
                            .get(&provider.id)
                            .cloned()
                            .unwrap_or_default()
                    };

                // Use voice command model with fallback to post-processing model
                let model = self
                    .voice_command_models
                    .get(&provider.id)
                    .cloned()
                    .filter(|m| !m.is_empty())
                    .or_else(|| self.post_process_models.get(&provider.id).cloned())
                    .unwrap_or_default();

                Some(LlmConfig {
                    provider_id: provider.id.clone(),
                    api_key,
                    model,
                    base_url: provider.base_url.clone(),
                })
            }
        }
    }
}

pub fn load_or_create_app_settings(app: &AppHandle) -> AppSettings {
    // Initialize store
    let store = app
        .store(SETTINGS_STORE_PATH)
        .expect("Failed to initialize store");

    let mut settings = if let Some(settings_value) = store.get("settings") {
        // Parse the entire settings object
        match serde_json::from_value::<AppSettings>(settings_value) {
            Ok(mut settings) => {
                debug!("Found existing settings: {:?}", settings);
                let default_settings = get_default_settings();
                let mut updated = false;

                // Merge default bindings into existing settings
                for (key, value) in default_settings.bindings {
                    if !settings.bindings.contains_key(&key) {
                        debug!("Adding missing binding: {}", key);
                        settings.bindings.insert(key, value);
                        updated = true;
                    }
                }

                // Migrate API keys from JSON to secure storage (Windows only)
                #[cfg(target_os = "windows")]
                {
                    let (migrated, migrated_pp, migrated_ai) =
                        crate::secure_keys::migrate_keys_from_settings(
                            &settings.post_process_api_keys,
                            &settings.ai_replace_api_keys,
                        );

                    if migrated {
                        debug!(
                            "Migrated API keys to secure storage. Post-process: {:?}, AI Replace: {:?}",
                            migrated_pp, migrated_ai
                        );

                        // Clear migrated keys from JSON settings
                        for provider_id in migrated_pp {
                            settings
                                .post_process_api_keys
                                .insert(provider_id, String::new());
                        }
                        for provider_id in migrated_ai {
                            settings
                                .ai_replace_api_keys
                                .insert(provider_id, String::new());
                        }
                        updated = true;
                    }
                }

                // Migrate old voice_command_keep_window_open to voice_command_defaults.silent
                // voice_command_keep_window_open: true → silent: false
                // voice_command_keep_window_open: false → silent: true (default)
                if settings.voice_command_keep_window_open {
                    debug!(
                        "Migrating voice_command_keep_window_open to voice_command_defaults.silent"
                    );
                    settings.voice_command_defaults.silent = false;
                    settings.voice_command_keep_window_open = false;
                    updated = true;
                }

                if updated {
                    debug!("Settings updated with new bindings");
                    store.set("settings", serde_json::to_value(&settings).unwrap());
                }

                settings
            }
            Err(e) => {
                warn!("Failed to parse settings: {}", e);
                // Fall back to default settings if parsing fails
                let default_settings = get_default_settings();
                store.set("settings", serde_json::to_value(&default_settings).unwrap());
                default_settings
            }
        }
    } else {
        let default_settings = get_default_settings();
        store.set("settings", serde_json::to_value(&default_settings).unwrap());
        default_settings
    };

    if ensure_post_process_defaults(&mut settings) {
        store.set("settings", serde_json::to_value(&settings).unwrap());
    }

    // Normalize active_profile_id: if it points to a non-existent profile, reset to "default"
    if settings.active_profile_id != "default"
        && !settings
            .transcription_profiles
            .iter()
            .any(|p| p.id == settings.active_profile_id)
    {
        warn!(
            "Active profile '{}' not found, resetting to default",
            settings.active_profile_id
        );
        settings.active_profile_id = "default".to_string();
        store.set("settings", serde_json::to_value(&settings).unwrap());
    }

    settings
}

pub fn get_settings(app: &AppHandle) -> AppSettings {
    let store = app
        .store(SETTINGS_STORE_PATH)
        .expect("Failed to initialize store");

    let mut settings = if let Some(settings_value) = store.get("settings") {
        serde_json::from_value::<AppSettings>(settings_value).unwrap_or_else(|_| {
            let default_settings = get_default_settings();
            store.set("settings", serde_json::to_value(&default_settings).unwrap());
            default_settings
        })
    } else {
        let default_settings = get_default_settings();
        store.set("settings", serde_json::to_value(&default_settings).unwrap());
        default_settings
    };

    if ensure_post_process_defaults(&mut settings) {
        store.set("settings", serde_json::to_value(&settings).unwrap());
    }

    settings
}

pub fn write_settings(app: &AppHandle, settings: AppSettings) {
    let store = app
        .store(SETTINGS_STORE_PATH)
        .expect("Failed to initialize store");

    store.set("settings", serde_json::to_value(&settings).unwrap());

    // Explicitly flush to disk to prevent data loss on app restart
    if let Err(e) = store.save() {
        warn!("Failed to flush settings to disk: {}", e);
    }
}

pub fn get_bindings(app: &AppHandle) -> HashMap<String, ShortcutBinding> {
    let settings = get_settings(app);

    settings.bindings
}

pub fn get_stored_binding(app: &AppHandle, id: &str) -> ShortcutBinding {
    let bindings = get_bindings(app);

    let binding = bindings.get(id).unwrap().clone();

    binding
}

pub fn get_history_limit(app: &AppHandle) -> usize {
    let settings = get_settings(app);
    settings.history_limit
}

pub fn get_recording_retention_period(app: &AppHandle) -> RecordingRetentionPeriod {
    let settings = get_settings(app);
    settings.recording_retention_period
}
