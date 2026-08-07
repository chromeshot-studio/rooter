use crate::cli::{Base64Mode, GenCommand, StashCommand};
use crate::commands::{ask, clean, config as config_cmd, doctor, envcheck, gen, kill, net, ports, secrets, serve, ship, stash, where_cmd, why};
use crate::util::format;
use anyhow::Result;
use colored::Colorize;
use inquire::ui::{Color, RenderConfig, StyleSheet, Styled};
use inquire::{Confirm, Select, Text};

const MAIN_MENU: &[&str] = &[
    "why      - explain a PID, port, path, or question",
    "where    - find a binary, process, file, or port",
    "ship     - stage, commit, and push your work",
    "envcheck - check the environment for broken dependencies",
    "doctor   - full health report on this folder",
    "stash    - a developer clipboard, auto-classified",
    "kill     - kill a process by PID, port, or name",
    "ports    - list every listening/bound port",
    "clean    - reclaim disk space from build artifacts",
    "secrets  - scan for leaked API keys/tokens",
    "gen      - uuid, password, hash, base64, jwt decode",
    "serve    - serve the current directory over HTTP",
    "net      - local IP, DNS resolution, reachability",
    "ask      - ask a local LLM a question",
    "config   - view/set the local LLM endpoint",
    "help     - show example commands for scripting/CI",
    "exit",
];

fn theme() -> RenderConfig<'static> {
    RenderConfig::default_colored()
        .with_prompt_prefix(Styled::new("\u{203a}").with_fg(Color::LightCyan))
        .with_answered_prompt_prefix(Styled::new("\u{2713}").with_fg(Color::LightGreen))
        .with_highlighted_option_prefix(Styled::new("\u{276f}").with_fg(Color::LightCyan))
        .with_selected_option(Some(StyleSheet::new().with_fg(Color::LightCyan)))
        .with_answer(StyleSheet::new().with_fg(Color::LightCyan))
        .with_help_message(StyleSheet::new().with_fg(Color::DarkGrey))
}

pub fn run() -> Result<()> {
    inquire::set_global_render_config(theme());
    format::banner();
    println!("{}\n", "navigate with arrows, Enter to select, Esc to quit".dimmed());

    loop {
        let choice = Select::new("What do you need?", MAIN_MENU.to_vec()).prompt();
        let Ok(choice) = choice else {
            break;
        };

        let result = match first_word(choice) {
            "why" => prompt_and_run("What are you asking about?", why::run),
            "where" => prompt_and_run("What are you looking for?", where_cmd::run),
            "ship" => {
                let ai = Confirm::new("Use a local LLM to write the commit message?")
                    .with_default(false)
                    .prompt()
                    .unwrap_or(false);
                ship::run(ship::ShipOptions {
                    message: None,
                    yes: false,
                    no_push: false,
                    pr: false,
                    ai,
                })
            }
            "envcheck" => envcheck::run(),
            "doctor" => doctor::run(),
            "stash" => run_stash_menu(),
            "kill" => run_kill(),
            "ports" => run_ports(),
            "clean" => clean::run(false, false),
            "secrets" => secrets::run(optional_text("Path to scan (blank = current directory):")).map(|_| ()),
            "gen" => run_gen_menu(),
            "serve" => run_serve(),
            "net" => net::run(optional_text("Host to check (blank = show local IP):")),
            "ask" => match Text::new("Ask:").prompt() {
                Ok(q) if !q.trim().is_empty() => ask::run(&q),
                _ => Ok(()),
            },
            "config" => config_cmd::run(
                optional_text("LLM URL (blank = leave unchanged):"),
                optional_text("LLM model (blank = leave unchanged):"),
            ),
            "help" => {
                println!("{}", crate::cli::EXAMPLES);
                Ok(())
            }
            "exit" => break,
            _ => Ok(()),
        };

        if let Err(e) = result {
            format::error(e);
        }

        println!();
    }

    Ok(())
}

fn first_word(s: &str) -> &str {
    s.split_whitespace().next().unwrap_or("")
}

fn prompt_and_run(prompt: &str, f: fn(&str) -> Result<()>) -> Result<()> {
    let Ok(query) = Text::new(prompt).prompt() else {
        return Ok(());
    };
    f(&query)
}

/// A `Text` prompt where an empty answer means "use the default" (`None`).
fn optional_text(prompt: &str) -> Option<String> {
    let answer = Text::new(prompt).prompt().unwrap_or_default();
    let trimmed = answer.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn run_kill() -> Result<()> {
    let Ok(target) = Text::new("PID, port, or process name to kill:").prompt() else {
        return Ok(());
    };
    if target.trim().is_empty() {
        return Ok(());
    }
    kill::run(&target, false)
}

fn run_ports() -> Result<()> {
    ports::run(optional_text("Filter by port or process name (blank = show all):"))
}

fn run_serve() -> Result<()> {
    let port = optional_text("Port (blank = 8080):").and_then(|s| s.parse::<u16>().ok());
    let dir = optional_text("Directory to serve (blank = current directory):").map(std::path::PathBuf::from);
    let open_browser = Confirm::new("Open in browser?").with_default(true).prompt().unwrap_or(false);
    serve::run(port, dir, open_browser)
}

const STASH_MENU: &[&str] = &[
    "capture (auto-detect)",
    "capture as code",
    "capture as url",
    "capture as json",
    "capture as password",
    "list",
    "pop most recent",
    "grep",
    "clear",
    "back",
];

fn run_stash_menu() -> Result<()> {
    loop {
        let choice = Select::new("Stash:", STASH_MENU.to_vec()).prompt();
        let Ok(choice) = choice else {
            return Ok(());
        };

        let action = match choice {
            "capture (auto-detect)" => stash::run(None),
            "capture as code" => stash::run(Some(StashCommand::Code)),
            "capture as url" => stash::run(Some(StashCommand::Url)),
            "capture as json" => stash::run(Some(StashCommand::Json)),
            "capture as password" => stash::run(Some(StashCommand::Password)),
            "list" => stash::run(Some(StashCommand::List)),
            "pop most recent" => stash::run(Some(StashCommand::Pop {
                index: None,
                copy: true,
                keep: false,
            })),
            "grep" => {
                let Ok(term) = Text::new("Search for:").prompt() else {
                    continue;
                };
                stash::run(Some(StashCommand::Grep { term }))
            }
            "clear" => stash::run(Some(StashCommand::Clear)),
            "back" => return Ok(()),
            _ => Ok(()),
        };

        if let Err(e) = action {
            format::error(e);
        }
        println!();
    }
}

const GEN_MENU: &[&str] = &["uuid", "password", "hash", "base64 encode", "base64 decode", "jwt decode", "back"];

fn run_gen_menu() -> Result<()> {
    loop {
        let choice = Select::new("Generate:", GEN_MENU.to_vec()).prompt();
        let Ok(choice) = choice else {
            return Ok(());
        };

        let action = match choice {
            "uuid" => {
                let count = optional_text("How many? (blank = 1):").and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
                gen::run(GenCommand::Uuid { count })
            }
            "password" => {
                let length = optional_text("Length (blank = 20):").and_then(|s| s.parse::<usize>().ok()).unwrap_or(20);
                let simple = Confirm::new("Letters and digits only (no symbols)?").with_default(false).prompt().unwrap_or(false);
                gen::run(GenCommand::Password { length, simple })
            }
            "hash" => {
                let Ok(text) = Text::new("Text to hash:").prompt() else { continue };
                let algo = Select::new("Algorithm:", vec!["sha256", "md5"]).prompt().unwrap_or("sha256");
                gen::run(GenCommand::Hash { text: Some(text), file: None, algo: algo.to_string() })
            }
            "base64 encode" => {
                let Ok(text) = Text::new("Text to encode:").prompt() else { continue };
                gen::run(GenCommand::Base64 { mode: Base64Mode::Encode, text: Some(text), file: None })
            }
            "base64 decode" => {
                let Ok(text) = Text::new("Text to decode:").prompt() else { continue };
                gen::run(GenCommand::Base64 { mode: Base64Mode::Decode, text: Some(text), file: None })
            }
            "jwt decode" => {
                let Ok(token) = Text::new("JWT:").prompt() else { continue };
                gen::run(GenCommand::Jwt { token })
            }
            "back" => return Ok(()),
            _ => Ok(()),
        };

        if let Err(e) = action {
            format::error(e);
        }
        println!();
    }
}
