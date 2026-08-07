use crate::util::config;
use crate::util::format;
use crate::util::llm::LlmConfig;
use anyhow::Result;

pub fn run(url: Option<String>, model: Option<String>) -> Result<()> {
    if url.is_none() && model.is_none() {
        let effective = LlmConfig::resolve();
        format::heading("Configuration");
        format::row("LLM URL", &effective.url);
        format::row("LLM model", &effective.model);
        format::info("");
        format::info("  set with:  rooter config --url <url> --model <model>");
        format::info("  or env vars: ROOTER_LLM_URL, ROOTER_LLM_MODEL");
        return Ok(());
    }

    let mut stored = config::load();
    if let Some(u) = url {
        stored.llm_url = Some(u);
    }
    if let Some(m) = model {
        stored.llm_model = Some(m);
    }
    config::save(&stored)?;
    format::ok("saved");
    Ok(())
}
