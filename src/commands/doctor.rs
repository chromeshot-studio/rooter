use crate::util::format::{self, size};
use crate::util::system;
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

const IGNORED_DIRS: &[&str] = &[".git", "node_modules", "target", ".venv", "venv", "__pycache__", "dist", "build", ".next"];
const MAX_WALK_ENTRIES: usize = 50_000;
const BIG_FILE_THRESHOLD: u64 = 10 * 1024 * 1024;
const TODO_MARKERS: &[&str] = &["TODO", "FIXME", "HACK", "XXX"];
const TEXT_EXTENSIONS: &[&str] = &[
    "rs", "js", "ts", "tsx", "jsx", "py", "go", "java", "c", "cpp", "h", "hpp", "cs", "rb", "php",
    "swift", "kt", "md", "txt", "json", "yaml", "yml", "toml", "html", "css",
];

pub fn run() -> Result<()> {
    let cwd = std::env::current_dir()?;
    format::heading(&format!("Doctor's report for {}", cwd.display()));

    report_project_types(&cwd);
    report_git(&cwd);
    let scan = walk_once(&cwd);
    report_size(&cwd, &scan);
    report_extensions(&scan);
    report_big_files(&scan);
    report_todos(&scan);

    Ok(())
}

fn report_project_types(cwd: &Path) {
    format::subheading("Project type");
    let markers: [(&str, &str); 6] = [
        ("package.json", "Node.js"),
        ("Cargo.toml", "Rust"),
        ("pyproject.toml", "Python"),
        ("go.mod", "Go"),
        ("Gemfile", "Ruby"),
        ("pom.xml", "Java (Maven)"),
    ];
    let mut found = false;
    for (file, label) in markers {
        if cwd.join(file).exists() {
            format::bullet(label);
            found = true;
        }
    }
    if !found {
        format::bullet("Unrecognized - no common manifest file found");
    }
    format::info(format!("  {}", "run `rooter envcheck` for a dependency-level check".to_string()));
}

fn report_git(cwd: &Path) {
    format::subheading("Git");
    if !cwd.join(".git").exists() {
        format::warn("not a git repository");
        return;
    }

    let branch = run_git(cwd, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let status = run_git(cwd, &["status", "--porcelain"]);
    let ahead_behind = run_git(cwd, &["rev-list", "--left-right", "--count", "HEAD...@{u}"]);

    if let Some(branch) = branch {
        format::row("Branch", branch);
    }

    match status {
        Some(s) if s.trim().is_empty() => format::ok("working tree clean"),
        Some(s) => {
            let count = s.lines().filter(|l| !l.trim().is_empty()).count();
            format::warn(format!("{count} uncommitted change(s)"));
        }
        None => format::warn("git not available or this isn't a valid repo"),
    }

    if let Some(ab) = ahead_behind {
        let parts: Vec<&str> = ab.split_whitespace().collect();
        if parts.len() == 2 {
            format::row("Ahead/behind upstream", format!("{} ahead, {} behind", parts[0], parts[1]));
        }
    }
}

fn run_git(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git").current_dir(cwd).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim_end().to_string())
}

struct ScanResult {
    file_count: u64,
    dir_count: u64,
    extensions: HashMap<String, u64>,
    big_files: Vec<(std::path::PathBuf, u64)>,
    todo_hits: Vec<(std::path::PathBuf, usize, String)>,
}

fn walk_once(cwd: &Path) -> ScanResult {
    let mut result = ScanResult {
        file_count: 0,
        dir_count: 0,
        extensions: HashMap::new(),
        big_files: Vec::new(),
        todo_hits: Vec::new(),
    };

    let walker = walkdir::WalkDir::new(cwd)
        .into_iter()
        .filter_entry(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| !IGNORED_DIRS.contains(&n))
                .unwrap_or(true)
        });

    for (count, entry) in walker.filter_map(|e| e.ok()).enumerate() {
        if count >= MAX_WALK_ENTRIES {
            break;
        }

        if entry.file_type().is_dir() {
            result.dir_count += 1;
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }

        result.file_count += 1;
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("(none)").to_lowercase();
        *result.extensions.entry(ext.clone()).or_insert(0) += 1;

        let len = entry.metadata().map(|m| m.len()).unwrap_or(0);
        if len >= BIG_FILE_THRESHOLD {
            result.big_files.push((path.to_path_buf(), len));
        }

        if TEXT_EXTENSIONS.contains(&ext.as_str()) && len < 2 * 1024 * 1024 {
            scan_todos(path, &mut result.todo_hits);
        }
    }

    result.big_files.sort_by(|a, b| b.1.cmp(&a.1));
    result
}

fn scan_todos(path: &Path, hits: &mut Vec<(std::path::PathBuf, usize, String)>) {
    let Ok(content) = std::fs::read_to_string(path) else { return };
    for (lineno, line) in content.lines().enumerate() {
        if TODO_MARKERS.iter().any(|m| line.contains(m)) {
            hits.push((path.to_path_buf(), lineno + 1, line.trim().to_string()));
        }
    }
}

fn report_size(cwd: &Path, scan: &ScanResult) {
    format::subheading("Size");
    format::row("Files", scan.file_count);
    format::row("Directories", scan.dir_count);

    let largest = system::largest_subdirs(cwd, 5);
    if !largest.is_empty() {
        format::info("  Largest top-level items:");
        for (path, bytes) in largest {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            format::bullet(format!("{}  {}", size(bytes), name));
        }
    }
}

fn report_extensions(scan: &ScanResult) {
    if scan.extensions.is_empty() {
        return;
    }
    format::subheading("File types");
    let mut sorted: Vec<(&String, &u64)> = scan.extensions.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    for (ext, count) in sorted.into_iter().take(8) {
        format::bullet(format!("{count:>6}  .{ext}"));
    }
}

fn report_big_files(scan: &ScanResult) {
    if scan.big_files.is_empty() {
        return;
    }
    format::subheading(&format!("Large files (>{})", size(BIG_FILE_THRESHOLD)));
    for (path, bytes) in scan.big_files.iter().take(10) {
        format::bullet(format!("{}  {}", size(*bytes), path.display()));
    }
}

fn report_todos(scan: &ScanResult) {
    format::subheading("TODO / FIXME / HACK");
    if scan.todo_hits.is_empty() {
        format::ok("none found");
        return;
    }
    format::warn(format!("{} marker(s) found", scan.todo_hits.len()));
    for (path, lineno, line) in scan.todo_hits.iter().take(10) {
        format::bullet(format!("{}:{}  {}", path.display(), lineno, line));
    }
    if scan.todo_hits.len() > 10 {
        format::bullet(format!("... and {} more", scan.todo_hits.len() - 10));
    }
}
