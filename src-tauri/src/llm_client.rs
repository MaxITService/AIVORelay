use crate::settings::PostProcessProvider;
use log::{debug, info, warn};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, REFERER, USER_AGENT};
use serde::{Deserialize, Serialize};

/// Configuration for Extended Thinking / Reasoning (OpenRouter)
#[derive(Debug, Clone, Default)]
pub struct ReasoningConfig {
    pub enabled: bool,
    pub budget: u32, // min 1024 for OpenRouter/Anthropic
}

impl ReasoningConfig {
    pub fn new(enabled: bool, budget: u32) -> Self {
        Self {
            enabled,
            budget: if enabled { budget.max(1024) } else { budget },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// Reasoning object for OpenRouter API
#[derive(Debug, Serialize)]
struct ReasoningParams {
    max_tokens: u32,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
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
        .map_err(|e| format!("Failed to build HTTP client: {}", e))
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
    let base_url = provider.base_url.trim_end_matches('/');
    let url = format!("{}/chat/completions", base_url);

    debug!("Sending chat completion request to: {}", url);

    let client = create_client(provider, &api_key)?;

    // Calculate max_tokens: if reasoning is enabled, ensure enough room for answer
    // Formula: max(4000, reasoning_budget + 2000)
    let (max_tokens, reasoning_params) = if reasoning.enabled {
        let budget = reasoning.budget.max(1024);
        let total = (budget + 2000).max(4000);
        debug!(
            "Extended Thinking enabled: reasoning_budget={}, max_tokens={}",
            budget, total
        );
        (Some(total), Some(ReasoningParams { max_tokens: budget }))
    } else {
        (None, None)
    };

    let request_body = ChatCompletionRequest {
        model: model.to_string(),
        messages: messages.clone(),
        max_tokens,
        reasoning: reasoning_params,
    };

    let response = client
        .post(&url)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    let status = response.status();

    // Fail-soft retry: if we get 400 and reasoning was enabled, retry without reasoning
    if status.as_u16() == 400 && reasoning.enabled {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read error response".to_string());

        warn!(
            "Extended Thinking request failed with 400, retrying without reasoning: {}",
            error_text
        );

        // Retry without reasoning
        let fallback_request = ChatCompletionRequest {
            model: model.to_string(),
            messages,
            max_tokens: None,
            reasoning: None,
        };

        let fallback_response = client
            .post(&url)
            .json(&fallback_request)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed (fallback): {}", e))?;

        let fallback_status = fallback_response.status();
        if !fallback_status.is_success() {
            let fallback_error = fallback_response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error response".to_string());
            return Err(format!(
                "API request failed with status {}: {}",
                fallback_status, fallback_error
            ));
        }

        let completion: ChatCompletionResponse = fallback_response
            .json()
            .await
            .map_err(|e| format!("Failed to parse API response: {}", e))?;

        return Ok(completion
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone()));
    }

    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read error response".to_string());
        return Err(format!(
            "API request failed with status {}: {}",
            status, error_text
        ));
    }

    let completion: ChatCompletionResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse API response: {}", e))?;

    // Log reasoning tokens if present (but don't include in response)
    if let Some(choice) = completion.choices.first() {
        if let Some(ref reasoning_text) = choice.message.reasoning {
            let reasoning_preview = if reasoning_text.len() > 200 {
                format!(
                    "{}... ({} chars total)",
                    &reasoning_text[..200],
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
    let base_url = provider.base_url.trim_end_matches('/');
    let url = format!("{}/models", base_url);

    debug!("Fetching models from: {}", url);

    let client = create_client(provider, &api_key)?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch models: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!(
            "Model list request failed ({}): {}",
            status, error_text
        ));
    }

    let parsed: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

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
