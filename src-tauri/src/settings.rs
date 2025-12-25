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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionProvider {
    Local,
    #[serde(rename = "remote_openai_compatible")]
    RemoteOpenAiCompatible,
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
    #[serde(default = "default_update_checks_enabled")]
    pub update_checks_enabled: bool,
    #[serde(default = "default_model")]
    pub selected_model: String,
    #[serde(default = "default_transcription_provider")]
    pub transcription_provider: TranscriptionProvider,
    #[serde(default = "default_remote_stt_settings")]
    pub remote_stt: RemoteSttSettings,
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
    #[serde(default)]
    pub clipboard_handling: ClipboardHandling,
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
    #[serde(default = "default_true")]
    pub send_to_extension_push_to_talk: bool,
    #[serde(default = "default_true")]
    pub send_to_extension_with_selection_push_to_talk: bool,
    #[serde(default = "default_true")]
    pub ai_replace_selection_push_to_talk: bool,
    #[serde(default)]
    pub mute_while_recording: bool,
    #[serde(default)]
    pub append_trailing_space: bool,
    #[serde(default = "default_connector_host")]
    pub connector_host: String,
    #[serde(default = "default_connector_port")]
    pub connector_port: u16,
    #[serde(default = "default_connector_path")]
    pub connector_path: String,
    #[serde(default = "default_connector_auto_open_enabled")]
    pub connector_auto_open_enabled: bool,
    #[serde(default = "default_connector_auto_open_url")]
    pub connector_auto_open_url: String,
    #[serde(default = "default_connector_send_system_prompt")]
    pub connector_send_system_prompt: String,
    #[serde(default = "default_connector_send_selection_system_prompt")]
    pub connector_send_selection_system_prompt: String,
    #[serde(default = "default_connector_send_selection_user_prompt")]
    pub connector_send_selection_user_prompt: String,
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
    #[serde(default = "default_screenshot_no_voice_default_prompt")]
    pub screenshot_no_voice_default_prompt: String,
    #[serde(default = "default_true")]
    pub send_screenshot_to_extension_push_to_talk: bool,
    #[serde(default = "default_app_language")]
    pub app_language: String,
    #[serde(default = "default_connector_password")]
    pub connector_password: String,
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
    LogLevel::Debug
}

fn default_word_correction_threshold() -> f64 {
    0.18
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

fn default_app_language() -> String {
    tauri_plugin_os::locale()
        .and_then(|l| l.split(['-', '_']).next().map(String::from))
        .unwrap_or_else(|| "en".to_string())
}

fn default_connector_host() -> String {
    "127.0.0.1".to_string()
}

fn default_connector_port() -> u16 {
    63155
}

fn default_connector_path() -> String {
    "/messages".to_string()
}

fn default_connector_auto_open_enabled() -> bool {
    false
}

fn default_connector_auto_open_url() -> String {
    "".to_string()
}

fn default_connector_send_system_prompt() -> String {
    "".to_string()
}

fn default_connector_send_selection_system_prompt() -> String {
    "You are a text transformation engine.\nReturn ONLY the final transformed text that is ready to be sent to an AI chat.\nDo not include explanations, commentary, labels, headings, lists, markdown, code fences, or any surrounding quotes.\nPreserve the original language and keep the original formatting (line breaks, punctuation, and spacing) unless the instruction explicitly asks to change it.\nMake the smallest change that satisfies the instruction.\nIf the instruction conflicts with the text or is unclear, prefer minimal edits and do not invent new facts.".to_string()
}

fn default_connector_send_selection_user_prompt() -> String {
    "INSTRUCTION:\n${instruction}\n\nTEXT:\n${output}".to_string()
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

fn default_connector_password() -> String {
    "fklejqwhfiu342lhk3".to_string()
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

fn default_post_process_providers() -> Vec<PostProcessProvider> {
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
            id: "custom".to_string(),
            label: "Custom".to_string(),
            base_url: "http://localhost:11434/v1".to_string(),
            allow_base_url_edit: true,
            models_endpoint: Some("/models".to_string()),
        },
    ];

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        if crate::apple_intelligence::check_apple_intelligence_availability() {
            providers.push(PostProcessProvider {
                id: APPLE_INTELLIGENCE_PROVIDER_ID.to_string(),
                label: "Apple Intelligence".to_string(),
                base_url: "apple-intelligence://local".to_string(),
                allow_base_url_edit: false,
                models_endpoint: None,
            });
        }
    }

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
            description: "Send transcription to Handy Connector.".to_string(),
            default_binding: default_send_shortcut.to_string(),
            current_binding: default_send_shortcut.to_string(),
        },
    );
    bindings.insert(
        "send_to_extension_with_selection".to_string(),
        ShortcutBinding {
            id: "send_to_extension_with_selection".to_string(),
            name: "Send + Selection to Extension".to_string(),
            description:
                "Send transcription plus copied selection to Handy Connector.".to_string(),
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
                "Capture screenshot with voice instruction and send to Handy Connector."
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

    AppSettings {
        bindings,
        push_to_talk: true,
        audio_feedback: false,
        audio_feedback_volume: default_audio_feedback_volume(),
        sound_theme: default_sound_theme(),
        start_hidden: default_start_hidden(),
        autostart_enabled: default_autostart_enabled(),
        update_checks_enabled: default_update_checks_enabled(),
        selected_model: "".to_string(),
        transcription_provider: default_transcription_provider(),
        remote_stt: default_remote_stt_settings(),
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
        model_unload_timeout: ModelUnloadTimeout::Never,
        word_correction_threshold: default_word_correction_threshold(),
        history_limit: default_history_limit(),
        recording_retention_period: default_recording_retention_period(),
        paste_method: PasteMethod::default(),
        clipboard_handling: ClipboardHandling::default(),
        post_process_enabled: default_post_process_enabled(),
        post_process_provider_id: default_post_process_provider_id(),
        post_process_providers: default_post_process_providers(),
        post_process_api_keys: default_post_process_api_keys(),
        post_process_models: default_post_process_models(),
        post_process_prompts: default_post_process_prompts(),
        post_process_selected_prompt_id: None,
        ai_replace_system_prompt: default_ai_replace_system_prompt(),
        ai_replace_user_prompt: default_ai_replace_user_prompt(),
        ai_replace_max_chars: default_ai_replace_max_chars(),
        ai_replace_allow_no_selection: default_ai_replace_allow_no_selection(),
        ai_replace_no_selection_system_prompt: default_ai_replace_no_selection_system_prompt(),
        ai_replace_allow_quick_tap: default_ai_replace_allow_quick_tap(),
        ai_replace_quick_tap_threshold_ms: default_ai_replace_quick_tap_threshold_ms(),
        ai_replace_quick_tap_system_prompt: default_ai_replace_quick_tap_system_prompt(),
        send_to_extension_push_to_talk: true,
        send_to_extension_with_selection_push_to_talk: true,
        ai_replace_selection_push_to_talk: true,
        mute_while_recording: false,
        append_trailing_space: false,
        connector_host: default_connector_host(),
        connector_port: default_connector_port(),
        connector_path: default_connector_path(),
        connector_auto_open_enabled: default_connector_auto_open_enabled(),
        connector_auto_open_url: default_connector_auto_open_url(),
        connector_send_system_prompt: default_connector_send_system_prompt(),
        connector_send_selection_system_prompt: default_connector_send_selection_system_prompt(),
        connector_send_selection_user_prompt: default_connector_send_selection_user_prompt(),
        screenshot_capture_command: default_screenshot_capture_command(),
        screenshot_folder: default_screenshot_folder(),
        screenshot_require_recent: true,
        screenshot_timeout_seconds: default_screenshot_timeout_seconds(),
        screenshot_include_subfolders: false,
        screenshot_allow_no_voice: true,
        screenshot_no_voice_default_prompt: default_screenshot_no_voice_default_prompt(),
        send_screenshot_to_extension_push_to_talk: true,
        app_language: default_app_language(),
        connector_password: default_connector_password(),
    }
}

impl AppSettings {
    pub fn active_post_process_provider(&self) -> Option<&PostProcessProvider> {
        self.post_process_providers
            .iter()
            .find(|provider| provider.id == self.post_process_provider_id)
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
