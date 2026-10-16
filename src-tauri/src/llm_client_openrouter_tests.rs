//! Paid requests to OpenRouter with the key saved in the app. Ignored by default:
//! `./test-local.ps1 -LibOnly -Filter openrouter_paid,--ignored,--nocapture`
use super::*;
use crate::llm_live_output::LlmLiveOutputProcessor;
use crate::settings::{get_default_settings, AppSettings, TextReplacement};
use std::time::{Duration, Instant};

const MODELS: &[&str] = &[
    "openai/gpt-oss-120b",
    "meta-llama/llama-3.1-8b-instruct",
    "qwen/qwen3.8-27b",
    "openai/gpt-oss-20b",
    "nvidia/nemotron-3.5-super-vl:free",
];

const PROMPT: &str = "Reply with exactly this text and nothing else: \
The quick brown fox jumps over the lazy dog near the river bank, \
and then the fox rests under an old oak tree until the evening.";

fn key_and_provider() -> (String, PostProcessProvider) {
    let key = crate::secure_keys::get_post_process_api_key("openrouter");
    assert!(!key.trim().is_empty(), "no saved OpenRouter key");
    let provider = get_default_settings()
        .post_process_provider("openrouter")
        .expect("OpenRouter provider")
        .clone();
    (key, provider)
}

fn live_settings() -> AppSettings {
    let mut settings = get_default_settings();
    settings.text_replacements_enabled = true;
    settings.text_replacements_before_llm = false;
    settings.text_replacements = vec![TextReplacement {
        id: "tr_paid".into(),
        from: "fox".into(),
        to: "cat".into(),
        enabled: true,
        case_sensitive: true,
        is_regex: false,
    }];
    settings
}

fn ms(duration: Duration) -> u128 {
    duration.as_millis()
}

async fn check_model(key: &str, provider: &PostProcessProvider, model: &str) -> Result<String, String> {
    let reasoning = || ReasoningConfig::from_user_toggle(false, 0);
    let settings = live_settings();

    // Streamed answer through the live insertion processor, as the app inserts it.
    let mut live = LlmLiveOutputProcessor::from_settings(&settings);
    let mut inserted = String::new();
    let mut pieces = 0usize;
    let started = Instant::now();
    let streamed = stream_chat_completion_with_reasoning(
        provider,
        key.to_string(),
        model,
        PROMPT.to_string(),
        reasoning(),
        &mut |piece| {
            pieces += 1;
            inserted.push_str(&live.push(piece));
            true
        },
    )
    .await?;
    let total = started.elapsed();
    inserted.push_str(&live.finish());
    let content = streamed.content.clone().unwrap_or_default();
    if content.trim().is_empty() {
        return Err("empty streamed answer".into());
    }
    if live.raw_text() != content {
        return Err("pieces do not add up to the answer".into());
    }
    let mut whole = LlmLiveOutputProcessor::from_settings(&settings);
    let mut expected = whole.push(&content);
    expected.push_str(&whole.finish());
    if inserted != expected {
        return Err(format!("live insertion differs:\n  live: {inserted:?}\n  whole: {expected:?}"));
    }
    if content.contains("fox") && inserted.contains("fox") {
        return Err("replacement was not applied while streaming".into());
    }
    let ttft = streamed
        .first_token_at
        .map(|at| format!("{} ms", ms(at - started)))
        .unwrap_or_else(|| "—".into());

    // Stopping after the first piece ends the request early.
    let mut stop_pieces = 0usize;
    let stopped = stream_chat_completion_with_reasoning(
        provider,
        key.to_string(),
        model,
        PROMPT.to_string(),
        reasoning(),
        &mut |_| {
            stop_pieces += 1;
            false
        },
    )
    .await?;
    if streamed.streamed && (!stopped.stopped || stop_pieces != 1) {
        return Err(format!("stop ignored: stopped={} pieces={stop_pieces}", stopped.stopped));
    }

    // Regular request for comparison.
    let regular_started = Instant::now();
    let regular =
        send_chat_completion_with_reasoning(provider, key.to_string(), model, PROMPT.to_string(), reasoning())
            .await?
            .unwrap_or_default();
    if regular.trim().is_empty() {
        return Err("empty regular answer".into());
    }

    Ok(format!(
        "streamed={} pieces={pieces} ttft={ttft} total={} ms | regular={} ms | stop ok | {:?}",
        streamed.streamed,
        ms(total),
        ms(regular_started.elapsed()),
        inserted.trim()
    ))
}

#[tokio::test]
#[ignore = "paid OpenRouter requests with the saved key"]
async fn openrouter_paid_streaming_models() {
    let (key, provider) = key_and_provider();
    let mut failures = Vec::new();
    for model in MODELS {
        match check_model(&key, &provider, model).await {
            Ok(report) => println!("OK   {model}: {report}"),
            Err(error) => {
                println!("FAIL {model}: {error}");
                failures.push(*model);
            }
        }
    }
    assert!(failures.is_empty(), "failed models: {failures:?}");
}
