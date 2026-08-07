use crate::util::classify::{classify_query, InputKind};
use crate::util::format::{self, size};
use crate::util::system::{self, PortMatch};
use anyhow::Result;
use colored::Colorize;
use std::fs;
use std::path::Path;
use std::time::SystemTime;
use sysinfo::{Process, System};

pub fn run(query: &str) -> Result<()> {
    if query.trim().is_empty() {
        format::error("nothing to explain. Try `rooter why 3000`, `rooter why <path>`, or `rooter why \"my disk is full\"`.");
        return Ok(());
    }

    match classify_query(query) {
        InputKind::Number(n) => explain_number(n),
        InputKind::Path(p) => explain_path(&p),
        InputKind::Text(t) => explain_text(&t),
    }

    Ok(())
}

fn explain_number(n: u32) {
    let sys = system::fresh_system();
    let mut found_anything = false;

    if let Some(process) = system::find_process(&sys, n) {
        found_anything = true;
        format::heading(&format!("PID {n} is a running process"));
        explain_process(&sys, n, process);
    }

    if n <= 65535 {
        let matches = system::sockets_on_port(n as u16);
        if !matches.is_empty() {
            found_anything = true;
            format::heading(&format!("Port {n} is in use"));
            explain_port(&sys, &matches);
        }
    }

    if !found_anything {
        format::heading(&format!("Nothing found for {n}"));
        format::bullet("No process has this PID right now.");
        if n <= 65535 {
            format::bullet("No socket is bound to this port right now.");
        }
        format::bullet("It may have belonged to something that already exited.");
    }
}

pub(crate) fn explain_process(sys: &System, pid: u32, process: &Process) {
    format::row("Name", process.name().to_string_lossy());
    format::row("PID", pid);

    if let Some(parent_pid) = process.parent() {
        let parent_name = sys
            .process(parent_pid)
            .map(|p| p.name().to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        format::row("Parent", format!("{parent_name} (PID {})", parent_pid.as_u32()));
    }

    format::row("Status", format!("{:?}", process.status()));

    if let Some(exe) = process.exe() {
        format::row("Executable", exe.display());
    }

    let cmd: Vec<String> = process
        .cmd()
        .iter()
        .map(|s| s.to_string_lossy().to_string())
        .collect();
    if !cmd.is_empty() {
        format::row("Command", cmd.join(" "));
    }

    if let Some(cwd) = process.cwd() {
        format::row("Working dir", cwd.display());
    }

    format::row("CPU", format!("{:.1}%", process.cpu_usage()));
    format::row("Memory", size(process.memory()));

    let run_secs = process.run_time();
    format::row("Running for", human_duration(run_secs));

    let disk = process.disk_usage();
    if disk.total_read_bytes > 0 || disk.total_written_bytes > 0 {
        format::row(
            "Disk I/O (lifetime)",
            format!("{} read, {} written", size(disk.total_read_bytes), size(disk.total_written_bytes)),
        );
    }

    let owned_ports: Vec<PortMatch> = system::all_sockets()
        .into_iter()
        .filter(|m| m.pids.contains(&pid))
        .collect();
    if !owned_ports.is_empty() {
        let list: Vec<String> = owned_ports
            .iter()
            .map(|m| format!("{}/{} ({})", m.local_port, m.protocol, m.state))
            .collect();
        format::row("Listening on", list.join(", "));
    }
}

fn explain_port(sys: &System, matches: &[PortMatch]) {
    let mut seen = std::collections::HashSet::new();
    for m in matches {
        let key = (m.protocol, m.state.clone(), m.pids.clone());
        if !seen.insert(key) {
            continue;
        }

        format::subheading(&format!("{} - {}", m.protocol, m.state));
        if m.pids.is_empty() {
            format::bullet("No owning process could be determined (may require elevated permissions).");
            continue;
        }
        for pid in &m.pids {
            if let Some(process) = system::find_process(sys, *pid) {
                let exe = process
                    .exe()
                    .map(|e| format!(" - {}", e.display()))
                    .unwrap_or_default();
                format::bullet(format!("{} (PID {}){}", process.name().to_string_lossy().bold(), pid, exe));
            } else {
                format::bullet(format!("PID {pid} (process details unavailable)"));
            }
        }
    }
    format::info("");
    format::info(format!(
        "  {}",
        "tip: `rooter why <pid>` for full detail on the owning process".dimmed()
    ));
}

fn explain_path(path: &Path) {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) => {
            format::error(format!("couldn't read metadata for {}: {e}", path.display()));
            return;
        }
    };

    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    format::heading(&format::clean_path(&canonical));

    let file_type = meta.file_type();
    let kind = if file_type.is_symlink() {
        "symlink"
    } else if file_type.is_dir() {
        "directory"
    } else {
        "file"
    };
    format::row("Type", kind);

    if file_type.is_symlink() {
        if let Ok(target) = fs::read_link(path) {
            format::row("Points to", target.display());
        }
    }

    if meta.is_file() {
        format::row("Size", size(meta.len()));
        format::row("Category", guess_category(path));
    } else if meta.is_dir() {
        match fs::read_dir(path) {
            Ok(entries) => {
                let (mut files, mut dirs, mut size_sum) = (0u64, 0u64, 0u64);
                for entry in entries.flatten() {
                    if let Ok(m) = entry.metadata() {
                        if m.is_dir() {
                            dirs += 1;
                        } else {
                            files += 1;
                            size_sum += m.len();
                        }
                    }
                }
                format::row("Contains", format!("{files} files, {dirs} subdirectories (top level)"));
                format::row("Top-level file size", size(size_sum));
                format::info(format!("  {}", "tip: `rooter doctor` for a full recursive breakdown".dimmed()));
            }
            Err(e) => format::warn(format!("couldn't list directory contents: {e}")),
        }
    }

    format::row("Read-only", meta.permissions().readonly());
    print_time("Modified", meta.modified());
    print_time("Accessed", meta.accessed());
    print_time("Created", meta.created());

    #[cfg(windows)]
    if meta.is_file() {
        check_lock_windows(path);
    }
}

fn print_time(label: &str, t: std::io::Result<SystemTime>) {
    match t {
        Ok(t) => {
            let datetime: chrono::DateTime<chrono::Local> = t.into();
            format::row(label, datetime.format("%Y-%m-%d %H:%M:%S"));
        }
        Err(_) => format::row(label, "unknown"),
    }
}

#[cfg(windows)]
fn check_lock_windows(path: &Path) {
    use std::fs::OpenOptions;
    match OpenOptions::new().write(true).open(path) {
        Ok(_) => {}
        Err(e) => {
            if e.raw_os_error() == Some(32) {
                format::warn("this file appears to be open/locked by another program right now");
            } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                // could be locked or just read-only/permissions; don't over-claim
            }
        }
    }
}

fn guess_category(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "zip" | "tar" | "gz" | "rar" | "7z" | "xz" | "bz2" => "Archive",
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico" => "Image",
        "mp4" | "mov" | "avi" | "mkv" | "webm" => "Video",
        "mp3" | "wav" | "flac" | "ogg" | "m4a" => "Audio",
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" => "Document",
        "txt" | "md" | "rst" | "log" => "Text",
        "js" | "ts" | "tsx" | "jsx" | "rs" | "py" | "go" | "java" | "c" | "cpp" | "h" | "hpp"
        | "cs" | "rb" | "php" | "swift" | "kt" => "Source code",
        "json" | "yaml" | "yml" | "toml" | "xml" | "ini" | "env" => "Config/data",
        "exe" | "msi" | "dmg" | "deb" | "rpm" | "appimage" => "Installer/executable",
        "dll" | "so" | "dylib" => "Library",
        _ => "Unknown",
    }
}

fn explain_text(text: &str) {
    let lower = text.to_lowercase();

    if lower.contains("disk") || lower.contains("space") || lower.contains("storage") {
        disk_report();
    } else if lower.contains("cpu") || lower.contains("slow") {
        cpu_report();
    } else if lower.contains("memory") || lower.contains("ram") {
        memory_report();
    } else if lower.contains("port") {
        port_report();
    } else {
        format::heading("Not sure how to explain that yet");
        format::bullet("Try a PID: `rooter why 48291`");
        format::bullet("Try a port: `rooter why 3000`");
        format::bullet("Try a path: `rooter why ~/Downloads/file.zip`");
        format::bullet("Or a phrase about disk, cpu, memory, or ports");
    }
}

fn disk_report() {
    format::heading("Disk usage");
    let disks = sysinfo::Disks::new_with_refreshed_list();
    for d in disks.list() {
        let total = d.total_space();
        let avail = d.available_space();
        let used = total.saturating_sub(avail);
        let pct = if total > 0 { (used as f64 / total as f64) * 100.0 } else { 0.0 };
        format::subheading(&d.mount_point().display().to_string());
        format::row("Used", format!("{} / {} ({:.1}%)", size(used), size(total), pct));
        format::row("Free", size(avail));
        format::row("Filesystem", d.file_system().to_string_lossy());
    }

    if let Ok(cwd) = std::env::current_dir() {
        format::subheading(&format!("Largest items in {}", cwd.display()));
        let largest = system::largest_subdirs(&cwd, 10);
        if largest.is_empty() {
            format::bullet("(nothing found, or the directory is empty)");
        }
        for (path, bytes) in largest {
            format::bullet(format!("{}  {}", size(bytes), path.display()));
        }
    }
}

fn cpu_report() {
    format::heading("Top processes by CPU");
    let sys = system::fresh_system();
    for (pid, process) in system::top_by_cpu(&sys, 10) {
        format::bullet(format!(
            "{:>5.1}%  {} (PID {})",
            process.cpu_usage(),
            process.name().to_string_lossy(),
            pid.as_u32()
        ));
    }
}

fn memory_report() {
    format::heading("Top processes by memory");
    let sys = system::fresh_system();
    for (pid, process) in system::top_by_memory(&sys, 10) {
        format::bullet(format!(
            "{:>10}  {} (PID {})",
            size(process.memory()),
            process.name().to_string_lossy(),
            pid.as_u32()
        ));
    }
}

fn port_report() {
    format::heading("Listening ports");
    format::bullet("Full list: `rooter ports`");
    let sys = system::fresh_system();
    for row in system::port_rows(&sys).into_iter().take(15) {
        let owner = match (row.pid, &row.process_name) {
            (Some(pid), Some(name)) => format!("{name} ({pid})"),
            (Some(pid), None) => pid.to_string(),
            (None, _) => "-".to_string(),
        };
        format::bullet(format!("{:<5} {:<5} {}", row.port, row.protocol, owner));
    }
}

pub(crate) fn human_duration(total_secs: u64) -> String {
    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if days > 0 {
        format!("{days}d {hours}h {mins}m")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else if mins > 0 {
        format!("{mins}m {secs}s")
    } else {
        format!("{secs}s")
    }
}
