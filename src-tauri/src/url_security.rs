use reqwest::Url;

use crate::settings::{PostProcessProvider, RemoteSttSettings, APPLE_INTELLIGENCE_PROVIDER_ID};

pub const REMOTE_STT_PRESET_GROQ: &str = "groq";
pub const REMOTE_STT_PRESET_OPENAI: &str = "openai";
pub const REMOTE_STT_PRESET_CUSTOM: &str = "custom";

pub const REMOTE_STT_GROQ_BASE_URL: &str = "https://api.groq.com/openai/v1";
pub const REMOTE_STT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

pub const REMOTE_STT_GROQ_DEFAULT_MODEL: &str = "whisper-large-v3-turbo";
pub const REMOTE_STT_OPENAI_DEFAULT_MODEL: &str = "whisper-1";

pub const LLM_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
pub const LLM_ZAI_BASE_URL: &str = "https://api.z.ai/api/paas/v4";
pub const LLM_OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";
pub const LLM_ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com/v1";
pub const LLM_GROQ_BASE_URL: &str = "https://api.groq.com/openai/v1";
pub const LLM_CEREBRAS_BASE_URL: &str = "https://api.cerebras.ai/v1";

fn parse_network_url(input: &str, context: &str) -> Result<Url, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(format!("{} is empty.", context));
    }

    Url::parse(trimmed).map_err(|err| format!("{} is invalid: {}", context, err))
}

fn normalize_url(url: &Url) -> String {
    url.as_str().trim_end_matches('/').to_string()
}

fn validate_network_base_url(
    input: &str,
    allow_insecure_http: bool,
    context: &str,
) -> Result<String, String> {
    let url = parse_network_url(input, context)?;

    match url.scheme() {
        "https" => Ok(normalize_url(&url)),
        "http" if allow_insecure_http => Ok(normalize_url(&url)),
        "http" => Err(format!(
            "{} must use HTTPS. Plain HTTP is allowed only for a Custom endpoint after enabling the advanced insecure HTTP override.",
            context
        )),
        scheme => Err(format!(
            "{} must use http:// or https://, but got '{}://'.",
            context, scheme
        )),
    }
}

pub fn remote_stt_base_url_for_preset(preset: &str) -> Option<&'static str> {
    match preset {
        REMOTE_STT_PRESET_GROQ => Some(REMOTE_STT_GROQ_BASE_URL),
        REMOTE_STT_PRESET_OPENAI => Some(REMOTE_STT_OPENAI_BASE_URL),
        REMOTE_STT_PRESET_CUSTOM => None,
        _ => None,
    }
}

pub fn remote_stt_default_model_for_preset(preset: &str) -> Option<&'static str> {
    match preset {
        REMOTE_STT_PRESET_GROQ => Some(REMOTE_STT_GROQ_DEFAULT_MODEL),
        REMOTE_STT_PRESET_OPENAI => Some(REMOTE_STT_OPENAI_DEFAULT_MODEL),
        _ => None,
    }
}

pub fn infer_remote_stt_preset(base_url: &str) -> &'static str {
    let trimmed = base_url.trim().trim_end_matches('/');
    match trimmed {
        REMOTE_STT_GROQ_BASE_URL => REMOTE_STT_PRESET_GROQ,
        REMOTE_STT_OPENAI_BASE_URL => REMOTE_STT_PRESET_OPENAI,
        _ => REMOTE_STT_PRESET_CUSTOM,
    }
}

pub fn is_plain_http_url(input: &str) -> bool {
    parse_network_url(input, "URL")
        .map(|url| url.scheme() == "http")
        .unwrap_or(false)
}

pub fn validate_remote_stt_base_url(
    settings: &RemoteSttSettings,
    override_base_url: Option<&str>,
) -> Result<String, String> {
    match settings.provider_preset.as_str() {
        REMOTE_STT_PRESET_GROQ => Ok(REMOTE_STT_GROQ_BASE_URL.to_string()),
        REMOTE_STT_PRESET_OPENAI => Ok(REMOTE_STT_OPENAI_BASE_URL.to_string()),
        REMOTE_STT_PRESET_CUSTOM => validate_network_base_url(
            override_base_url.unwrap_or(&settings.base_url),
            settings.allow_insecure_http,
            "Remote STT base URL",
        ),
        _ => {
            let inferred = infer_remote_stt_preset(&settings.base_url);
            if let Some(base_url) = remote_stt_base_url_for_preset(inferred) {
                return Ok(base_url.to_string());
            }
            validate_network_base_url(
                override_base_url.unwrap_or(&settings.base_url),
                settings.allow_insecure_http,
                "Remote STT base URL",
            )
        }
    }
}

pub fn canonical_llm_provider_base_url(provider: &PostProcessProvider) -> Result<String, String> {
    match provider.id.as_str() {
        "openai" => Ok(LLM_OPENAI_BASE_URL.to_string()),
        "zai" => Ok(LLM_ZAI_BASE_URL.to_string()),
        "openrouter" => Ok(LLM_OPENROUTER_BASE_URL.to_string()),
        "anthropic" => Ok(LLM_ANTHROPIC_BASE_URL.to_string()),
        "groq" => Ok(LLM_GROQ_BASE_URL.to_string()),
        "cerebras" => Ok(LLM_CEREBRAS_BASE_URL.to_string()),
        "custom" => validate_network_base_url(
            &provider.base_url,
            provider.allow_insecure_http,
            "Custom LLM base URL",
        ),
        APPLE_INTELLIGENCE_PROVIDER_ID => Ok(provider.base_url.clone()),
        _ => validate_network_base_url(&provider.base_url, false, "LLM provider base URL"),
    }
}
