use crate::util::format::{self, size};
use crate::util::system::dir_size_capped;
use anyhow::Result;
use inquire::MultiSelect;
use std::path::{Path, PathBuf};

const ARTIFACT_NAMES: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".nuxt",
    "__pycache__",
    ".pytest_cache",
    ".turbo",
    ".parcel-cache",
    "coverage",
    ".cache",
];
const MAX_DEPTH: usize = 6;
const MAX_ENTRIES_PER_DIR: usize = 300_000;

pub fn run(yes: bool, force: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    format::heading("Scanning for build artifacts and caches");

    let candidates = find_candidates(&cwd);
    if candidates.is_empty() {
        format::info("Nothing found to clean.");
        return Ok(());
    }

    let sized: Vec<(PathBuf, u64)> = candidates
        .into_iter()
        .map(|p| {
            let bytes = dir_size_capped(&p, MAX_ENTRIES_PER_DIR);
            (p, bytes)
        })
        .collect();

    let total: u64 = sized.iter().map(|(_, b)| b).sum();
    format::row("Found", format!("{} item(s), {} reclaimable", sized.len(), size(total)));

    let labels: Vec<String> = sized
        .iter()
        .map(|(p, b)| format!("{}  {}", size(*b), p.strip_prefix(&cwd).unwrap_or(p).display()))
        .collect();

    let to_delete: Vec<usize> = if yes {
        (0..labels.len()).collect()
    } else {
        let selected = MultiSelect::new("Select what to delete:", labels.clone())
            .prompt()
            .unwrap_or_default();
        selected
            .iter()
            .filter_map(|s| labels.iter().position(|l| l == s))
            .collect()
    };

    if to_delete.is_empty() {
        format::info("Nothing selected, nothing deleted.");
        return Ok(());
    }

    if !force {
        let reclaimed: u64 = to_delete.iter().map(|&i| sized[i].1).sum();
        let confirmed = inquire::Confirm::new(&format!("Delete {} item(s), freeing {}?", to_delete.len(), size(reclaimed)))
            .with_default(false)
            .prompt()
            .unwrap_or(false);
        if !confirmed {
            format::info("Cancelled.");
            return Ok(());
        }
    }

    for i in to_delete {
        let (path, bytes) = &sized[i];
        match std::fs::remove_dir_all(path) {
            Ok(_) => format::ok(format!("removed {} ({})", path.display(), size(*bytes))),
            Err(e) => format::fail(format!("couldn't remove {}: {e}", path.display())),
        }
    }

    Ok(())
}

fn find_candidates(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    walk(root, 0, &mut found);
    found
}

fn walk(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };

    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else { continue };
        if !file_type.is_dir() {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if ARTIFACT_NAMES.contains(&name.as_str()) {
            found.push(path);
            continue; // don't recurse into a directory we're about to offer to delete
        }
        if name == ".git" {
            continue;
        }
        walk(&path, depth + 1, found);
    }
}
