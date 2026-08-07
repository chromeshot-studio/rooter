use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct StoredConfig {
    pub llm_url: Option<String>,
    pub llm_model: Option<String>,
}

fn config_path() -> Result<PathBuf> {
    let base = dirs::config_dir().context("could not determine a config directory for this OS")?;
    let dir = base.join("rooter");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("config.json"))
}

pub fn load() -> StoredConfig {
    config_path()
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save(cfg: &StoredConfig) -> Result<()> {
    let path = config_path()?;
    let raw = serde_json::to_string_pretty(cfg)?;
    std::fs::write(&path, raw).with_context(|| format!("failed to write config to {}", path.display()))?;
    Ok(())
}
