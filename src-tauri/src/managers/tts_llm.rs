//! Optional LLM cleanup shared by interactive TTS and TTS file conversion.
//!
//! This module deliberately reuses the application's provider-neutral LLM
//! client and secure provider credentials. It has no knowledge of WebViews or
//! overlays; callers decide how progress is presented for their operation kind.

use crate::llm_client::{self, ReasoningConfig};
use crate::managers::provider_error::safe_text;
use crate::managers::tts::semantic_chunks;
use crate::settings::{
    AppSettings, LLMPrompt, PostProcessProvider, TtsKeySource, TtsLlmScope,
    APPLE_INTELLIGENCE_PROVIDER_ID,
};
use anyhow::{anyhow, Result};
use std::time::Duration;

const MAX_LLM_OUTPUT_CHARS: usize = 1_000_000;

#[derive(Clone)]
pub struct ResolvedTtsLlmConfig {
    pub provider: PostProcessProvider,
    pub api_key: String,
    pub model: String,
    pub instructions: String,
    pub reasoning: ReasoningConfig,
    pub chunk_target_chars: usize,
    pub retry_count: u8,
    pub retry_base_delay_ms: u32,
    pub request_timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtsLlmProgress {
    pub completed_chunks: usize,
    pub total_chunks: usize,
    pub current_chunk: usize,
    pub attempt: u8,
    pub retrying: bool,
    pub message: String,
}

pub fn resolve_config(
    settings: &AppSettings,
    scope: TtsLlmScope,
) -> Result<Option<ResolvedTtsLlmConfig>> {
    let llm = &settings.tts.llm_preprocessing;
    let enabled = match scope {
        TtsLlmScope::Interactive => llm.interactive_enabled,
        TtsLlmScope::File => llm.file_enabled,
    };
    if !enabled {
        return Ok(None);
    }

    let (provider, api_key) = resolve_provider_and_key(settings)?;

    let model = llm.model.trim().to_string();
    if model.is_empty() {
        return Err(anyhow!(
            "Choose a model for TTS AI text cleanup before enabling it"
        ));
    }

    let selected_prompt_id = match scope {
        TtsLlmScope::Interactive => llm.interactive_selected_prompt_id.trim(),
        TtsLlmScope::File => llm.file_selected_prompt_id.trim(),
    };
    let prompts = match scope {
        TtsLlmScope::Interactive => &llm.interactive_prompts,
        TtsLlmScope::File => &llm.file_prompts,
    };
    let prompt = selected_prompt(prompts, selected_prompt_id)?;

    Ok(Some(ResolvedTtsLlmConfig {
        provider,
        api_key,
        model,
        instructions: prompt.prompt.trim().to_string(),
        reasoning: ReasoningConfig::new(llm.reasoning_enabled, llm.reasoning_budget)
            .with_disable_by_default_on_compatible_providers(true),
        chunk_target_chars: (llm.chunk_target_chars as usize).clamp(1_000, 50_000),
        retry_count: llm.retry_count.min(10),
        retry_base_delay_ms: llm.retry_base_delay_ms.clamp(100, 30_000),
        request_timeout: Duration::from_secs(u64::from(llm.request_timeout_seconds.clamp(10, 600))),
    }))
}

pub fn resolve_provider_and_key(settings: &AppSettings) -> Result<(PostProcessProvider, String)> {
    let llm = &settings.tts.llm_preprocessing;
    let mut provider = settings
        .post_process_provider(llm.provider_id.trim())
        .cloned()
        .ok_or_else(|| {
            anyhow!(
                "TTS AI text cleanup provider '{}' is not available",
                llm.provider_id.trim()
            )
        })?;
    if provider.id == APPLE_INTELLIGENCE_PROVIDER_ID {
        return Err(anyhow!(
            "Apple Intelligence is not yet supported by TTS AI text cleanup"
        ));
    }
    if provider.id == "custom" {
        provider.base_url = llm.custom_base_url.trim().trim_end_matches('/').to_string();
        provider.allow_insecure_http = llm.custom_allow_insecure_http;
    }
    // Validate and canonicalize the URL before any paid/network request.
    crate::url_security::canonical_llm_provider_base_url(&provider)
        .map_err(|error| anyhow!(error))?;

    let api_key = match llm.key_source {
        TtsKeySource::Shared => crate::secure_keys::get_post_process_api_key(&provider.id),
        TtsKeySource::Separate => crate::secure_keys::get_tts_llm_api_key(&provider.id),
    };
    if provider.id != "custom" && api_key.trim().is_empty() {
        let source = match llm.key_source {
            TtsKeySource::Shared => "shared LLM Post Processing",
            TtsKeySource::Separate => "separate TTS AI cleanup",
        };
        return Err(anyhow!(
            "No {source} API key is configured for {}",
            provider.label
        ));
    }

    Ok((provider, api_key))
}

fn selected_prompt<'a>(prompts: &'a [LLMPrompt], selected_id: &str) -> Result<&'a LLMPrompt> {
    let prompt = prompts
        .iter()
        .find(|prompt| prompt.id == selected_id)
        .ok_or_else(|| anyhow!("The selected TTS AI cleanup prompt no longer exists"))?;
    if prompt.prompt.trim().is_empty() {
        return Err(anyhow!("The selected TTS AI cleanup prompt is empty"));
    }
    Ok(prompt)
}

pub async fn preprocess_text(
    text: &str,
    config: &ResolvedTtsLlmConfig,
    mut on_progress: impl FnMut(TtsLlmProgress),
) -> Result<String> {
    if text.trim().is_empty() {
        return Err(anyhow!("There is no text for TTS AI cleanup"));
    }
    let chunks = llm_input_chunks(text, config.chunk_target_chars);
    if chunks.is_empty() {
        return Err(anyhow!("There is no text for TTS AI cleanup"));
    }

    let total_chunks = chunks.len();
    let mut cleaned_chunks = Vec::with_capacity(total_chunks);
    let mut total_output_chars = 0usize;
    for chunk in &chunks {
        let max_attempts = config.retry_count.saturating_add(1);
        // Retries apply only to the current LLM cleanup chunk.
        let cleaned = preprocess_chunk_with_retry(
            chunk.index,
            total_chunks,
            &chunk.text,
            config,
            max_attempts,
            &mut on_progress,
        )
        .await?;

        let cleaned_chars = cleaned.chars().count();
        total_output_chars = total_output_chars.saturating_add(cleaned_chars);
        if total_output_chars > MAX_LLM_OUTPUT_CHARS {
            return Err(anyhow!(
                "TTS AI cleanup output exceeded the {} character safety limit",
                MAX_LLM_OUTPUT_CHARS
            ));
        }
        cleaned_chunks.push(cleaned);
        on_progress(TtsLlmProgress {
            completed_chunks: cleaned_chunks.len(),
            total_chunks,
            current_chunk: chunk.index,
            attempt: 1,
            retrying: false,
            message: format!(
                "AI text cleanup completed part {}/{}",
                chunk.index, total_chunks
            ),
        });
    }

    let output = join_cleaned_chunks(&cleaned_chunks);
    if output.trim().is_empty() {
        return Err(anyhow!("TTS AI cleanup returned no speakable text"));
    }
    Ok(output)
}

fn llm_input_chunks(text: &str, target_chars: usize) -> Vec<crate::managers::tts::TtsChunk> {
    semantic_chunks(text, target_chars.clamp(1_000, 50_000), 50_000)
}

fn join_cleaned_chunks(chunks: &[String]) -> String {
    chunks.join("\n\n")
}

async fn preprocess_chunk_with_retry(
    chunk_index: usize,
    total_chunks: usize,
    input: &str,
    config: &ResolvedTtsLlmConfig,
    max_attempts: u8,
    on_progress: &mut impl FnMut(TtsLlmProgress),
) -> Result<String> {
    let mut attempt = 1u8;
    loop {
        on_progress(TtsLlmProgress {
            completed_chunks: chunk_index.saturating_sub(1),
            total_chunks,
            current_chunk: chunk_index,
            attempt,
            retrying: attempt > 1,
            message: format!(
                "AI text cleanup part {chunk_index}/{total_chunks}, attempt {attempt}/{max_attempts}"
            ),
        });

        let request = llm_client::send_chat_completion_with_system_and_reasoning(
            &config.provider,
            config.api_key.clone(),
            &config.model,
            config.instructions.clone(),
            input.to_string(),
            config.reasoning.clone(),
        );
        let result = tokio::time::timeout(config.request_timeout, request).await;
        match result {
            Ok(Ok(Some(output))) => {
                let output = output.trim().to_string();
                if output.is_empty() {
                    return Err(anyhow!(
                        "TTS AI cleanup returned empty text for part {chunk_index}/{total_chunks}"
                    ));
                }
                let input_chars = input.chars().count();
                let output_chars = output.chars().count();
                let chunk_limit = input_chars
                    .saturating_mul(4)
                    .max(input_chars.saturating_add(4_096))
                    .min(MAX_LLM_OUTPUT_CHARS);
                if output_chars > chunk_limit {
                    return Err(anyhow!(
                        "TTS AI cleanup returned {output_chars} characters for a {input_chars}-character part; the {chunk_limit}-character safety limit was exceeded"
                    ));
                }
                return Ok(output);
            }
            Ok(Ok(None)) => {
                return Err(anyhow!(
                    "TTS AI cleanup provider returned no content for part {chunk_index}/{total_chunks}"
                ));
            }
            Ok(Err(error)) => {
                let error = safe_provider_error(&error, &config.api_key);
                if attempt >= max_attempts || !is_retryable_error(&error) {
                    return Err(anyhow!(
                        "TTS AI cleanup failed for part {chunk_index}/{total_chunks}: {error}"
                    ));
                }
                on_progress(TtsLlmProgress {
                    completed_chunks: chunk_index.saturating_sub(1),
                    total_chunks,
                    current_chunk: chunk_index,
                    attempt,
                    retrying: true,
                    message: format!(
                        "TTS AI cleanup retrying part {chunk_index}/{total_chunks}: {error}"
                    ),
                });
            }
            Err(_) => {
                if attempt >= max_attempts {
                    return Err(anyhow!(
                        "TTS AI cleanup timed out after {} seconds for part {chunk_index}/{total_chunks}",
                        config.request_timeout.as_secs()
                    ));
                }
            }
        }

        tokio::time::sleep(exponential_delay(config.retry_base_delay_ms, attempt)).await;
        attempt = attempt.saturating_add(1);
    }
}

pub(crate) fn safe_provider_error(error: &str, api_key: &str) -> String {
    if api_key.is_empty() {
        safe_text(error)
    } else {
        safe_text(&error.replace(api_key, "[redacted]"))
    }
}

fn is_retryable_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    if lower.contains("quota")
        || lower.contains("insufficient_quota")
        || lower.contains("billing")
        || lower.contains("credit")
        || lower.contains("status 400")
        || lower.contains("status 401")
        || lower.contains("status 403")
        || lower.contains("status 404")
    {
        return false;
    }
    lower.contains("status 408")
        || lower.contains("status 409")
        || lower.contains("status 425")
        || lower.contains("status 429")
        || lower.contains("status 500")
        || lower.contains("status 502")
        || lower.contains("status 503")
        || lower.contains("status 504")
        || lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("connection")
        || lower.contains("http request failed")
}

fn exponential_delay(base_delay_ms: u32, attempt: u8) -> Duration {
    let multiplier = 1u64
        .checked_shl(u32::from(attempt.saturating_sub(1)).min(10))
        .unwrap_or(u64::MAX);
    Duration::from_millis(
        u64::from(base_delay_ms)
            .saturating_mul(multiplier)
            .min(30_000),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_classification_does_not_retry_quota_or_auth_errors() {
        assert!(!is_retryable_error(
            "API request failed with status 429: insufficient_quota"
        ));
        assert!(!is_retryable_error(
            "API request failed with status 401: invalid key"
        ));
        assert!(is_retryable_error(
            "API request failed with status 503: overloaded"
        ));
        assert!(is_retryable_error("HTTP request failed: connection reset"));
    }

    #[test]
    fn provider_errors_redact_the_resolved_api_key() {
        let api_key = "sk-test-cleanup-secret";
        let safe = safe_provider_error(
            &format!("custom provider echoed Authorization: Bearer {api_key}"),
            api_key,
        );

        assert!(!safe.contains(api_key));
        assert!(safe.contains("[redacted]"));
    }

    #[test]
    fn exponential_delay_is_bounded() {
        assert_eq!(exponential_delay(750, 1), Duration::from_millis(750));
        assert_eq!(exponential_delay(750, 2), Duration::from_millis(1_500));
        assert_eq!(exponential_delay(30_000, 10), Duration::from_secs(30));
    }

    #[test]
    fn long_unicode_input_chunks_preserve_every_character_in_order() {
        let paragraph = "Глава 1. Первое предложение не должно разрываться. 第二句也必须保留。\n\n";
        let input = paragraph.repeat(90);
        let chunks = llm_input_chunks(&input, 1_000);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|chunk| chunk.character_count <= 50_000));
        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<String>(),
            input
        );
    }

    #[test]
    fn cleaned_chunk_recombination_is_ordered_and_readable() {
        let chunks = vec![
            "First cleaned paragraph.".to_string(),
            "Second cleaned paragraph.".to_string(),
            "Третий очищенный абзац.".to_string(),
        ];
        assert_eq!(
            join_cleaned_chunks(&chunks),
            "First cleaned paragraph.\n\nSecond cleaned paragraph.\n\nТретий очищенный абзац."
        );
    }
}
