use crate::util::format;
use anyhow::{bail, Context, Result};
use inquire::{Confirm, MultiSelect, Text};
use std::path::Path;
use std::process::Command;

pub struct ShipOptions {
    pub message: Option<String>,
    pub yes: bool,
    pub no_push: bool,
    pub pr: bool,
    pub ai: bool,
}

pub fn run(opts: ShipOptions) -> Result<()> {
    let cwd = std::env::current_dir()?;

    if git(&cwd, &["rev-parse", "--is-inside-work-tree"]).is_err() {
        bail!("this isn't a git repository (or git isn't installed)");
    }

    let status = git(&cwd, &["status", "--porcelain"])?;
    let changed: Vec<&str> = status.lines().filter(|l| !l.trim().is_empty()).collect();

    if changed.is_empty() {
        format::info("Nothing to ship - working tree is clean.");
        return Ok(());
    }

    let files: Vec<String> = changed.iter().map(|l| parse_status_line(l)).collect();

    let to_stage: Vec<String> = if opts.yes {
        files.clone()
    } else {
        format::heading("Changes");
        MultiSelect::new("Select what to ship:", files.clone())
            .with_default(&(0..files.len()).collect::<Vec<_>>())
            .prompt()
            .context("selection cancelled")?
    };

    if to_stage.is_empty() {
        format::info("Nothing selected, nothing shipped.");
        return Ok(());
    }

    let mut add_args = vec!["add", "--"];
    add_args.extend(to_stage.iter().map(|s| s.as_str()));
    git(&cwd, &add_args)?;
    format::ok(format!("staged {} file(s)", to_stage.len()));

    if !warn_on_secrets(&cwd, &to_stage, opts.yes)? {
        format::info("Ship cancelled.");
        return Ok(());
    }

    let default_message = if opts.ai {
        match generate_ai_message(&cwd) {
            Ok(msg) => msg,
            Err(e) => {
                format::warn(format!("AI commit message unavailable ({e}) - using a plain default"));
                default_commit_message(&to_stage)
            }
        }
    } else {
        default_commit_message(&to_stage)
    };
    let message = match opts.message {
        Some(m) => m,
        None if opts.yes => default_message,
        None => Text::new("Commit message:")
            .with_default(&default_message)
            .prompt()
            .context("commit message entry cancelled")?,
    };

    if message.trim().is_empty() {
        bail!("commit message can't be empty");
    }

    git(&cwd, &["commit", "-m", &message])?;
    format::ok(format!("committed: {message}"));

    if opts.no_push {
        format::info("Skipping push (--no-push).");
        return Ok(());
    }

    let should_push = opts.yes
        || Confirm::new("Push to remote?")
            .with_default(true)
            .prompt()
            .unwrap_or(false);

    if !should_push {
        format::info("Not pushed. Run `git push` when you're ready.");
        return Ok(());
    }

    let branch = git(&cwd, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let branch = branch.trim();
    let has_upstream = git(&cwd, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]).is_ok();

    if has_upstream {
        git(&cwd, &["push"])?;
    } else {
        git(&cwd, &["push", "-u", "origin", branch])?;
    }
    format::ok(format!("pushed {branch}"));

    let gh_available = which::which("gh").is_ok();
    let wants_pr = opts.pr
        || (!opts.yes
            && gh_available
            && Confirm::new("Open a pull request?")
                .with_default(false)
                .prompt()
                .unwrap_or(false));

    if wants_pr {
        if !gh_available {
            format::warn("--pr was set but the `gh` CLI isn't installed - install it from https://cli.github.com to enable this");
            return Ok(());
        }
        let output = Command::new("gh")
            .current_dir(&cwd)
            .args(["pr", "create", "--fill"])
            .output()
            .context("failed to run `gh pr create`")?;

        if output.status.success() {
            let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
            format::ok(format!("PR created: {url}"));
            let _ = open::that(&url);
        } else {
            format::warn(format!(
                "gh pr create failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
    }

    Ok(())
}

fn git(cwd: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .context("failed to run git - is it installed?")?;

    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_status_line(line: &str) -> String {
    let path = if line.len() > 3 { &line[3..] } else { line };
    if let Some((_, renamed_to)) = path.split_once(" -> ") {
        renamed_to.trim().to_string()
    } else {
        path.trim().to_string()
    }
}

fn default_commit_message(files: &[String]) -> String {
    match files.len() {
        1 => format!("Update {}", files[0]),
        n => format!("Update {n} files"),
    }
}

/// Summarizes `git diff --staged` via the local LLM into a single commit
/// message line. Bails cleanly (caller falls back to the plain heuristic)
/// if there's nothing staged or no LLM is reachable.
fn generate_ai_message(cwd: &Path) -> Result<String> {
    let diff = git(cwd, &["diff", "--staged"])?;
    if diff.trim().is_empty() {
        bail!("nothing staged to summarize");
    }
    let truncated: String = diff.chars().take(6000).collect();

    let cfg = crate::util::llm::LlmConfig::resolve();
    if !crate::util::llm::is_reachable(&cfg) {
        bail!("no local LLM reachable at {}", cfg.url);
    }

    let system = "You write concise, conventional-commit-style git commit messages (e.g. \
'fix: handle empty response body'). Respond with ONLY the commit message on a single line - \
no quotes, no explanation, no markdown.";
    let raw = crate::util::llm::chat(&cfg, system, &format!("Write a commit message for this diff:\n\n{truncated}"))?;
    let cleaned = raw.lines().next().unwrap_or("").trim().trim_matches('"').to_string();

    if cleaned.is_empty() {
        bail!("model returned an empty message");
    }
    Ok(cleaned)
}

/// Advisory secrets check on the staged files. Returns `false` if the user
/// wants to bail out instead of continuing to ship.
fn warn_on_secrets(cwd: &Path, staged: &[String], yes: bool) -> Result<bool> {
    let paths: Vec<std::path::PathBuf> = staged.iter().map(|f| cwd.join(f)).collect();
    let hits = crate::commands::secrets::scan_paths(paths.iter().map(|p| p.as_path()));

    if hits.is_empty() {
        return Ok(true);
    }

    format::warn(format!("{} possible secret(s) in what you're about to ship:", hits.len()));
    for hit in &hits {
        format::bullet(format!("{}:{}  [{}]  {}", hit.path.display(), hit.line, hit.rule, hit.redacted));
    }

    if yes {
        format::info("  continuing anyway (--yes) - double check this isn't a real leak");
        return Ok(true);
    }

    Ok(Confirm::new("Ship anyway?").with_default(false).prompt().unwrap_or(false))
}
