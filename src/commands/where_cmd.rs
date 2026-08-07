use crate::util::format;
use crate::util::system;
use anyhow::Result;
use colored::Colorize;
use std::path::Path;

const MAX_FILE_RESULTS: usize = 25;
const MAX_SCAN_ENTRIES: usize = 200_000;

pub fn run(query: &str) -> Result<()> {
    let query = query.trim();
    if query.is_empty() {
        format::error("tell me what to look for, e.g. `rooter where node` or `rooter where config.json`");
        return Ok(());
    }

    let mut found_anything = false;

    if let Ok(n) = query.parse::<u32>() {
        found_anything |= show_number_matches(n);
    }

    found_anything |= show_path_matches(query);
    found_anything |= show_binaries(query);
    found_anything |= show_processes(query);

    if !found_anything {
        format::heading("No matches");
        format::bullet(format!("Nothing in PATH, running processes, or the current directory matched '{query}'."));
    }

    Ok(())
}

fn show_number_matches(n: u32) -> bool {
    let sys = system::fresh_system();
    let mut any = false;

    if system::find_process(&sys, n).is_some() {
        any = true;
        format::heading(&format!("PID {n}"));
        format::bullet(format!("A process is running with this PID. Run `rooter why {n}` for detail."));
    }

    if n <= 65535 {
        let matches = system::sockets_on_port(n as u16);
        if !matches.is_empty() {
            any = true;
            format::heading(&format!("Port {n}"));
            for m in matches {
                format::bullet(format!("{} - {}", m.protocol, m.state));
            }
            format::bullet(format!("Run `rooter why {n}` for the owning process."));
        }
    }

    any
}

fn show_binaries(query: &str) -> bool {
    let mut matches: Vec<std::path::PathBuf> = which::which_all(query)
        .map(|iter| iter.filter(|p| is_executable_candidate(p)).collect())
        .unwrap_or_default();
    matches.dedup();

    if matches.is_empty() {
        return false;
    }

    format::heading("Binaries in PATH");
    for m in &matches {
        format::bullet(m.display());
    }
    true
}

fn show_processes(query: &str) -> bool {
    let sys = system::fresh_system();
    let matches = system::processes_matching(&sys, query);
    if matches.is_empty() {
        return false;
    }

    format::heading("Running processes");
    for (pid, process) in matches.iter().take(15) {
        format::bullet(format!(
            "{} (PID {}){}",
            process.name().to_string_lossy().bold(),
            pid.as_u32(),
            process
                .exe()
                .map(|e| format!("  {}", e.display()))
                .unwrap_or_default()
        ));
    }
    if matches.len() > 15 {
        format::bullet(format!("... and {} more", matches.len() - 15));
    }
    true
}

fn show_path_matches(query: &str) -> bool {
    let cwd = match std::env::current_dir() {
        Ok(c) => c,
        Err(_) => return false,
    };

    let needle = query.to_lowercase();
    let mut results: Vec<std::path::PathBuf> = Vec::new();
    let mut scanned = 0usize;

    for entry in walkdir::WalkDir::new(&cwd)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path()))
        .filter_map(|e| e.ok())
    {
        scanned += 1;
        if scanned > MAX_SCAN_ENTRIES || results.len() >= MAX_FILE_RESULTS {
            break;
        }
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name.contains(&needle) {
            results.push(entry.path().to_path_buf());
        }
    }

    if results.is_empty() {
        return false;
    }

    format::heading(&format!("Files under {}", cwd.display()));
    for r in &results {
        format::bullet(r.display());
    }
    if results.len() >= MAX_FILE_RESULTS {
        format::bullet("... results capped, narrow your query for more");
    }
    true
}

/// `which_all` mimics Windows' "check the current directory first" behavior,
/// which means it happily "finds" plain data files like `Cargo.toml` sitting
/// in cwd. Filter down to things that are actually runnable.
fn is_executable_candidate(path: &Path) -> bool {
    #[cfg(windows)]
    {
        let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
        let valid: Vec<String> = pathext.split(';').map(|e| e.trim().to_lowercase()).collect();
        match path.extension() {
            Some(ext) => valid.contains(&format!(".{}", ext.to_string_lossy().to_lowercase())),
            None => false,
        }
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
}

fn is_ignored(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some(".git") | Some("node_modules") | Some("target") | Some(".venv") | Some("__pycache__")
    )
}
