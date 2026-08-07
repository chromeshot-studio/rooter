use crate::util::config;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::io::BufRead;
use std::time::Duration;

const DEFAULT_URL: &str = "http://localhost:11434/v1";
const DEFAULT_MODEL: &str = "llama3.2";

/// Talks to any OpenAI-compatible `/v1/chat/completions` endpoint - Ollama,
/// LM Studio, llama.cpp's server, etc. all speak this dialect, so one client
/// covers the common local-LLM setups without picking a single vendor.
pub struct LlmConfig {
    pub url: String,
    pub model: String,
}

impl LlmConfig {
    pub fn resolve() -> Self {
        let stored = config::load();
        let url = std::env::var("ROOTER_LLM_URL")
            .ok()
            .or(stored.llm_url)
            .unwrap_or_else(|| DEFAULT_URL.to_string());
        let model = std::env::var("ROOTER_LLM_MODEL")
            .ok()
            .or(stored.llm_model)
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());
        LlmConfig { url, model }
    }

    fn base(&self) -> &str {
        self.url.trim_end_matches('/')
    }
}

/// Quick reachability probe with a short timeout, so callers can fail fast
/// with a friendly message instead of waiting on a hung connection.
pub fn is_reachable(cfg: &LlmConfig) -> bool {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(800))
        .timeout(Duration::from_secs(2))
        .build();
    agent.get(&format!("{}/models", cfg.base())).call().is_ok()
}

/// Streams a chat completion, invoking `on_token` with each chunk of text as
/// it arrives, and returns the full accumulated text.
pub fn chat_stream(cfg: &LlmConfig, system: &str, user: &str, mut on_token: impl FnMut(&str)) -> Result<String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(3))
        .timeout(Duration::from_secs(180))
        .build();

    let url = format!("{}/chat/completions", cfg.base());
    let body = json!({
        "model": cfg.model,
        "stream": true,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ],
    });

    let resp = agent
        .post(&url)
        .send_json(body)
        .map_err(|e| friendly_error(cfg, &e))?;

    let reader = std::io::BufReader::new(resp.into_reader());
    let mut full = String::new();

    for line in reader.lines() {
        let line = line.context("reading streamed response from the LLM")?;
        let Some(data) = line.strip_prefix("data: ") else { continue };
        if data == "[DONE]" {
            break;
        }
        let Ok(value) = serde_json::from_str::<Value>(data) else { continue };
        if let Some(token) = value["choices"][0]["delta"]["content"].as_str() {
            on_token(token);
            full.push_str(token);
        }
    }

    Ok(full)
}

/// Same as [`chat_stream`] but discards the incremental callback and just
/// returns the final text - for when you need one clean string (e.g. a
/// commit message) rather than a live typewriter effect.
pub fn chat(cfg: &LlmConfig, system: &str, user: &str) -> Result<String> {
    chat_stream(cfg, system, user, |_| {})
}

fn friendly_error(cfg: &LlmConfig, e: &ureq::Error) -> anyhow::Error {
    anyhow::anyhow!(
        "couldn't reach a local LLM at {} ({e})\n  is it running? for Ollama: `ollama serve`\n  wrong endpoint? `rooter config --url <url> --model <model>`",
        cfg.url
    )
}
