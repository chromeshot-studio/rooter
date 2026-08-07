use crate::util::format;
use anyhow::Result;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const IGNORED_DIRS: &[&str] = &[".git", "node_modules", "target", "dist", "build", ".venv", "venv", "__pycache__"];
const MAX_FILE_SIZE: u64 = 2 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_000;

pub struct Hit {
    pub path: PathBuf,
    pub line: usize,
    pub rule: &'static str,
    pub redacted: String,
}

struct Rule {
    name: &'static str,
    pattern: &'static str,
}

const RULES: &[Rule] = &[
    Rule { name: "AWS access key", pattern: r"AKIA[0-9A-Z]{16}" },
    Rule { name: "GitHub token", pattern: r"gh[pousr]_[A-Za-z0-9]{36,}" },
    Rule { name: "Slack token", pattern: r"xox[baprs]-[0-9A-Za-z-]{10,}" },
    Rule { name: "Stripe live key", pattern: r"sk_live_[0-9a-zA-Z]{16,}" },
    Rule { name: "Google API key", pattern: r"AIza[0-9A-Za-z\-_]{35}" },
    Rule { name: "Private key block", pattern: r"-----BEGIN (RSA |EC |OPENSSH |DSA |PGP )?PRIVATE KEY-----" },
    Rule {
        name: "possible hardcoded secret",
        pattern: r#"(?i)(api[_-]?key|secret|token|password|passwd|pwd)\s*[:=]\s*["'][A-Za-z0-9_\-/+]{16,}["']"#,
    },
];

fn compiled() -> &'static Vec<(Regex, &'static str)> {
    static CELL: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    CELL.get_or_init(|| RULES.iter().map(|r| (Regex::new(r.pattern).unwrap(), r.name)).collect())
}

/// Returns `Ok(true)` if the scan came back clean, `Ok(false)` if it found
/// something - the caller decides what that should mean for the exit code.
pub fn run(path: Option<String>) -> Result<bool> {
    let root = match path {
        Some(p) => crate::util::classify::expand_tilde(&p),
        None => std::env::current_dir()?,
    };

    format::heading(&format!("Scanning {} for leaked secrets", root.display()));
    let hits = scan(&root);

    if hits.is_empty() {
        format::ok("no obvious secrets found");
        return Ok(true);
    }

    format::warn(format!("{} possible secret(s) found", hits.len()));
    for hit in &hits {
        format::bullet(format!("{}:{}  [{}]  {}", hit.path.display(), hit.line, hit.rule, hit.redacted));
    }
    format::info("");
    format::info("  these may be false positives - double check before assuming the worst");

    Ok(false)
}

/// Scans `root` for likely secrets, skipping binary/huge files and common
/// vendored/build directories. Bounded so it can't hang on a huge tree.
pub fn scan(root: &Path) -> Vec<Hit> {
    let mut hits = Vec::new();
    let walker = walkdir::WalkDir::new(root).into_iter().filter_entry(|e| {
        e.path()
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| !IGNORED_DIRS.contains(&n))
            .unwrap_or(true)
    });

    for (count, entry) in walker.filter_map(|e| e.ok()).enumerate() {
        if count >= MAX_ENTRIES {
            break;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if meta.len() > MAX_FILE_SIZE {
            continue;
        }
        scan_file(entry.path(), &mut hits);
    }

    hits
}

pub fn scan_paths<'a>(paths: impl Iterator<Item = &'a Path>) -> Vec<Hit> {
    let mut hits = Vec::new();
    for path in paths {
        if path.is_file() {
            scan_file(path, &mut hits);
        }
    }
    hits
}

fn scan_file(path: &Path, hits: &mut Vec<Hit>) {
    let Ok(content) = std::fs::read_to_string(path) else { return };
    for (lineno, line) in content.lines().enumerate() {
        for (re, name) in compiled() {
            if let Some(m) = re.find(line) {
                hits.push(Hit {
                    path: path.to_path_buf(),
                    line: lineno + 1,
                    rule: name,
                    redacted: redact(m.as_str()),
                });
            }
        }
    }
}

fn redact(secret: &str) -> String {
    let chars: Vec<char> = secret.chars().collect();
    if chars.len() <= 8 {
        return "*".repeat(chars.len());
    }
    let head: String = chars[..4].iter().collect();
    let tail: String = chars[chars.len() - 2..].iter().collect();
    format!("{head}{}{tail}", "*".repeat(chars.len() - 6))
}
