use crate::settings::PostProcessProvider;
use crate::url_security::canonical_llm_provider_base_url;
use futures_util::StreamExt;
use log::{debug, error, info, warn};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, REFERER, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::error::Error as StdError;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

/// Configuration for provider-specific Extended Thinking / Reasoning controls.
#[derive(Debug, Clone, Default)]
pub struct ReasoningConfig {
    pub enabled: bool,
    pub budget: u32, // min 1024 for OpenRouter/Anthropic
    pub disable_by_default_on_compatible_providers: bool,
}

impl ReasoningConfig {
    fn new(enabled: bool, budget: u32) -> Self {
        Self {
            enabled,
            budget: if enabled { budget.max(1024) } else { budget },
            disable_by_default_on_compatible_providers: false,
        }
    }

    pub fn from_user_toggle(enabled: bool, budget: u32) -> Self {
        Self::new(enabled, budget).with_disable_by_default_on_compatible_providers(true)
    }

    fn with_disable_by_default_on_compatible_providers(mut self, disable: bool) -> Self {
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
    max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<ReasoningParams>,
}

#[derive(Debug, Default)]
struct ReasoningRequestOptions {
    max_tokens: Option<u32>,
    max_completion_tokens: Option<u32>,
    reasoning_effort: Option<String>,
    reasoning: Option<ReasoningParams>,
}

fn groq_reasoning_effort(model: &str, enabled: bool) -> Option<&'static str> {
    let model = model.trim();

    if model.eq_ignore_ascii_case("qwen/qwen3.6-27b") {
        return Some(if enabled { "default" } else { "none" });
    }

    if enabled
        && (model.eq_ignore_ascii_case("openai/gpt-oss-20b")
            || model.eq_ignore_ascii_case("openai/gpt-oss-120b"))
    {
        return Some("medium");
    }

    None
}

fn reasoning_request_options(
    provider_id: &str,
    model: &str,
    reasoning: &ReasoningConfig,
) -> ReasoningRequestOptions {
    if reasoning.enabled {
        let budget = reasoning.budget.max(1024);
        let total = (budget + 2000).max(4000);

        if provider_id == "groq" {
            return if let Some(effort) = groq_reasoning_effort(model, true) {
                debug!(
                    "Extended Thinking enabled for Groq with effort='{}', max_completion_tokens={}",
                    effort, total
                );
                ReasoningRequestOptions {
                    max_completion_tokens: Some(total),
                    reasoning_effort: Some(effort.to_string()),
                    ..Default::default()
                }
            } else {
                ReasoningRequestOptions::default()
            };
        }

        debug!(
            "Extended Thinking enabled: reasoning_budget={}, max_tokens={}",
            budget, total
        );
        return ReasoningRequestOptions {
            max_tokens: Some(total),
            reasoning: Some(ReasoningParams {
                max_tokens: Some(budget),
                ..Default::default()
            }),
            ..Default::default()
        };
    }

    if !reasoning.disable_by_default_on_compatible_providers {
        return ReasoningRequestOptions::default();
    }

    match provider_id {
        "custom" => {
            debug!(
                "Disabling default provider reasoning for post-processing on '{}'",
                provider_id
            );
            ReasoningRequestOptions {
                reasoning_effort: Some("none".to_string()),
                ..Default::default()
            }
        }
        "groq" => {
            if let Some(effort) = groq_reasoning_effort(model, false) {
                debug!(
                    "Disabling default provider reasoning for post-processing on '{}'",
                    provider_id
                );
                ReasoningRequestOptions {
                    reasoning_effort: Some(effort.to_string()),
                    ..Default::default()
                }
            } else {
                ReasoningRequestOptions::default()
            }
        }
        "openrouter" => {
            debug!(
                "Disabling default provider reasoning for post-processing on '{}'",
                provider_id
            );
            ReasoningRequestOptions {
                reasoning: Some(ReasoningParams {
                    effort: Some("none".to_string()),
                    exclude: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            }
        }
        _ => ReasoningRequestOptions::default(),
    }
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
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
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
        vec![user_message(prompt)],
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
    let messages = system_and_user_messages(system_prompt, user_prompt);
    send_chat_completion_with_messages_internal(provider, api_key, model, messages, reasoning).await
}

fn user_message(prompt: String) -> ChatMessage {
    ChatMessage {
        role: "user".to_string(),
        content: prompt,
    }
}

fn system_and_user_messages(system_prompt: String, user_prompt: String) -> Vec<ChatMessage> {
    let mut messages = Vec::new();

    if !system_prompt.trim().is_empty() {
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: system_prompt,
        });
    }

    messages.push(user_message(user_prompt));
    messages
}

/// Whether a 400 answer may be retried without the optional reasoning controls.
/// Never remove Groq's explicit Qwen OFF control; omitting it enables reasoning again.
fn may_retry_without_reasoning(
    provider: &PostProcessProvider,
    reasoning: &ReasoningConfig,
    request: &ChatCompletionRequest,
) -> bool {
    let must_preserve_groq_qwen_off = provider.id == "groq"
        && !reasoning.enabled
        && request.reasoning_effort.as_deref() == Some("none");
    (request.reasoning.is_some() || request.reasoning_effort.is_some())
        && !must_preserve_groq_qwen_off
}

fn log_reasoning_preview(reasoning_text: &str) {
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
        reasoning_text.to_string()
    };
    info!("Extended Thinking reasoning tokens: {}", reasoning_preview);
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
    send_chat_completion_with_messages_at_base_url(
        provider, api_key, model, messages, reasoning, &base_url,
    )
    .await
}

async fn send_chat_completion_with_messages_at_base_url(
    provider: &PostProcessProvider,
    api_key: String,
    model: &str,
    messages: Vec<ChatMessage>,
    reasoning: ReasoningConfig,
    base_url: &str,
) -> Result<Option<String>, String> {
    let url = format!("{}/chat/completions", base_url);

    debug!(
        "Sending chat completion request to: {}",
        sanitized_url_for_log(&url)
    );

    let client = create_client(provider, &api_key)?;

    let reasoning_options = reasoning_request_options(&provider.id, model, &reasoning);

    let request_body = ChatCompletionRequest {
        model: model.to_string(),
        messages: messages.clone(),
        stream: false,
        max_tokens: reasoning_options.max_tokens,
        max_completion_tokens: reasoning_options.max_completion_tokens,
        reasoning_effort: reasoning_options.reasoning_effort,
        reasoning: reasoning_options.reasoning,
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

    // Fail-soft retry: on 400, retry without optional reasoning controls.
    if status.as_u16() == 400 && may_retry_without_reasoning(provider, &reasoning, &request_body) {
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
            max_completion_tokens: None,
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
            log_reasoning_preview(reasoning_text);
        }
    }

    Ok(completion
        .choices
        .first()
        .and_then(|choice| choice.message.content.clone()))
}

/// Answer text from a streamed chat completion.
#[derive(Debug, Default)]
pub struct StreamedCompletion {
    pub content: Option<String>,
    /// When the first answer text arrived. `None` when the provider answered
    /// without streaming, so there is no meaningful time to first token.
    pub first_token_at: Option<Instant>,
    /// The answer arrived as an event stream rather than one regular body.
    pub streamed: bool,
    /// The caller's callback asked to stop before the answer was complete.
    pub stopped: bool,
}

/// Stream a chat completion with Extended Thinking / Reasoning support.
///
/// `on_content` receives each piece of answer text as it arrives and returns
/// `false` to stop. A provider or model that rejects streaming is answered by
/// a regular request instead, delivered to `on_content` as one piece.
pub async fn stream_chat_completion_with_reasoning(
    provider: &PostProcessProvider,
    api_key: String,
    model: &str,
    prompt: String,
    reasoning: ReasoningConfig,
    on_content: &mut (dyn FnMut(&str) -> bool + Send),
) -> Result<StreamedCompletion, String> {
    let base_url = canonical_llm_provider_base_url(provider)?;
    stream_chat_completion_with_messages_at_base_url(
        provider,
        api_key,
        model,
        vec![user_message(prompt)],
        reasoning,
        &base_url,
        on_content,
    )
    .await
}

/// Stream a chat completion with system/user prompts; see
/// [`stream_chat_completion_with_reasoning`].
pub async fn stream_chat_completion_with_system_and_reasoning(
    provider: &PostProcessProvider,
    api_key: String,
    model: &str,
    system_prompt: String,
    user_prompt: String,
    reasoning: ReasoningConfig,
    on_content: &mut (dyn FnMut(&str) -> bool + Send),
) -> Result<StreamedCompletion, String> {
    let base_url = canonical_llm_provider_base_url(provider)?;
    stream_chat_completion_with_messages_at_base_url(
        provider,
        api_key,
        model,
        system_and_user_messages(system_prompt, user_prompt),
        reasoning,
        &base_url,
        on_content,
    )
    .await
}

/// Endpoint/model pairs that rejected a streaming request but answered a
/// regular one. Later requests skip the doomed streaming attempt.
static STREAMING_UNSUPPORTED: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(Default::default);

fn streaming_support_key(base_url: &str, model: &str) -> String {
    format!("{}\n{}", base_url.trim_end_matches('/'), model.trim())
}

fn streaming_known_unsupported(key: &str) -> bool {
    STREAMING_UNSUPPORTED
        .lock()
        .map(|keys| keys.contains(key))
        .unwrap_or(false)
}

fn remember_streaming_unsupported(key: String) {
    if let Ok(mut keys) = STREAMING_UNSUPPORTED.lock() {
        keys.insert(key);
    }
}

/// Statuses with which servers refuse a request they cannot stream. Auth,
/// rate-limit and server errors would fail the same way without streaming.
fn may_reject_streaming(status: reqwest::StatusCode) -> bool {
    matches!(status.as_u16(), 400 | 405 | 406 | 415 | 422 | 501)
}

async fn stream_chat_completion_with_messages_at_base_url(
    provider: &PostProcessProvider,
    api_key: String,
    model: &str,
    messages: Vec<ChatMessage>,
    reasoning: ReasoningConfig,
    base_url: &str,
    on_content: &mut (dyn FnMut(&str) -> bool + Send),
) -> Result<StreamedCompletion, String> {
    let support_key = streaming_support_key(base_url, model);
    if streaming_known_unsupported(&support_key) {
        debug!("Streaming was rejected earlier for this model; sending a regular request");
        return complete_without_streaming(
            provider, api_key, model, messages, reasoning, base_url, on_content,
        )
        .await;
    }

    let url = format!("{}/chat/completions", base_url);
    debug!(
        "Sending streaming chat completion request to: {}",
        sanitized_url_for_log(&url)
    );

    let client = create_client(provider, &api_key)?;
    let reasoning_options = reasoning_request_options(&provider.id, model, &reasoning);
    let request_body = ChatCompletionRequest {
        model: model.to_string(),
        messages: messages.clone(),
        stream: true,
        max_tokens: reasoning_options.max_tokens,
        max_completion_tokens: reasoning_options.max_completion_tokens,
        reasoning_effort: reasoning_options.reasoning_effort,
        reasoning: reasoning_options.reasoning,
    };

    let mut response = client
        .post(&url)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| report_reqwest_error("HTTP streaming request failed", &e))?;
    let mut status = response.status();
    debug!(
        "Streaming chat completion response received with status {} over {:?} from {}",
        status,
        response.version(),
        sanitized_url(response.url())
    );

    // Fail-soft retry, as for regular requests: on 400, retry without reasoning controls.
    if status.as_u16() == 400 && may_retry_without_reasoning(provider, &reasoning, &request_body) {
        let error_text = response.text().await.unwrap_or_else(|e| {
            report_reqwest_error("Failed to read reasoning rejection response", &e)
        });
        warn!(
            "Reasoning-configured streaming request failed with status {}: {}. Retrying without reasoning controls",
            status, error_text
        );

        let fallback_request = ChatCompletionRequest {
            model: model.to_string(),
            messages: messages.clone(),
            stream: true,
            max_tokens: None,
            max_completion_tokens: None,
            reasoning_effort: None,
            reasoning: None,
        };
        response = client
            .post(&url)
            .json(&fallback_request)
            .send()
            .await
            .map_err(|e| report_reqwest_error("HTTP streaming retry failed", &e))?;
        status = response.status();
        debug!(
            "Streaming chat completion retry response received with status {} over {:?} from {}",
            status,
            response.version(),
            sanitized_url(response.url())
        );
    }

    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|e| report_reqwest_error("Failed to read API error response", &e));
        if !may_reject_streaming(status) {
            return Err(format!(
                "API request failed with status {}: {}",
                status, error_text
            ));
        }

        warn!(
            "Streaming request failed with status {}: {}. Retrying without streaming",
            status, error_text
        );
        let result = complete_without_streaming(
            provider, api_key, model, messages, reasoning, base_url, on_content,
        )
        .await;
        if result.is_ok() {
            info!("Streaming is not supported for this model; later requests will not stream");
            remember_streaming_unsupported(support_key);
        }
        return result;
    }

    let (completion, answered_without_streaming) =
        read_completion_stream(response, on_content).await?;
    if answered_without_streaming {
        info!("Provider answered a streaming request without streaming");
        remember_streaming_unsupported(support_key);
    }
    Ok(completion)
}

async fn complete_without_streaming(
    provider: &PostProcessProvider,
    api_key: String,
    model: &str,
    messages: Vec<ChatMessage>,
    reasoning: ReasoningConfig,
    base_url: &str,
    on_content: &mut (dyn FnMut(&str) -> bool + Send),
) -> Result<StreamedCompletion, String> {
    let content = send_chat_completion_with_messages_at_base_url(
        provider, api_key, model, messages, reasoning, base_url,
    )
    .await?;
    let stopped = match content.as_deref() {
        Some(text) if !text.is_empty() => !on_content(text),
        _ => false,
    };
    Ok(StreamedCompletion {
        content,
        first_token_at: None,
        streamed: false,
        stopped,
    })
}

/// Reads a response to a streaming request. Returns whether the provider
/// ignored the request and answered with one regular JSON body.
async fn read_completion_stream(
    response: reqwest::Response,
    on_content: &mut (dyn FnMut(&str) -> bool + Send),
) -> Result<(StreamedCompletion, bool), String> {
    let mut parser = CompletionStreamParser::default();
    let mut body = response.bytes_stream();

    while let Some(chunk) = body.next().await {
        let chunk =
            chunk.map_err(|e| report_reqwest_error("Failed to read streamed API response", &e))?;
        parser.push(&chunk, on_content)?;
        // Stop at [DONE] even if the server keeps the connection open, and drop
        // the connection when the caller stops so the provider stops generating.
        if parser.is_finished() {
            break;
        }
    }

    let answered_without_streaming = parser.kind == ResponseBodyKind::Json;
    let completion = parser.finish(on_content)?;
    debug!(
        "Streamed chat completion finished with {} chars{}",
        completion
            .content
            .as_deref()
            .map_or(0, |text| text.chars().count()),
        if completion.stopped {
            " (stopped by caller)"
        } else {
            ""
        }
    );
    Ok((completion, answered_without_streaming))
}

/// Upper bound for one buffered event-stream line or a regular JSON body.
const MAX_BUFFERED_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

#[derive(Debug, Default, PartialEq)]
enum ResponseBodyKind {
    #[default]
    Unknown,
    EventStream,
    Json,
}

/// Incremental parser for OpenAI-compatible `text/event-stream` completions.
///
/// It also accepts a regular JSON completion, because some servers ignore
/// `stream: true`, and decides by the first non-blank byte rather than trusting
/// the Content-Type header. Errors never quote response text, which may contain
/// the user's transcription.
#[derive(Default)]
struct CompletionStreamParser {
    kind: ResponseBodyKind,
    buffer: Vec<u8>,
    pending_data: String,
    saw_data: bool,
    done: bool,
    stopped: bool,
    content: Option<String>,
    first_token_at: Option<Instant>,
    finish_reason: Option<String>,
    reasoning: String,
}

impl CompletionStreamParser {
    fn is_finished(&self) -> bool {
        self.done || self.stopped
    }

    fn push(
        &mut self,
        bytes: &[u8],
        on_content: &mut (dyn FnMut(&str) -> bool + Send),
    ) -> Result<(), String> {
        if self.is_finished() {
            return Ok(());
        }
        self.buffer.extend_from_slice(bytes);

        if self.kind == ResponseBodyKind::Unknown {
            let Some(&first) = self
                .buffer
                .iter()
                .find(|byte| !byte.is_ascii_whitespace() && !UTF8_BOM.contains(byte))
            else {
                return Ok(());
            };
            if self.buffer.starts_with(UTF8_BOM) {
                self.buffer.drain(..UTF8_BOM.len());
            }
            self.kind = if first == b'{' {
                ResponseBodyKind::Json
            } else {
                ResponseBodyKind::EventStream
            };
        }

        if self.kind == ResponseBodyKind::EventStream {
            while let Some(newline) = self.buffer.iter().position(|&byte| byte == b'\n') {
                let line: Vec<u8> = self.buffer.drain(..=newline).collect();
                self.handle_line(&line, on_content)?;
                if self.is_finished() {
                    self.buffer.clear();
                    return Ok(());
                }
            }
        }

        if self.buffer.len() > MAX_BUFFERED_RESPONSE_BYTES {
            return Err("API response is too large".to_string());
        }
        Ok(())
    }

    fn handle_line(
        &mut self,
        line: &[u8],
        on_content: &mut (dyn FnMut(&str) -> bool + Send),
    ) -> Result<(), String> {
        let line = line.strip_suffix(b"\n").unwrap_or(line);
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let line = String::from_utf8_lossy(line);

        if line.is_empty() {
            return self.dispatch_pending(on_content);
        }
        if line.starts_with(':') {
            // Comment or keep-alive, e.g. ": OPENROUTER PROCESSING".
            return Ok(());
        }
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line.as_ref(), ""),
        };
        if field != "data" {
            return Ok(());
        }

        self.saw_data = true;
        if !self.pending_data.is_empty() {
            self.pending_data.push('\n');
        }
        self.pending_data.push_str(value);

        // Some servers omit the blank line between events, so dispatch as soon
        // as the data is complete. Multi-line data waits for more lines.
        if self.pending_data.trim() == "[DONE]" {
            self.pending_data.clear();
            self.done = true;
        } else if let Ok(event) = serde_json::from_str::<Value>(&self.pending_data) {
            self.pending_data.clear();
            self.handle_event(&event, on_content)?;
        }
        Ok(())
    }

    fn dispatch_pending(
        &mut self,
        on_content: &mut (dyn FnMut(&str) -> bool + Send),
    ) -> Result<(), String> {
        let data = std::mem::take(&mut self.pending_data);
        let data = data.trim();
        if data.is_empty() {
            return Ok(());
        }
        if data == "[DONE]" {
            self.done = true;
            return Ok(());
        }
        let event = serde_json::from_str::<Value>(data).map_err(|_| {
            "Failed to parse streamed API response: an event was not valid JSON".to_string()
        })?;
        self.handle_event(&event, on_content)
    }

    fn handle_event(
        &mut self,
        event: &Value,
        on_content: &mut (dyn FnMut(&str) -> bool + Send),
    ) -> Result<(), String> {
        if let Some(error) = event.get("error").filter(|error| !error.is_null()) {
            return Err(format!(
                "API stream reported an error: {}",
                api_error_message(error)
            ));
        }

        let Some(choice) = event
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| {
                choices
                    .iter()
                    .find(|choice| choice.get("index").and_then(Value::as_u64).unwrap_or(0) == 0)
            })
        else {
            // For example the final usage-only event.
            return Ok(());
        };

        if let Some(message) = choice.get("delta").or_else(|| choice.get("message")) {
            if let Some(reasoning) = message
                .get("reasoning")
                .or_else(|| message.get("reasoning_content"))
                .and_then(Value::as_str)
            {
                self.reasoning.push_str(reasoning);
            }
            if let Some(text) = message.get("content").and_then(content_text) {
                self.push_content(&text, on_content);
            }
        }

        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            if reason == "error" {
                return Err("API stream finished with an error".to_string());
            }
            self.finish_reason = Some(reason.to_string());
        }
        Ok(())
    }

    fn push_content(&mut self, text: &str, on_content: &mut (dyn FnMut(&str) -> bool + Send)) {
        let content = self.content.get_or_insert_with(String::new);
        if text.is_empty() || self.stopped {
            return;
        }
        self.first_token_at.get_or_insert_with(Instant::now);
        content.push_str(text);
        if !on_content(text) {
            self.stopped = true;
        }
    }

    fn finish(
        mut self,
        on_content: &mut (dyn FnMut(&str) -> bool + Send),
    ) -> Result<StreamedCompletion, String> {
        match self.kind {
            ResponseBodyKind::Unknown => {
                return Err("API returned an empty response".to_string());
            }
            ResponseBodyKind::Json => {
                let body = std::mem::take(&mut self.buffer);
                let response = serde_json::from_slice::<Value>(&body).map_err(|_| {
                    "Failed to parse API response: the body was not valid JSON".to_string()
                })?;
                self.handle_event(&response, on_content)?;
                // The whole answer arrived at once; there was no first token.
                self.first_token_at = None;
            }
            ResponseBodyKind::EventStream => {
                if !self.is_finished() && !self.buffer.is_empty() {
                    let line = std::mem::take(&mut self.buffer);
                    self.handle_line(&line, on_content)?;
                }
                if !self.is_finished() {
                    self.dispatch_pending(on_content)?;
                }
                if !self.saw_data {
                    return Err(
                        "API returned a response that is neither JSON nor an event stream"
                            .to_string(),
                    );
                }
                if !self.is_finished() && self.finish_reason.is_none() {
                    warn!("Streamed API response ended without a completion marker");
                }
            }
        }

        if self.finish_reason.as_deref() == Some("length") {
            warn!("API response was cut off by the output token limit");
        }
        if !self.reasoning.is_empty() {
            log_reasoning_preview(&self.reasoning);
        }

        Ok(StreamedCompletion {
            content: self.content,
            first_token_at: self.first_token_at,
            streamed: self.kind == ResponseBodyKind::EventStream,
            stopped: self.stopped,
        })
    }
}

/// Text of a message `content`, which is a string or, for some providers, a
/// list of typed parts of which only the text parts are the answer.
fn content_text(content: &Value) -> Option<String> {
    match content {
        Value::String(text) => Some(text.clone()),
        Value::Array(parts) => {
            let texts: Vec<&str> = parts
                .iter()
                .filter(|part| {
                    part.get("type")
                        .and_then(Value::as_str)
                        .map_or(true, |kind| kind == "text")
                })
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect();
            (!texts.is_empty()).then(|| texts.concat())
        }
        _ => None,
    }
}

fn api_error_message(error: &Value) -> String {
    if let Some(message) = error.as_str() {
        return message.to_string();
    }
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("unknown error");
    match error.get("code") {
        Some(Value::String(code)) => format!("{message} ({code})"),
        Some(Value::Number(code)) => format!("{message} ({code})"),
        _ => message.to_string(),
    }
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
        .timeout(Duration::from_secs(60))
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
    use tokio::sync::{mpsc, oneshot};

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

    fn request_json_with_reasoning_options(
        reasoning_options: ReasoningRequestOptions,
    ) -> serde_json::Value {
        let request = ChatCompletionRequest {
            model: "test-model".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "hi".to_string(),
            }],
            stream: false,
            max_tokens: reasoning_options.max_tokens,
            max_completion_tokens: reasoning_options.max_completion_tokens,
            reasoning_effort: reasoning_options.reasoning_effort,
            reasoning: reasoning_options.reasoning,
        };
        serde_json::to_value(&request).unwrap()
    }

    fn request_json() -> serde_json::Value {
        request_json_with_reasoning_options(ReasoningRequestOptions::default())
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

    #[derive(Debug)]
    struct CapturedJsonRequest {
        request_line: String,
        body: serde_json::Value,
    }

    async fn serve_one_captured_json_response(
        status: &str,
        body: &str,
    ) -> (String, oneshot::Receiver<CapturedJsonRequest>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let (request_sender, request_receiver) = oneshot::channel();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            request_sender
                .send(read_captured_json_request(&mut stream).await)
                .unwrap();
            stream.write_all(response.as_bytes()).await.unwrap();
        });

        (format!("http://{address}"), request_receiver)
    }

    async fn read_captured_json_request(stream: &mut tokio::net::TcpStream) -> CapturedJsonRequest {
        let mut request = Vec::new();
        let mut chunk = [0_u8; 2048];

        let header_end = loop {
            let bytes_read = stream.read(&mut chunk).await.unwrap();
            assert!(
                bytes_read > 0,
                "mock client disconnected before sending headers"
            );
            request.extend_from_slice(&chunk[..bytes_read]);

            if let Some(position) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                break position + 4;
            }
        };

        let (request_line, content_length) = {
            let headers = std::str::from_utf8(&request[..header_end]).unwrap();
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .expect("mock request must include Content-Length");
            let request_line = headers.lines().next().unwrap().to_string();
            (request_line, content_length)
        };

        while request.len() < header_end + content_length {
            let bytes_read = stream.read(&mut chunk).await.unwrap();
            assert!(
                bytes_read > 0,
                "mock client disconnected before sending its body"
            );
            request.extend_from_slice(&chunk[..bytes_read]);
        }

        let body =
            serde_json::from_slice(&request[header_end..header_end + content_length]).unwrap();
        CapturedJsonRequest { request_line, body }
    }

    struct MockResponse {
        status: &'static str,
        content_type: &'static str,
        /// Written one piece at a time, so the client reads the body incrementally.
        body_pieces: Vec<String>,
    }

    fn mock_response(status: &'static str, content_type: &'static str, body: &str) -> MockResponse {
        MockResponse {
            status,
            content_type,
            body_pieces: vec![body.to_string()],
        }
    }

    fn mock_event_stream(events: &[&str]) -> MockResponse {
        MockResponse {
            status: "200 OK",
            content_type: "text/event-stream",
            body_pieces: events
                .iter()
                .map(|event| format!("data: {event}\n\n"))
                .collect(),
        }
    }

    /// Serves one response per connection, in order, with a close-delimited body.
    async fn serve_captured_responses(
        responses: Vec<MockResponse>,
    ) -> (String, mpsc::UnboundedReceiver<CapturedJsonRequest>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (request_sender, request_receiver) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            for response in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                request_sender
                    .send(read_captured_json_request(&mut stream).await)
                    .unwrap();
                let head = format!(
                    "HTTP/1.1 {}\r\nContent-Type: {}\r\nConnection: close\r\n\r\n",
                    response.status, response.content_type
                );
                stream.write_all(head.as_bytes()).await.unwrap();
                // A client that stops reading may close the connection mid-body.
                for piece in response.body_pieces {
                    if stream.write_all(piece.as_bytes()).await.is_err() {
                        break;
                    }
                    let _ = stream.flush().await;
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
                let _ = stream.shutdown().await;
            }
        });

        (format!("http://{address}"), request_receiver)
    }

    fn hello_messages() -> Vec<ChatMessage> {
        vec![user_message("hello".to_string())]
    }

    /// Runs a streaming request and collects the pieces the callback received.
    async fn stream_at(
        provider: &PostProcessProvider,
        model: &str,
        reasoning: ReasoningConfig,
        base_url: &str,
    ) -> (Result<StreamedCompletion, String>, Vec<String>) {
        let mut pieces = Vec::new();
        let result = stream_chat_completion_with_messages_at_base_url(
            provider,
            "not-a-real-key".to_string(),
            model,
            hello_messages(),
            reasoning,
            base_url,
            &mut |text: &str| {
                pieces.push(text.to_string());
                true
            },
        )
        .await;
        (result, pieces)
    }

    /// Feeds `body` to the stream parser in pieces of `piece_len` bytes.
    fn parse_stream_body(
        body: &[u8],
        piece_len: usize,
        keep_going: impl Fn(usize) -> bool + Send + Sync,
    ) -> (Result<StreamedCompletion, String>, Vec<String>) {
        let mut pieces = Vec::new();
        let result = {
            let mut on_content = |text: &str| {
                pieces.push(text.to_string());
                keep_going(pieces.len())
            };
            let mut parser = CompletionStreamParser::default();
            let mut pushed = Ok(());
            for piece in body.chunks(piece_len) {
                pushed = parser.push(piece, &mut on_content);
                if pushed.is_err() || parser.is_finished() {
                    break;
                }
            }
            pushed.and_then(|()| parser.finish(&mut on_content))
        };
        (result, pieces)
    }

    fn test_provider(id: &str) -> PostProcessProvider {
        PostProcessProvider {
            id: id.to_string(),
            label: id.to_string(),
            base_url: "https://unused.example.com/v1".to_string(),
            allow_base_url_edit: false,
            allow_insecure_http: false,
            models_endpoint: Some("/models".to_string()),
        }
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

    #[tokio::test]
    async fn groq_qwen_off_request_reaches_mock_api_without_reasoning() {
        let response_body = r#"{"choices":[{"message":{"content":"mock answer"}}]}"#;
        let (base_url, captured_request) =
            serve_one_captured_json_response("200 OK", response_body).await;
        let provider = test_provider("groq");
        let reasoning = ReasoningConfig::from_user_toggle(false, 2048);

        let result = send_chat_completion_with_messages_at_base_url(
            &provider,
            "not-a-real-key".to_string(),
            "qwen/qwen3.6-27b",
            vec![ChatMessage {
                role: "user".to_string(),
                content: "hello".to_string(),
            }],
            reasoning,
            &base_url,
        )
        .await
        .unwrap();
        let request = captured_request.await.unwrap();

        assert_eq!(result.as_deref(), Some("mock answer"));
        assert_eq!(request.request_line, "POST /chat/completions HTTP/1.1");
        assert_eq!(request.body["model"], "qwen/qwen3.6-27b");
        assert_eq!(request.body["reasoning_effort"], "none");
        assert_eq!(request.body["stream"], false);
        assert!(request.body.get("reasoning").is_none());
        assert!(request.body.get("max_tokens").is_none());
        assert!(request.body.get("max_completion_tokens").is_none());
    }

    #[tokio::test]
    async fn groq_qwen_off_400_does_not_retry_without_none() {
        let (base_url, captured_request) =
            serve_one_captured_json_response("400 Bad Request", r#"{"error":"rejected"}"#).await;
        let provider = test_provider("groq");

        let error = send_chat_completion_with_messages_at_base_url(
            &provider,
            "not-a-real-key".to_string(),
            "qwen/qwen3.6-27b",
            vec![ChatMessage {
                role: "user".to_string(),
                content: "hello".to_string(),
            }],
            ReasoningConfig::from_user_toggle(false, 2048),
            &base_url,
        )
        .await
        .unwrap_err();
        let request = captured_request.await.unwrap();

        assert_eq!(request.body["reasoning_effort"], "none");
        assert!(error.contains("API request failed with status 400"));
        assert!(!error.contains("HTTP retry failed"));
    }

    #[test]
    fn requests_explicitly_disable_streaming() {
        let json = request_json();
        assert_eq!(json["stream"], false);
    }

    #[test]
    fn stream_parser_reassembles_events_split_at_every_byte() {
        let body = concat!(
            ": OPENROUTER PROCESSING\r\n\r\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":null}}]}\r\n\r\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Привет\"}}]}\r\n\r\n",
            "event: message\r\nid: 7\r\nretry: 10\r\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\" мир 👋\"},\"finish_reason\":\"stop\"}]}\r\n\r\n",
            "data: {\"choices\":[],\"usage\":{\"total_tokens\":9}}\r\n\r\n",
            "data: [DONE]\r\n\r\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"after done\"}}]}\r\n\r\n",
        );

        for piece_len in [1, 2, 3, 7, body.len()] {
            let (result, pieces) = parse_stream_body(body.as_bytes(), piece_len, |_| true);
            let completion = result.unwrap();
            assert_eq!(completion.content.as_deref(), Some("Привет мир 👋"));
            assert_eq!(pieces, vec!["Привет", " мир 👋"]);
            assert!(completion.first_token_at.is_some());
            assert!(!completion.stopped);
        }
    }

    #[test]
    fn stream_parser_accepts_multiline_data_and_missing_blank_lines() {
        let body = concat!(
            "data: {\"choices\":[{\"delta\":\n",
            "data: {\"content\":\"one\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\" two\"}}]}\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\" three\"}}]}",
        );

        let (result, pieces) = parse_stream_body(body.as_bytes(), 5, |_| true);
        assert_eq!(result.unwrap().content.as_deref(), Some("one two three"));
        assert_eq!(pieces, vec!["one", " two", " three"]);
    }

    #[test]
    fn stream_parser_reports_mid_stream_error_after_partial_content() {
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
            "data: {\"error\":{\"message\":\"Provider overloaded\",\"code\":502},",
            "\"choices\":[{\"delta\":{\"content\":\"\"},\"finish_reason\":\"error\"}]}\n\n",
        );

        let (result, pieces) = parse_stream_body(body.as_bytes(), 16, |_| true);
        let error = result.unwrap_err();
        assert!(error.contains("Provider overloaded (502)"), "{error}");
        assert_eq!(pieces, vec!["Hello"]);

        let body = "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"error\"}]}\n\n";
        let (result, _) = parse_stream_body(body.as_bytes(), 16, |_| true);
        assert!(result.unwrap_err().contains("finished with an error"));
    }

    #[test]
    fn stream_parser_reads_regular_json_body_without_first_token_time() {
        let body = "\u{FEFF}\r\n  {\"choices\":[{\"message\":{\"content\":\"Whole answer\",\"reasoning\":\"hidden\"}}]}";

        let (result, pieces) = parse_stream_body(body.as_bytes(), 4, |_| true);
        let completion = result.unwrap();
        assert_eq!(completion.content.as_deref(), Some("Whole answer"));
        assert_eq!(pieces, vec!["Whole answer"]);
        assert!(completion.first_token_at.is_none());

        let body = r#"{"error":{"message":"Model not found"}}"#;
        let (result, _) = parse_stream_body(body.as_bytes(), 4, |_| true);
        assert!(result.unwrap_err().contains("Model not found"));
    }

    #[test]
    fn stream_parser_rejects_unusable_bodies_without_quoting_them() {
        for body in [
            "",
            " \r\n ",
            "<html>PRIVATE TRANSCRIPTION CONTENT</html>",
            "{\"choices\":[{\"message\":{\"content\":\"PRIVATE TRANSCRIPTION CONTENT\"",
            "data: {\"choices\":[{\"delta\":{\"content\":\"PRIVATE TRANSCRIPTION CONTENT\"\n\n",
        ] {
            let (result, pieces) = parse_stream_body(body.as_bytes(), 3, |_| true);
            let error = result.unwrap_err();
            assert!(!error.contains("PRIVATE"), "{error}");
            assert!(pieces.is_empty());
        }
    }

    #[test]
    fn stream_parser_keeps_reasoning_out_of_the_answer() {
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"reasoning\":\"Let me think\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"more\",\"content\":\"\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":[",
            "{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"secret\"}]},",
            "{\"type\":\"text\",\"text\":\"Answer\"}]}}]}\n\n",
            "data: {\"choices\":[{\"index\":1,\"delta\":{\"content\":\"other choice\"}}]}\n\n",
            "data: [DONE]\n\n",
        );

        let (result, pieces) = parse_stream_body(body.as_bytes(), 9, |_| true);
        assert_eq!(result.unwrap().content.as_deref(), Some("Answer"));
        assert_eq!(pieces, vec!["Answer"]);
    }

    #[test]
    fn stream_parser_stops_when_the_callback_declines() {
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"first\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\" second\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\" third\"}}]}\n\n",
        );

        let (result, pieces) = parse_stream_body(body.as_bytes(), body.len(), |count| count < 2);
        let completion = result.unwrap();
        assert!(completion.stopped);
        assert_eq!(pieces, vec!["first", " second"]);
        assert_eq!(completion.content.as_deref(), Some("first second"));
    }

    #[test]
    fn stream_parser_without_answer_text_returns_none() {
        let body = "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\ndata: [DONE]\n\n";
        let (result, pieces) = parse_stream_body(body.as_bytes(), 8, |_| true);
        let completion = result.unwrap();
        assert!(completion.content.is_none());
        assert!(completion.first_token_at.is_none());
        assert!(pieces.is_empty());
    }

    #[tokio::test]
    async fn streaming_request_asks_for_a_stream_and_delivers_pieces() {
        let (base_url, mut requests) = serve_captured_responses(vec![mock_event_stream(&[
            r#"{"choices":[{"delta":{"content":"Hello"}}]}"#,
            r#"{"choices":[{"delta":{"content":", world"},"finish_reason":"stop"}]}"#,
            "[DONE]",
        ])])
        .await;

        let (result, pieces) = stream_at(
            &test_provider("openrouter"),
            "openai/gpt-oss-20b",
            ReasoningConfig::from_user_toggle(false, 2048),
            &base_url,
        )
        .await;
        let completion = result.unwrap();
        let request = requests.recv().await.unwrap();

        assert_eq!(completion.content.as_deref(), Some("Hello, world"));
        assert_eq!(pieces, vec!["Hello", ", world"]);
        assert!(completion.first_token_at.is_some());
        assert!(completion.streamed);
        assert_eq!(request.request_line, "POST /chat/completions HTTP/1.1");
        assert_eq!(request.body["stream"], true);
        assert_eq!(request.body["model"], "openai/gpt-oss-20b");
        assert_eq!(request.body["reasoning"]["effort"], "none");
    }

    #[tokio::test]
    async fn streaming_retries_without_reasoning_after_400() {
        let (base_url, mut requests) = serve_captured_responses(vec![
            mock_response("400 Bad Request", "application/json", r#"{"error":"reasoning"}"#),
            mock_event_stream(&[r#"{"choices":[{"delta":{"content":"ok"}}]}"#, "[DONE]"]),
        ])
        .await;

        let (result, pieces) = stream_at(
            &test_provider("openrouter"),
            "some/model",
            ReasoningConfig::from_user_toggle(false, 2048),
            &base_url,
        )
        .await;
        let first = requests.recv().await.unwrap();
        let retry = requests.recv().await.unwrap();

        assert_eq!(result.unwrap().content.as_deref(), Some("ok"));
        assert_eq!(pieces, vec!["ok"]);
        assert!(first.body.get("reasoning").is_some());
        assert_eq!(retry.body["stream"], true);
        assert!(retry.body.get("reasoning").is_none());
    }

    #[tokio::test]
    async fn rejected_streaming_falls_back_to_a_regular_request_and_is_remembered() {
        let answer = r#"{"choices":[{"message":{"content":"whole answer"}}]}"#;
        let (base_url, mut requests) = serve_captured_responses(vec![
            mock_response(
                "400 Bad Request",
                "application/json",
                r#"{"error":"stream not supported"}"#,
            ),
            mock_response("200 OK", "application/json", answer),
            mock_response("200 OK", "application/json", answer),
        ])
        .await;
        let provider = test_provider("openai");
        let reasoning = || ReasoningConfig::from_user_toggle(false, 2048);

        let (result, pieces) = stream_at(&provider, "no-stream-model", reasoning(), &base_url).await;
        let completion = result.unwrap();
        assert_eq!(completion.content.as_deref(), Some("whole answer"));
        assert_eq!(pieces, vec!["whole answer"]);
        assert!(completion.first_token_at.is_none());
        assert!(!completion.streamed);
        assert_eq!(requests.recv().await.unwrap().body["stream"], true);
        assert_eq!(requests.recv().await.unwrap().body["stream"], false);

        let (result, _) = stream_at(&provider, "no-stream-model", reasoning(), &base_url).await;
        assert_eq!(result.unwrap().content.as_deref(), Some("whole answer"));
        assert_eq!(requests.recv().await.unwrap().body["stream"], false);
        assert!(requests.try_recv().is_err());
    }

    #[tokio::test]
    async fn json_answer_to_a_streaming_request_has_no_first_token_time() {
        let (base_url, mut requests) = serve_captured_responses(vec![mock_response(
            "200 OK",
            "application/json",
            r#"{"choices":[{"message":{"content":"at once"}}]}"#,
        )])
        .await;

        let (result, pieces) = stream_at(
            &test_provider("openai"),
            "json-model",
            ReasoningConfig::from_user_toggle(false, 2048),
            &base_url,
        )
        .await;
        let completion = result.unwrap();

        assert_eq!(completion.content.as_deref(), Some("at once"));
        assert_eq!(pieces, vec!["at once"]);
        assert!(completion.first_token_at.is_none());
        assert!(!completion.streamed);
        assert_eq!(requests.recv().await.unwrap().body["stream"], true);
    }

    #[tokio::test]
    async fn auth_errors_do_not_fall_back_to_a_regular_request() {
        let (base_url, mut requests) = serve_captured_responses(vec![mock_response(
            "401 Unauthorized",
            "application/json",
            r#"{"error":"bad key"}"#,
        )])
        .await;

        let (result, pieces) = stream_at(
            &test_provider("openai"),
            "auth-model",
            ReasoningConfig::from_user_toggle(false, 2048),
            &base_url,
        )
        .await;

        assert!(result.unwrap_err().contains("status 401"));
        assert!(pieces.is_empty());
        assert_eq!(requests.recv().await.unwrap().body["stream"], true);
        assert!(requests.recv().await.is_none());
    }

    #[tokio::test]
    async fn groq_qwen_off_stream_fallback_never_drops_none() {
        let (base_url, mut requests) = serve_captured_responses(vec![
            mock_response("400 Bad Request", "application/json", r#"{"error":"no"}"#),
            mock_response("400 Bad Request", "application/json", r#"{"error":"no"}"#),
        ])
        .await;

        let (result, _) = stream_at(
            &test_provider("groq"),
            "qwen/qwen3.6-27b",
            ReasoningConfig::from_user_toggle(false, 2048),
            &base_url,
        )
        .await;
        let streamed = requests.recv().await.unwrap();
        let regular = requests.recv().await.unwrap();

        assert!(result.unwrap_err().contains("status 400"));
        assert_eq!(streamed.body["stream"], true);
        assert_eq!(streamed.body["reasoning_effort"], "none");
        assert_eq!(regular.body["stream"], false);
        assert_eq!(regular.body["reasoning_effort"], "none");
        assert!(requests.recv().await.is_none());
    }

    #[tokio::test]
    async fn stopping_the_stream_returns_the_text_so_far() {
        let (base_url, _requests) = serve_captured_responses(vec![mock_event_stream(&[
            r#"{"choices":[{"delta":{"content":"keep"}}]}"#,
            r#"{"choices":[{"delta":{"content":" drop"}}]}"#,
            "[DONE]",
        ])])
        .await;

        let mut pieces = Vec::new();
        let completion = stream_chat_completion_with_messages_at_base_url(
            &test_provider("openai"),
            "not-a-real-key".to_string(),
            "stop-model",
            hello_messages(),
            ReasoningConfig::from_user_toggle(false, 2048),
            &base_url,
            &mut |text: &str| {
                pieces.push(text.to_string());
                false
            },
        )
        .await
        .unwrap();

        assert!(completion.stopped);
        assert_eq!(pieces, vec!["keep"]);
        assert_eq!(completion.content.as_deref(), Some("keep"));
    }

    #[test]
    fn groq_qwen_disables_reasoning_with_documented_effort() {
        let reasoning = ReasoningConfig::from_user_toggle(false, 2048);
        let options = reasoning_request_options("groq", "qwen/qwen3.6-27b", &reasoning);
        let json = request_json_with_reasoning_options(options);

        assert_eq!(json["reasoning_effort"], "none");
        assert!(json.get("reasoning").is_none());
        assert!(json.get("max_tokens").is_none());
        assert!(json.get("max_completion_tokens").is_none());
    }

    #[test]
    fn groq_non_qwen_omits_unsupported_none_effort() {
        let reasoning = ReasoningConfig::from_user_toggle(false, 2048);
        let options = reasoning_request_options("groq", "openai/gpt-oss-120b", &reasoning);
        let json = request_json_with_reasoning_options(options);

        assert!(json.get("reasoning_effort").is_none());
        assert!(json.get("reasoning").is_none());
    }

    #[test]
    fn groq_qwen_enables_reasoning_with_documented_effort() {
        let reasoning = ReasoningConfig::new(true, 1024);
        let options = reasoning_request_options("groq", "qwen/qwen3.6-27b", &reasoning);
        let json = request_json_with_reasoning_options(options);

        assert_eq!(json["reasoning_effort"], "default");
        assert_eq!(json["max_completion_tokens"], 4000);
        assert!(json.get("max_tokens").is_none());
        assert!(json.get("reasoning").is_none());
    }

    #[test]
    fn groq_gpt_oss_uses_documented_reasoning_effort() {
        let reasoning = ReasoningConfig::new(true, 1024);
        let options = reasoning_request_options("groq", "openai/gpt-oss-120b", &reasoning);
        let json = request_json_with_reasoning_options(options);

        assert_eq!(json["reasoning_effort"], "medium");
        assert_eq!(json["max_completion_tokens"], 4000);
        assert!(json.get("max_tokens").is_none());
        assert!(json.get("reasoning").is_none());
    }

    #[test]
    fn undocumented_groq_qwen_variant_omits_reasoning_controls() {
        let reasoning = ReasoningConfig::from_user_toggle(false, 2048);
        let options = reasoning_request_options("groq", "qwen/qwen3-future", &reasoning);
        let json = request_json_with_reasoning_options(options);

        assert!(json.get("reasoning_effort").is_none());
        assert!(json.get("reasoning").is_none());
    }
}

#[cfg(test)]
#[path = "llm_client_openrouter_tests.rs"]
mod openrouter_paid_tests;
