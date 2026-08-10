//! Optional LLM cleanup shared by interactive TTS and TTS file conversion.
//!
//! This module deliberately reuses the application's provider-neutral LLM
//! client and secure provider credentials. It has no knowledge of WebViews or
//! overlays; callers decide how progress is presented for their operation kind.

use crate::llm_client::{self, ReasoningConfig};
use crate::managers::provider_error::safe_text;
use crate::managers::tts::{
    TtsBoundary, TtsChunk, MAX_TTS_PROCESSED_TEXT_BYTES,
};
use crate::settings::{
    AppSettings, LLMPrompt, PostProcessProvider, TtsKeySource, TtsLlmScope,
    APPLE_INTELLIGENCE_PROVIDER_ID,
};
use anyhow::{anyhow, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::time::Duration;

const LLM_CHUNKING_REVISION: &str = "packed-semantic-v1";
const MAX_LLM_CHUNK_CHARS: usize = 50_000;

#[derive(Clone)]
pub struct ResolvedTtsLlmConfig {
    pub provider: PostProcessProvider,
    pub api_key: String,
    pub model: String,
    pub prompt_id: String,
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
        prompt_id: prompt.id.trim().to_string(),
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
    on_progress: impl FnMut(TtsLlmProgress),
) -> Result<String> {
    if text.trim().is_empty() {
        return Err(anyhow!("There is no text for TTS AI cleanup"));
    }
    let chunks = input_chunks(text, config.chunk_target_chars);
    preprocess_chunks(
        &chunks,
        config,
        Vec::new(),
        on_progress,
        |_, _| Ok(()),
    )
    .await
}

pub async fn preprocess_chunks(
    chunks: &[TtsChunk],
    config: &ResolvedTtsLlmConfig,
    resumed_chunks: Vec<String>,
    mut on_progress: impl FnMut(TtsLlmProgress),
    mut on_chunk_completed: impl FnMut(&TtsChunk, &str) -> Result<()>,
) -> Result<String> {
    if chunks.is_empty() {
        return Err(anyhow!("There is no text for TTS AI cleanup"));
    }
    if resumed_chunks.len() > chunks.len() {
        return Err(anyhow!(
            "The saved TTS AI cleanup prefix has more parts than the current document"
        ));
    }

    let total_chunks = chunks.len();
    let total_input_bytes = chunks.iter().try_fold(0usize, |total, chunk| {
        total
            .checked_add(chunk.text.len())
            .ok_or_else(|| anyhow!("TTS AI cleanup input size overflowed"))
    })?;
    let max_total_output_bytes = total_input_bytes
        .saturating_mul(4)
        .max(total_input_bytes.saturating_add(4_096))
        .min(MAX_TTS_PROCESSED_TEXT_BYTES);
    let mut cleaned_chunks = Vec::with_capacity(total_chunks);
    let mut total_output_bytes = 0usize;
    for (offset, cleaned) in resumed_chunks.into_iter().enumerate() {
        let chunk = &chunks[offset];
        validate_cleaned_chunk_output(
            &chunk.text,
            &cleaned,
            chunk.index,
            total_chunks,
        )?;
        total_output_bytes = checked_total_output_bytes(
            total_output_bytes,
            &cleaned,
            !cleaned_chunks.is_empty(),
            max_total_output_bytes,
        )?;
        cleaned_chunks.push(cleaned);
    }
    if let Some(last) = cleaned_chunks.len().checked_sub(1).map(|index| &chunks[index]) {
        on_progress(TtsLlmProgress {
            completed_chunks: cleaned_chunks.len(),
            total_chunks,
            current_chunk: last.index,
            attempt: 0,
            retrying: false,
            message: format!(
                "Recovered {}/{} verified AI text cleanup parts",
                cleaned_chunks.len(),
                total_chunks
            ),
        });
    }

    for chunk in chunks.iter().skip(cleaned_chunks.len()) {
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

        total_output_bytes = checked_total_output_bytes(
            total_output_bytes,
            &cleaned,
            !cleaned_chunks.is_empty(),
            max_total_output_bytes,
        )?;
        on_chunk_completed(chunk, &cleaned)?;
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

pub(crate) fn input_chunks(text: &str, target_chars: usize) -> Vec<TtsChunk> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    let chars: Vec<char> = text.chars().collect();
    let target = target_chars.clamp(1_000, MAX_LLM_CHUNK_CHARS);
    let mut chunks = Vec::new();
    let mut start = 0usize;

    while start < chars.len() {
        let upper = start.saturating_add(target).min(chars.len());
        let (end, boundary_after) = if upper == chars.len() {
            (upper, TtsBoundary::End)
        } else {
            choose_llm_boundary(&chars, start, upper)
        };
        let end = end
            .max(start.saturating_add(1))
            .min(start.saturating_add(MAX_LLM_CHUNK_CHARS).min(chars.len()));
        let chunk_text: String = chars[start..end].iter().collect();
        if !chunk_text.trim().is_empty() {
            chunks.push(TtsChunk {
                index: chunks.len() + 1,
                character_count: chunk_text.chars().count(),
                text: chunk_text,
                boundary_after,
            });
        }
        start = end;
    }
    chunks
}

fn choose_llm_boundary(chars: &[char], start: usize, upper: usize) -> (usize, TtsBoundary) {
    if let Some(end) = last_paragraph_boundary(chars, start, upper) {
        return (end, TtsBoundary::Paragraph);
    }
    if let Some(end) = last_llm_boundary(chars, start, upper, is_sentence_boundary) {
        return (end, TtsBoundary::Sentence);
    }
    if let Some(end) = last_llm_boundary(chars, start, upper, is_clause_boundary) {
        return (end, TtsBoundary::Clause);
    }
    if let Some(end) = last_llm_boundary(chars, start, upper, char::is_whitespace) {
        return (end, TtsBoundary::Whitespace);
    }
    (upper, TtsBoundary::Hard)
}

fn last_paragraph_boundary(chars: &[char], start: usize, upper: usize) -> Option<usize> {
    let mut previous_newline = None;
    let mut candidate = None;
    for index in start..upper {
        if chars[index] == '\n' {
            if let Some(previous) = previous_newline {
                if chars[previous + 1..index]
                    .iter()
                    .all(|character| character.is_whitespace())
                    && chars[start..=index]
                        .iter()
                        .any(|character| !character.is_whitespace())
                {
                    candidate = Some(index + 1);
                }
            }
            previous_newline = Some(index);
        } else if !chars[index].is_whitespace() {
            previous_newline = None;
        }
    }
    candidate
}

fn last_llm_boundary(
    chars: &[char],
    start: usize,
    upper: usize,
    predicate: impl Fn(char) -> bool,
) -> Option<usize> {
    (start..upper)
        .rev()
        .find(|&index| {
            predicate(chars[index])
                && chars[start..=index]
                    .iter()
                    .any(|character| !character.is_whitespace())
        })
        .map(|index| index + 1)
}

fn is_sentence_boundary(character: char) -> bool {
    matches!(character, '.' | '!' | '?' | '。' | '！' | '？' | '…' | '\n')
}

fn is_clause_boundary(character: char) -> bool {
    matches!(character, ',' | ';' | ':' | '，' | '、' | '；' | '：' | '—' | '–')
}

fn join_cleaned_chunks(chunks: &[String]) -> String {
    chunks.join("\n\n")
}

fn checked_total_output_bytes(
    current_bytes: usize,
    output: &str,
    has_previous: bool,
    maximum_bytes: usize,
) -> Result<usize> {
    let separator_bytes = if has_previous { 2 } else { 0 };
    let total = current_bytes
        .checked_add(separator_bytes)
        .and_then(|value| value.checked_add(output.len()))
        .ok_or_else(|| anyhow!("TTS AI cleanup output size overflowed"))?;
    if total > maximum_bytes {
        return Err(anyhow!(
            "TTS AI cleanup output exceeded its {maximum_bytes}-byte processed-text safety limit"
        ));
    }
    Ok(total)
}

fn validate_cleaned_chunk_output(
    input: &str,
    output: &str,
    chunk_index: usize,
    total_chunks: usize,
) -> Result<()> {
    if output.trim().is_empty() {
        return Err(anyhow!(
            "TTS AI cleanup returned empty text for part {chunk_index}/{total_chunks}"
        ));
    }
    let input_chars = input.chars().count();
    let output_chars = output.chars().count();
    let chunk_limit = input_chars
        .saturating_mul(4)
        .max(input_chars.saturating_add(4_096));
    if output_chars > chunk_limit {
        return Err(anyhow!(
            "TTS AI cleanup returned {output_chars} characters for a {input_chars}-character part; the {chunk_limit}-character safety limit was exceeded"
        ));
    }
    Ok(())
}

#[derive(Serialize)]
struct CleanupSignature<'a> {
    chunking_revision: &'static str,
    provider_id: &'a str,
    provider_base_url: &'a str,
    provider_allow_insecure_http: bool,
    model: &'a str,
    prompt_id: &'a str,
    instructions: &'a str,
    reasoning_enabled: bool,
    reasoning_budget: u32,
    reasoning_disable_by_default: bool,
    chunk_target_chars: usize,
    input_chunk_hashes: Vec<String>,
}

pub(crate) fn cleanup_fingerprint(
    config: &ResolvedTtsLlmConfig,
    chunks: &[TtsChunk],
) -> Result<String> {
    let signature = CleanupSignature {
        chunking_revision: LLM_CHUNKING_REVISION,
        provider_id: config.provider.id.trim(),
        provider_base_url: config.provider.base_url.trim().trim_end_matches('/'),
        provider_allow_insecure_http: config.provider.allow_insecure_http,
        model: config.model.trim(),
        prompt_id: config.prompt_id.trim(),
        instructions: config.instructions.trim(),
        reasoning_enabled: config.reasoning.enabled,
        reasoning_budget: config.reasoning.budget,
        reasoning_disable_by_default: config
            .reasoning
            .disable_by_default_on_compatible_providers,
        chunk_target_chars: config.chunk_target_chars,
        input_chunk_hashes: chunks
            .iter()
            .map(|chunk| input_chunk_sha256(&chunk.text))
            .collect(),
    };
    let bytes = serde_json::to_vec(&signature)
        .map_err(|error| anyhow!("Failed to fingerprint TTS AI cleanup: {error}"))?;
    Ok(sha256_hex(&bytes))
}

pub(crate) fn input_chunk_sha256(input: &str) -> String {
    sha256_hex(input.as_bytes())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
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
                validate_cleaned_chunk_output(input, &output, chunk_index, total_chunks)?;
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
        let chunks = input_chunks(&input, 1_000);
        assert!(chunks.len() > 1);
        assert!(chunks
            .iter()
            .all(|chunk| chunk.character_count <= MAX_LLM_CHUNK_CHARS));
        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<String>(),
            input
        );
    }

    #[test]
    fn cleanup_chunking_packs_multiple_paragraphs_toward_the_target() {
        let input = (0..30)
            .map(|index| format!("Paragraph {index}: {}\n\n", "word ".repeat(20)))
            .collect::<String>();
        let chunks = input_chunks(&input, 1_000);

        assert!(chunks.len() < 30);
        assert!(chunks.iter().all(|chunk| chunk.character_count <= 1_000));
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
