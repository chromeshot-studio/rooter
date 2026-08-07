use crate::util::format;
use crate::util::llm::{self, LlmConfig};
use anyhow::Result;
use std::io::Write;

const SYSTEM_PROMPT: &str = "You are a terse, practical assistant embedded in `rooter`, a developer \
command-line tool. Answer plainly for a terminal - no markdown headers, minimal formatting, get to \
the point. Assume the person asking is a software developer working in a terminal right now.";

pub fn run(question: &str) -> Result<()> {
    if question.trim().is_empty() {
        format::error("ask something, e.g. `rooter ask \"why would ECONNREFUSED happen on localhost\"`");
        return Ok(());
    }

    let cfg = LlmConfig::resolve();
    if !llm::is_reachable(&cfg) {
        format::warn(format!("no local LLM reachable at {}", cfg.url));
        format::bullet("is it running? for Ollama: `ollama serve`");
        format::bullet("wrong endpoint? `rooter config --url <url> --model <model>`");
        return Ok(());
    }

    format::heading("rooter");
    let stdout = std::io::stdout();
    let result = llm::chat_stream(&cfg, SYSTEM_PROMPT, question, |token| {
        print!("{token}");
        let _ = stdout.lock().flush();
    });
    println!();

    match result {
        Ok(text) if text.trim().is_empty() => format::warn("the model returned an empty response"),
        Ok(_) => {}
        Err(e) => format::error(e),
    }

    Ok(())
}
