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

#[cfg(test)]
mod tests {
    use super::*;

    fn remote_settings(
        provider_preset: &str,
        base_url: &str,
        allow_insecure_http: bool,
    ) -> RemoteSttSettings {
        RemoteSttSettings {
            base_url: base_url.to_string(),
            provider_preset: provider_preset.to_string(),
            allow_insecure_http,
            model_id: "test-model".to_string(),
            debug_capture: false,
            debug_mode: crate::settings::RemoteSttDebugMode::Normal,
        }
    }

    fn provider(id: &str, base_url: &str, allow_insecure_http: bool) -> PostProcessProvider {
        PostProcessProvider {
            id: id.to_string(),
            label: id.to_string(),
            base_url: base_url.to_string(),
            allow_base_url_edit: true,
            allow_insecure_http,
            models_endpoint: None,
        }
    }

    #[test]
    fn remote_stt_base_url_for_preset_returns_expected_urls() {
        assert_eq!(
            remote_stt_base_url_for_preset(REMOTE_STT_PRESET_GROQ),
            Some(REMOTE_STT_GROQ_BASE_URL)
        );
        assert_eq!(
            remote_stt_base_url_for_preset(REMOTE_STT_PRESET_OPENAI),
            Some(REMOTE_STT_OPENAI_BASE_URL)
        );
        assert_eq!(
            remote_stt_base_url_for_preset(REMOTE_STT_PRESET_CUSTOM),
            None
        );
        assert_eq!(remote_stt_base_url_for_preset("unknown"), None);
    }

    #[test]
    fn remote_stt_default_model_for_preset_returns_expected_models() {
        assert_eq!(
            remote_stt_default_model_for_preset(REMOTE_STT_PRESET_GROQ),
            Some(REMOTE_STT_GROQ_DEFAULT_MODEL)
        );
        assert_eq!(
            remote_stt_default_model_for_preset(REMOTE_STT_PRESET_OPENAI),
            Some(REMOTE_STT_OPENAI_DEFAULT_MODEL)
        );
        assert_eq!(remote_stt_default_model_for_preset("custom"), None);
    }

    #[test]
    fn infer_remote_stt_preset_trims_whitespace_and_trailing_slash() {
        assert_eq!(
            infer_remote_stt_preset("  https://api.groq.com/openai/v1/  "),
            REMOTE_STT_PRESET_GROQ
        );
        assert_eq!(
            infer_remote_stt_preset("https://api.openai.com/v1/"),
            REMOTE_STT_PRESET_OPENAI
        );
    }

    #[test]
    fn infer_remote_stt_preset_returns_custom_for_unknown_urls() {
        assert_eq!(
            infer_remote_stt_preset("https://transcribe.example.com/v1"),
            REMOTE_STT_PRESET_CUSTOM
        );
    }

    #[test]
    fn is_plain_http_url_detects_plain_http_only() {
        assert!(is_plain_http_url(" http://localhost:8080/v1 "));
        assert!(!is_plain_http_url("https://localhost:8080/v1"));
        assert!(!is_plain_http_url("not-a-url"));
    }

    #[test]
    fn validate_remote_stt_base_url_returns_canonical_url_for_known_preset() {
        let settings =
            remote_settings(REMOTE_STT_PRESET_GROQ, "https://ignored.example.com", false);

        assert_eq!(
            validate_remote_stt_base_url(&settings, Some("https://override.example.com")).unwrap(),
            REMOTE_STT_GROQ_BASE_URL
        );
    }

    #[test]
    fn validate_remote_stt_base_url_normalizes_custom_https_urls() {
        let settings = remote_settings(
            REMOTE_STT_PRESET_CUSTOM,
            "  https://custom.example.com/v1/ ",
            false,
        );

        assert_eq!(
            validate_remote_stt_base_url(&settings, None).unwrap(),
            "https://custom.example.com/v1"
        );
    }

    #[test]
    fn validate_remote_stt_base_url_rejects_plain_http_without_override_flag() {
        let settings = remote_settings(REMOTE_STT_PRESET_CUSTOM, "http://localhost:8000/v1", false);

        let error = validate_remote_stt_base_url(&settings, None).unwrap_err();
        assert!(error.contains("must use HTTPS"));
        assert!(error.contains("advanced insecure HTTP override"));
    }

    #[test]
    fn validate_remote_stt_base_url_accepts_plain_http_when_override_flag_enabled() {
        let settings =
            remote_settings(REMOTE_STT_PRESET_CUSTOM, "https://unused.example.com", true);

        assert_eq!(
            validate_remote_stt_base_url(&settings, Some("http://localhost:8000/v1/")).unwrap(),
            "http://localhost:8000/v1"
        );
    }

    #[test]
    fn validate_remote_stt_base_url_infers_known_base_url_for_unknown_preset() {
        let settings = remote_settings("legacy", "https://api.openai.com/v1/", false);

        assert_eq!(
            validate_remote_stt_base_url(&settings, None).unwrap(),
            REMOTE_STT_OPENAI_BASE_URL
        );
    }

    #[test]
    fn validate_remote_stt_base_url_rejects_non_http_schemes() {
        let settings = remote_settings(
            REMOTE_STT_PRESET_CUSTOM,
            "ftp://files.example.com/api",
            false,
        );

        let error = validate_remote_stt_base_url(&settings, None).unwrap_err();
        assert!(error.contains("http:// or https://"));
        assert!(error.contains("ftp://"));
    }

    #[test]
    fn canonical_llm_provider_base_url_returns_known_provider_defaults() {
        assert_eq!(
            canonical_llm_provider_base_url(&provider(
                "openai",
                "https://ignored.example.com",
                false
            ))
            .unwrap(),
            LLM_OPENAI_BASE_URL
        );
        assert_eq!(
            canonical_llm_provider_base_url(&provider(
                "groq",
                "https://ignored.example.com",
                false
            ))
            .unwrap(),
            LLM_GROQ_BASE_URL
        );
        assert_eq!(
            canonical_llm_provider_base_url(&provider(
                "cerebras",
                "https://ignored.example.com",
                false
            ))
            .unwrap(),
            LLM_CEREBRAS_BASE_URL
        );
    }

    #[test]
    fn canonical_llm_provider_base_url_normalizes_custom_provider_urls() {
        let actual = canonical_llm_provider_base_url(&provider(
            "custom",
            " https://llm.example.com/v1/ ",
            false,
        ))
        .unwrap();

        assert_eq!(actual, "https://llm.example.com/v1");
    }

    #[test]
    fn canonical_llm_provider_base_url_rejects_custom_http_without_opt_in() {
        let error =
            canonical_llm_provider_base_url(&provider("custom", "http://llm.local/v1", false))
                .unwrap_err();

        assert!(error.contains("Custom LLM base URL"));
        assert!(error.contains("must use HTTPS"));
    }

    #[test]
    fn canonical_llm_provider_base_url_preserves_apple_intelligence_base_url() {
        let actual = canonical_llm_provider_base_url(&provider(
            APPLE_INTELLIGENCE_PROVIDER_ID,
            "apple-intelligence://on-device",
            false,
        ))
        .unwrap();

        assert_eq!(actual, "apple-intelligence://on-device");
    }

    #[test]
    fn canonical_llm_provider_base_url_validates_unknown_provider_as_https() {
        let error = canonical_llm_provider_base_url(&provider(
            "custom-like",
            "http://unsafe.example.com/v1",
            true,
        ))
        .unwrap_err();

        assert!(error.contains("LLM provider base URL"));
        assert!(error.contains("must use HTTPS"));
    }
}
