use colored::Colorize;
use humansize::{format_size, DECIMAL};

/// The bordered title card shown once, at the top of the interactive menu.
pub fn banner() {
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    let subtitle = "explains your system, ships your code, keeps things honest";
    let inner_width = subtitle.chars().count() + 4;

    println!("{}", format!("\u{256d}{}\u{256e}", "\u{2500}".repeat(inner_width)).cyan());

    let title = "rooter";
    let left = format!("  {title}");
    let right = format!("{version}  ");
    let pad = inner_width.saturating_sub(left.chars().count() + right.chars().count());
    println!(
        "{}{}{}{}{}",
        "\u{2502}".cyan(),
        left.bold().cyan(),
        " ".repeat(pad),
        right.dimmed(),
        "\u{2502}".cyan()
    );

    let sub_content = format!("  {subtitle}");
    let sub_pad = inner_width.saturating_sub(sub_content.chars().count());
    println!(
        "{}{}{}{}",
        "\u{2502}".cyan(),
        sub_content.dimmed(),
        " ".repeat(sub_pad),
        "\u{2502}".cyan()
    );

    println!("{}", format!("\u{2570}{}\u{256f}", "\u{2500}".repeat(inner_width)).cyan());
    println!();
}

pub fn heading(text: &str) {
    println!("\n{} {}", "\u{25c6}".cyan().bold(), text.bold());
}

pub fn subheading(text: &str) {
    println!("\n  {} {}", "\u{b7}".cyan(), text.bold());
}

pub fn row(key: &str, value: impl std::fmt::Display) {
    println!("  {:<18} {}", key.dimmed(), value);
}

pub fn bullet(text: impl std::fmt::Display) {
    println!("  {} {}", "\u{203a}".cyan(), text);
}

pub fn ok(text: impl std::fmt::Display) {
    println!("  {} {}", "\u{2713}".green().bold(), text);
}

pub fn warn(text: impl std::fmt::Display) {
    println!("  {} {}", "\u{26a0}".yellow().bold(), text);
}

pub fn fail(text: impl std::fmt::Display) {
    println!("  {} {}", "\u{2717}".red().bold(), text);
}

pub fn info(text: impl std::fmt::Display) {
    println!("{}", text);
}

pub fn size(bytes: u64) -> String {
    format_size(bytes, DECIMAL)
}

/// Strips the Windows `\\?\` extended-length-path prefix that
/// `fs::canonicalize` adds, purely for cleaner display.
pub fn clean_path(path: &std::path::Path) -> String {
    let s = path.display().to_string();
    s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
}

pub fn error(text: impl std::fmt::Display) {
    eprintln!("{} {}", "\u{2717}".red().bold(), text.to_string().red());
}
