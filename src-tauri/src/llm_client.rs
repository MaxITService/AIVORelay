use crate::settings::PostProcessProvider;
use crate::url_security::canonical_llm_provider_base_url;
use log::{debug, error, info, warn};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, REFERER, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::error::Error as StdError;

/// Configuration for Extended Thinking / Reasoning (OpenRouter)
#[derive(Debug, Clone, Default)]
pub struct ReasoningConfig {
    pub enabled: bool,
    pub budget: u32, // min 1024 for OpenRouter/Anthropic
    pub disable_by_default_on_compatible_providers: bool,
}

impl ReasoningConfig {
    pub fn new(enabled: bool, budget: u32) -> Self {
        Self {
            enabled,
            budget: if enabled { budget.max(1024) } else { budget },
            disable_by_default_on_compatible_providers: false,
        }
    }

    pub fn with_disable_by_default_on_compatible_providers(mut self, disable: bool) -> Self {
        self.disable_by_default_on_compatible_providers = disable;
        self
    }
}

#[derive(Debug, Clone, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// Reasoning object for OpenRouter API
#[derive(Debug, Serialize, Clone, Default)]
struct ReasoningParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exclude: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<ReasoningParams>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Debug, Deserialize)]
struct ChatMessageResponse {
    content: Option<String>,
    /// Reasoning/thinking tokens returned by OpenRouter (logged but not included in response)
    #[serde(default)]
    reasoning: Option<String>,
}

/// Build headers for API requests based on provider type
fn build_headers(provider: &PostProcessProvider, api_key: &str) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();

    // Common headers
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        REFERER,
        HeaderValue::from_static("https://github.com/MaxITService/AIVORelay"),
    );
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("AivoRelay/1.0 (+https://github.com/MaxITService/AIVORelay)"),
    );
    headers.insert("X-Title", HeaderValue::from_static("AivoRelay"));

    // Provider-specific auth headers
    if !api_key.is_empty() {
        if provider.id == "anthropic" {
            headers.insert(
                "x-api-key",
                HeaderValue::from_str(api_key)
                    .map_err(|e| format!("Invalid API key header value: {}", e))?,
            );
            headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        } else {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", api_key))
                    .map_err(|e| format!("Invalid authorization header value: {}", e))?,
            );
        }
    }

    Ok(headers)
}

/// Create an HTTP client with provider-specific headers
fn create_client(provider: &PostProcessProvider, api_key: &str) -> Result<reqwest::Client, String> {
    let headers = build_headers(provider, api_key)?;
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|e| report_reqwest_error("Failed to build HTTP client", &e))
}

/// Format a bounded error source chain.
///
/// `reqwest::Error`'s Display implementation intentionally gives only a short
/// summary. Nested causes contain the useful transport details, such as a
/// certificate validation failure, an HTTP/2 error, or a connection reset.
/// Callers must skip source types whose Display text can quote payload data.
fn error_source_chain(error: &(dyn StdError + 'static)) -> Vec<String> {
    let mut causes = Vec::new();
    let mut source = error.source();

    // Defensive cap in case a third-party error exposes a cyclic source chain.
    for _ in 0..16 {
        let Some(cause) = source else {
            break;
        };
        causes.push(cause.to_string());
        source = cause.source();
    }

    causes
}

fn reqwest_error_kinds(error: &reqwest::Error) -> String {
    let mut kinds = Vec::new();

    if error.is_builder() {
        kinds.push("builder");
    }
    if error.is_connect() {
        kinds.push("connect");
    }
    if error.is_request() {
        kinds.push("request");
    }
    if error.is_redirect() {
        kinds.push("redirect");
    }
    if error.is_timeout() {
        kinds.push("timeout");
    }
    if error.is_status() {
        kinds.push("status");
    }
    if error.is_body() {
        kinds.push("body");
    }
    if error.is_decode() {
        kinds.push("decode");
    }
    if error.is_upgrade() {
        kinds.push("upgrade");
    }

    if kinds.is_empty() {
        "unknown".to_string()
    } else {
        kinds.join(", ")
    }
}

fn sanitized_url(url: &reqwest::Url) -> String {
    let mut url = url.clone();

    // Custom endpoints should not contain credentials or query-string tokens,
    // but omit them from diagnostics in case one does.
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_query(None);
    url.set_fragment(None);

    url.to_string()
}

fn sanitized_url_for_log(url: &str) -> String {
    reqwest::Url::parse(url)
        .map(|url| sanitized_url(&url))
        // Do not echo an invalid URL: the parse failure might have been caused
        // by sensitive data entered in the custom endpoint field.
        .unwrap_or_else(|_| "<invalid URL>".to_string())
}

fn report_reqwest_error(context: &str, error: &reqwest::Error) -> String {
    let kinds = reqwest_error_kinds(error);
    let url = error
        .url()
        .map(sanitized_url)
        .map(|url| format!(", url: {url}"))
        .unwrap_or_default();

    // serde_json's error text can quote values from a malformed response. That
    // response may contain transcription content, so retain the useful decode
    // classification but never put its nested source in logs or UI errors.
    let causes = if error.is_decode() {
        Vec::new()
    } else {
        error_source_chain(error)
    };
    let cause_details = if !causes.is_empty() {
        format!(": caused by: {}", causes.join(" -> "))
    } else if error.url().is_none() {
        // Reqwest's short Display text is safe when it cannot append a raw URL.
        format!(": {error}")
    } else {
        // The sanitized URL is already included above. Avoid formatting the
        // original error because its Display implementation includes the raw URL.
        String::new()
    };

    let details = format!("{context} (kind: {kinds}{url}){cause_details}");
    error!("{details}");
    details
}

/// Send a chat completion with Extended Thinking / Reasoning support
pub async fn send_chat_completion_with_reasoning(
    provider: &PostProcessProvider,
    api_key: String,
    model: &str,
    prompt: String,
    reasoning: ReasoningConfig,
) -> Result<Option<String>, String> {
    send_chat_completion_with_messages_internal(
        provider,
        api_key,
        model,
        vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }],
        reasoning,
    )
    .await
}

/// Send a chat completion with system/user prompts and Extended Thinking support
pub async fn send_chat_completion_with_system_and_reasoning(
    provider: &PostProcessProvider,
    api_key: String,
    model: &str,
    system_prompt: String,
    user_prompt: String,
    reasoning: ReasoningConfig,
) -> Result<Option<String>, String> {
    let mut messages = Vec::new();

    if !system_prompt.trim().is_empty() {
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: system_prompt,
        });
    }

    messages.push(ChatMessage {
        role: "user".to_string(),
        content: user_prompt,
    });

    send_chat_completion_with_messages_internal(provider, api_key, model, messages, reasoning).await
}

/// Internal function that sends the actual chat completion request
/// with optional reasoning and fail-soft retry
async fn send_chat_completion_with_messages_internal(
    provider: &PostProcessProvider,
    api_key: String,
    model: &str,
    messages: Vec<ChatMessage>,
    reasoning: ReasoningConfig,
) -> Result<Option<String>, String> {
    let base_url = canonical_llm_provider_base_url(provider)?;
    let url = format!("{}/chat/completions", base_url);

    debug!(
        "Sending chat completion request to: {}",
        sanitized_url_for_log(&url)
    );

    let client = create_client(provider, &api_key)?;

    // Calculate max_tokens: if reasoning is enabled, ensure enough room for answer
    // Formula: max(4000, reasoning_budget + 2000)
    let (max_tokens, reasoning_effort, reasoning_params) = if reasoning.enabled {
        let budget = reasoning.budget.max(1024);
        let total = (budget + 2000).max(4000);
        debug!(
            "Extended Thinking enabled: reasoning_budget={}, max_tokens={}",
            budget, total
        );
        (
            Some(total),
            None,
            Some(ReasoningParams {
                max_tokens: Some(budget),
                ..Default::default()
            }),
        )
    } else if reasoning.disable_by_default_on_compatible_providers {
        match provider.id.as_str() {
            "custom" => {
                debug!(
                    "Disabling default provider reasoning for post-processing on '{}'",
                    provider.id
                );
                (None, Some("none".to_string()), None)
            }
            "openrouter" => {
                debug!(
                    "Disabling default provider reasoning for post-processing on '{}'",
                    provider.id
                );
                (
                    None,
                    None,
                    Some(ReasoningParams {
                        effort: Some("none".to_string()),
                        exclude: Some(true),
                        ..Default::default()
                    }),
                )
            }
            _ => (None, None, None),
        }
    } else {
        (None, None, None)
    };

    let request_body = ChatCompletionRequest {
        model: model.to_string(),
        messages: messages.clone(),
        stream: false,
        max_tokens,
        reasoning_effort,
        reasoning: reasoning_params,
    };

    let response = client
        .post(&url)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| report_reqwest_error("HTTP request failed", &e))?;
    let status = response.status();
    debug!(
        "Chat completion response received with status {} over {:?} from {}",
        status,
        response.version(),
        sanitized_url(response.url())
    );

    // Fail-soft retry: if we get 400 and reasoning was enabled, retry without reasoning
    if status.as_u16() == 400
        && (request_body.reasoning.is_some() || request_body.reasoning_effort.is_some())
    {
        let error_text = response.text().await.unwrap_or_else(|e| {
            report_reqwest_error("Failed to read reasoning rejection response", &e)
        });
        warn!(
            "Reasoning-configured request failed with status {}: {}. Retrying without reasoning controls",
            status, error_text
        );

        // Retry without reasoning
        let fallback_request = ChatCompletionRequest {
            model: model.to_string(),
            messages,
            stream: false,
            max_tokens: None,
            reasoning_effort: None,
            reasoning: None,
        };

        let fallback_response = client
            .post(&url)
            .json(&fallback_request)
            .send()
            .await
            .map_err(|e| report_reqwest_error("HTTP retry failed", &e))?;

        let fallback_status = fallback_response.status();
        debug!(
            "Chat completion retry response received with status {} over {:?} from {}",
            fallback_status,
            fallback_response.version(),
            sanitized_url(fallback_response.url())
        );
        if !fallback_status.is_success() {
            let fallback_error = fallback_response
                .text()
                .await
                .unwrap_or_else(|e| report_reqwest_error("Failed to read retry error", &e));
            return Err(format!(
                "API request failed with status {}: {}",
                fallback_status, fallback_error
            ));
        }

        let completion: ChatCompletionResponse = fallback_response
            .json()
            .await
            .map_err(|e| report_reqwest_error("Failed to parse retry response", &e))?;

        return Ok(completion
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone()));
    }

    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|e| report_reqwest_error("Failed to read API error response", &e));
        return Err(format!(
            "API request failed with status {}: {}",
            status, error_text
        ));
    }

    let completion: ChatCompletionResponse = response
        .json()
        .await
        .map_err(|e| report_reqwest_error("Failed to parse API response", &e))?;

    // Log reasoning tokens if present (but don't include in response)
    if let Some(choice) = completion.choices.first() {
        if let Some(ref reasoning_text) = choice.message.reasoning {
            let reasoning_preview = if reasoning_text.len() > 200 {
                let end = reasoning_text
                    .char_indices()
                    .map(|(i, _)| i)
                    .find(|&i| i >= 200)
                    .unwrap_or(reasoning_text.len());
                format!(
                    "{}... ({} chars total)",
                    &reasoning_text[..end],
                    reasoning_text.len()
                )
            } else {
                reasoning_text.clone()
            };
            info!("Extended Thinking reasoning tokens: {}", reasoning_preview);
        }
    }

    Ok(completion
        .choices
        .first()
        .and_then(|choice| choice.message.content.clone()))
}

/// Fetch available models from an OpenAI-compatible API
/// Returns a list of model IDs
pub async fn fetch_models(
    provider: &PostProcessProvider,
    api_key: String,
) -> Result<Vec<String>, String> {
    let base_url = canonical_llm_provider_base_url(provider)?;
    let url = format!("{}/models", base_url);

    debug!("Fetching models from: {}", sanitized_url_for_log(&url));

    let client = create_client(provider, &api_key)?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| report_reqwest_error("Failed to fetch models", &e))?;

    let status = response.status();
    debug!(
        "Model list response received with status {} over {:?} from {}",
        status,
        response.version(),
        sanitized_url(response.url())
    );
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|e| report_reqwest_error("Failed to read model list error", &e));
        return Err(format!(
            "Model list request failed ({}): {}",
            status, error_text
        ));
    }

    let parsed: serde_json::Value = response
        .json()
        .await
        .map_err(|e| report_reqwest_error("Failed to parse model list response", &e))?;

    let mut models = Vec::new();

    // Handle OpenAI format: { data: [ { id: "..." }, ... ] }
    if let Some(data) = parsed.get("data").and_then(|d| d.as_array()) {
        for entry in data {
            if let Some(id) = entry.get("id").and_then(|i| i.as_str()) {
                models.push(id.to_string());
            } else if let Some(name) = entry.get("name").and_then(|n| n.as_str()) {
                models.push(name.to_string());
            }
        }
    }
    // Handle array format: [ "model1", "model2", ... ]
    else if let Some(array) = parsed.as_array() {
        for entry in array {
            if let Some(model) = entry.as_str() {
                models.push(model.to_string());
            }
        }
    }

    Ok(models)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[derive(Debug)]
    struct TestError {
        message: &'static str,
        source: Option<Box<TestError>>,
    }

    impl fmt::Display for TestError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(self.message)
        }
    }

    impl StdError for TestError {
        fn source(&self) -> Option<&(dyn StdError + 'static)> {
            self.source
                .as_deref()
                .map(|source| source as &(dyn StdError + 'static))
        }
    }

    fn request_json() -> serde_json::Value {
        let request = ChatCompletionRequest {
            model: "test-model".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "hi".to_string(),
            }],
            stream: false,
            max_tokens: None,
            reasoning_effort: None,
            reasoning: None,
        };
        serde_json::to_value(&request).unwrap()
    }

    async fn serve_one_response(status: &str, body: &str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request).await.unwrap();
            stream.write_all(response.as_bytes()).await.unwrap();
        });

        format!("http://{address}")
    }

    #[test]
    fn error_source_chain_includes_all_nested_causes() {
        let error = TestError {
            message: "request failed",
            source: Some(Box::new(TestError {
                message: "TLS handshake failed",
                source: Some(Box::new(TestError {
                    message: "unknown certificate authority",
                    source: None,
                })),
            })),
        };

        assert_eq!(
            error_source_chain(&error),
            vec!["TLS handshake failed", "unknown certificate authority"]
        );
    }

    #[test]
    fn log_url_sanitization_removes_credentials_and_tokens() {
        let url = "https://user:password@example.com/v1/models?api_key=secret#private";
        assert_eq!(sanitized_url_for_log(url), "https://example.com/v1/models");
    }

    #[test]
    fn invalid_log_urls_are_not_echoed() {
        assert_eq!(
            sanitized_url_for_log("not a URL containing secret"),
            "<invalid URL>"
        );
    }

    #[tokio::test]
    async fn decode_error_does_not_echo_response_values() {
        let base_url =
            serve_one_response("200 OK", r#"{"choices":"PRIVATE TRANSCRIPTION CONTENT"}"#).await;
        let error = reqwest::get(base_url)
            .await
            .unwrap()
            .json::<ChatCompletionResponse>()
            .await
            .unwrap_err();

        let details = report_reqwest_error("Failed to parse API response", &error);
        assert!(details.contains("kind: decode"));
        assert!(!details.contains("PRIVATE TRANSCRIPTION CONTENT"));
    }

    #[tokio::test]
    async fn raw_error_url_is_not_reintroduced_without_a_source() {
        let base_url = serve_one_response("400 Bad Request", "bad request").await;
        let error = reqwest::get(format!(
            "{base_url}/private?api_key=SECRET_QUERY_TOKEN#private"
        ))
        .await
        .unwrap()
        .error_for_status()
        .unwrap_err();

        let details = report_reqwest_error("Request failed", &error);
        assert!(details.contains(&format!("url: {base_url}/private")));
        assert!(!details.contains("SECRET_QUERY_TOKEN"));
        assert!(!details.contains("#private"));
    }

    #[test]
    fn requests_explicitly_disable_streaming() {
        let json = request_json();
        assert_eq!(json["stream"], false);
    }
}
