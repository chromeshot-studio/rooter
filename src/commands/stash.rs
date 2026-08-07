use crate::cli::StashCommand;
use crate::util::classify::{classify_content, StashKind};
use crate::util::format;
use crate::util::storage::{self, StashEntry};
use anyhow::{Context, Result};
use arboard::Clipboard;
use colored::Colorize;

pub fn run(action: Option<StashCommand>) -> Result<()> {
    match action {
        None => capture(None),
        Some(StashCommand::Code) => capture(Some(StashKind::Code)),
        Some(StashCommand::Url) => capture(Some(StashKind::Url)),
        Some(StashCommand::Json) => capture(Some(StashKind::Json)),
        Some(StashCommand::Password) => capture(Some(StashKind::Password)),
        Some(StashCommand::List) => list(),
        Some(StashCommand::Pop { index, copy, keep }) => pop(index, copy, keep),
        Some(StashCommand::Grep { term }) => grep(&term),
        Some(StashCommand::Clear) => clear(),
    }
}

fn read_clipboard() -> Result<String> {
    let mut clipboard = Clipboard::new().context("couldn't access the system clipboard")?;
    let text = clipboard.get_text().context("clipboard doesn't contain text")?;
    if text.trim().is_empty() {
        anyhow::bail!("clipboard is empty");
    }
    Ok(text)
}

pub fn capture(forced_kind: Option<StashKind>) -> Result<()> {
    let content = read_clipboard()?;
    let kind = forced_kind.unwrap_or_else(|| classify_content(&content));
    let entry = storage::push(kind, content)?;

    format::heading("Stashed");
    format::row("Type", kind.label());
    format::row("Preview", preview(&entry.content, kind));
    format::info(format!("  {}", format!("id: {}", short_id(&entry.id)).dimmed()));
    Ok(())
}

fn list() -> Result<()> {
    let entries = storage::load()?;
    if entries.is_empty() {
        format::info("Stash is empty. Copy something and run `rooter stash`.");
        return Ok(());
    }

    format::heading(&format!("Stash ({} items)", entries.len()));
    for (i, entry) in entries.iter().enumerate().rev() {
        println!(
            "  {:>3}  {:<9} {}",
            i.to_string().dimmed(),
            entry.kind.label(),
            preview(&entry.content, entry.kind)
        );
    }
    format::info(format!("  {}", "tip: `rooter stash pop [index]` or `rooter stash grep <term>`".dimmed()));
    Ok(())
}

fn pop(index: Option<usize>, copy: bool, keep: bool) -> Result<()> {
    let mut entries = storage::load()?;
    if entries.is_empty() {
        format::info("Stash is empty.");
        return Ok(());
    }

    let idx = index.unwrap_or(entries.len() - 1);
    if idx >= entries.len() {
        format::error(format!("no entry at index {idx} (stash has {} items, indices 0..{})", entries.len(), entries.len() - 1));
        return Ok(());
    }

    let entry = if keep { entries[idx].clone() } else { entries.remove(idx) };
    if !keep {
        storage::save(&entries)?;
    }

    format::heading(&format!("[{}] {}", idx, entry.kind.label()));
    println!("{}", entry.content);

    if copy {
        if let Ok(mut clipboard) = Clipboard::new() {
            let _ = clipboard.set_text(entry.content.clone());
            format::info(format!("  {}", "copied to clipboard".dimmed()));
        }
    }

    Ok(())
}

fn grep(term: &str) -> Result<()> {
    let entries = storage::load()?;
    let needle = term.to_lowercase();
    let matches: Vec<(usize, &StashEntry)> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.content.to_lowercase().contains(&needle))
        .collect();

    if matches.is_empty() {
        format::info(format!("No stash entries match '{term}'."));
        return Ok(());
    }

    format::heading(&format!("Matches for '{term}'"));
    for (i, entry) in matches {
        println!("  {:>3}  {:<9} {}", i.to_string().dimmed(), entry.kind.label(), preview(&entry.content, entry.kind));
    }
    Ok(())
}

fn clear() -> Result<()> {
    storage::save(&[])?;
    format::info("Stash cleared.");
    Ok(())
}

fn preview(content: &str, kind: StashKind) -> String {
    if matches!(kind, StashKind::Password) {
        return "*".repeat(content.trim().chars().count().min(12).max(4));
    }
    let one_line = content.replace(['\n', '\r'], " ");
    let one_line = one_line.trim();
    if one_line.chars().count() > 80 {
        format!("{}...", one_line.chars().take(77).collect::<String>())
    } else {
        one_line.to_string()
    }
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}
