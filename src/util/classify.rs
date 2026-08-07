use std::path::PathBuf;

/// What a `why`/`where` query looks like.
pub enum InputKind {
    /// A bare number: could be a PID, a port, or both.
    Number(u32),
    /// A path that exists on disk.
    Path(PathBuf),
    /// Free-text, natural-language-ish query.
    Text(String),
}

pub fn classify_query(raw: &str) -> InputKind {
    let trimmed = raw.trim();

    if let Ok(n) = trimmed.parse::<u32>() {
        return InputKind::Number(n);
    }

    let expanded = expand_tilde(trimmed);
    if expanded.exists() {
        return InputKind::Path(expanded);
    }

    InputKind::Text(trimmed.to_string())
}

/// Expands a leading `~` to the user's home directory. Cross-platform;
/// no-op if there's no home dir or no leading `~`.
pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~") {
        if let Some(home) = dirs::home_dir() {
            let rest = rest.trim_start_matches(['/', '\\']);
            if rest.is_empty() {
                return home;
            }
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StashKind {
    Code,
    Url,
    Json,
    Password,
    Text,
}

impl StashKind {
    pub fn label(&self) -> &'static str {
        match self {
            StashKind::Code => "code",
            StashKind::Url => "url",
            StashKind::Json => "json",
            StashKind::Password => "password",
            StashKind::Text => "text",
        }
    }
}

/// Best-effort auto-classification of clipboard/text content for `stash`.
pub fn classify_content(content: &str) -> StashKind {
    let trimmed = content.trim();

    if trimmed.is_empty() {
        return StashKind::Text;
    }

    if looks_like_json(trimmed) {
        return StashKind::Json;
    }

    if looks_like_url(trimmed) {
        return StashKind::Url;
    }

    if looks_like_code(trimmed) {
        return StashKind::Code;
    }

    if looks_like_password(trimmed) {
        return StashKind::Password;
    }

    StashKind::Text
}

fn looks_like_json(s: &str) -> bool {
    let starts_ok = s.starts_with('{') || s.starts_with('[');
    if !starts_ok {
        return false;
    }
    serde_json::from_str::<serde_json::Value>(s).is_ok()
}

fn looks_like_url(s: &str) -> bool {
    if s.contains(char::is_whitespace) {
        return false;
    }
    let schemes = ["http://", "https://", "ftp://", "git@", "ssh://", "ws://", "wss://"];
    if schemes.iter().any(|p| s.starts_with(p)) {
        return true;
    }
    // bare domain like example.com/path
    let re = regex::Regex::new(r"^[a-zA-Z0-9-]+(\.[a-zA-Z0-9-]+)+(:\d+)?(/\S*)?$").unwrap();
    re.is_match(s)
}

fn looks_like_code(s: &str) -> bool {
    let multiline = s.lines().count() > 1;
    let code_tokens = [
        "function ", "def ", "class ", "import ", "const ", "let ", "var ", "=>", "public ",
        "private ", "#include", "fn ", "pub fn", "SELECT ", "select ", "</", "<div", "package ",
        "using ", "namespace ",
    ];
    let has_token = code_tokens.iter().any(|t| s.contains(t));
    let punct_heavy = {
        let punct = s.chars().filter(|c| "{}();=<>".contains(*c)).count();
        (punct as f64) / (s.len().max(1) as f64) > 0.04
    };
    has_token || (multiline && punct_heavy)
}

fn looks_like_password(s: &str) -> bool {
    if s.lines().count() != 1 {
        return false;
    }
    let len = s.chars().count();
    if !(8..=64).contains(&len) || s.contains(char::is_whitespace) {
        return false;
    }
    let has_lower = s.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = s.chars().any(|c| c.is_ascii_uppercase());
    let has_digit = s.chars().any(|c| c.is_ascii_digit());
    let has_symbol = s.chars().any(|c| !c.is_ascii_alphanumeric());
    let variety = [has_lower, has_upper, has_digit, has_symbol]
        .iter()
        .filter(|b| **b)
        .count();
    variety >= 3
}
