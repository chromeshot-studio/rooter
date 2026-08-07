use crate::util::classify::StashKind;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StashEntry {
    pub id: String,
    pub kind: StashKind,
    pub content: String,
    pub created_at: String,
}

fn data_dir() -> Result<PathBuf> {
    let base = dirs::data_dir().context("could not determine a data directory for this OS")?;
    let dir = base.join("rooter");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn stash_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("stash.json"))
}

pub fn load() -> Result<Vec<StashEntry>> {
    let path = stash_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read stash file at {}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    let entries: Vec<StashEntry> = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse stash file at {}", path.display()))?;
    Ok(entries)
}

pub fn save(entries: &[StashEntry]) -> Result<()> {
    let path = stash_path()?;
    let raw = serde_json::to_string_pretty(entries)?;
    fs::write(&path, raw).with_context(|| format!("failed to write stash file at {}", path.display()))?;
    Ok(())
}

pub fn push(kind: StashKind, content: String) -> Result<StashEntry> {
    let mut entries = load()?;
    let entry = StashEntry {
        id: uuid::Uuid::new_v4().to_string(),
        kind,
        content,
        created_at: chrono::Local::now().to_rfc3339(),
    };
    entries.push(entry.clone());
    save(&entries)?;
    Ok(entry)
}
