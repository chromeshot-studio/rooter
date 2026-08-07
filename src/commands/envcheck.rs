use crate::util::format;
use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

pub fn run() -> Result<()> {
    let cwd = std::env::current_dir()?;
    format::heading(&format!("Environment check for {}", cwd.display()));

    let mut any_project = false;

    if cwd.join("package.json").exists() {
        any_project = true;
        check_node(&cwd);
    }
    if cwd.join("Cargo.toml").exists() {
        any_project = true;
        check_rust();
    }
    if cwd.join("requirements.txt").exists() || cwd.join("pyproject.toml").exists() {
        any_project = true;
        check_python(&cwd);
    }
    if cwd.join("go.mod").exists() {
        any_project = true;
        check_go();
    }
    if cwd.join("Gemfile").exists() {
        any_project = true;
        check_ruby();
    }
    if cwd.join("pom.xml").exists() || cwd.join("build.gradle").exists() || cwd.join("build.gradle.kts").exists() {
        any_project = true;
        check_java();
    }
    if cwd.join("Dockerfile").exists() || cwd.join("docker-compose.yml").exists() || cwd.join("compose.yaml").exists() {
        check_docker();
    }

    check_git();
    check_env_files(&cwd);

    if !any_project {
        format::subheading("Project type");
        format::warn("No recognized project manifest found (package.json, Cargo.toml, requirements.txt, go.mod, ...)");
    }

    Ok(())
}

fn tool_version(cmd: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(cmd).args(args).output().ok()?;
    if !output.status.success() && output.stdout.is_empty() && output.stderr.is_empty() {
        return None;
    }
    let text = if !output.stdout.is_empty() {
        String::from_utf8_lossy(&output.stdout)
    } else {
        String::from_utf8_lossy(&output.stderr)
    };
    text.lines().next().map(|l| l.trim().to_string())
}

fn check_node(cwd: &Path) {
    format::subheading("Node.js");
    match tool_version("node", &["--version"]) {
        Some(v) => format::ok(format!("node {v}")),
        None => {
            format::fail("node is not installed or not on PATH");
            return;
        }
    }
    match tool_version("npm", &["--version"]) {
        Some(v) => format::ok(format!("npm {v}")),
        None => format::warn("npm not found on PATH"),
    }

    let has_lock_yarn = cwd.join("yarn.lock").exists();
    let has_lock_pnpm = cwd.join("pnpm-lock.yaml").exists();
    let has_lock_npm = cwd.join("package-lock.json").exists();
    let manager = if has_lock_pnpm {
        "pnpm"
    } else if has_lock_yarn {
        "yarn"
    } else {
        "npm"
    };

    if !cwd.join("node_modules").exists() {
        format::warn(format!("node_modules is missing - run `{manager} install`"));
    } else {
        format::ok("node_modules present");
    }

    if has_lock_npm && has_lock_yarn {
        format::warn("both package-lock.json and yarn.lock exist - pick one package manager");
    }
}

fn check_rust() {
    format::subheading("Rust");
    match tool_version("rustc", &["--version"]) {
        Some(v) => format::ok(v),
        None => {
            format::fail("rustc is not installed or not on PATH");
            return;
        }
    }
    match tool_version("cargo", &["--version"]) {
        Some(v) => format::ok(v),
        None => format::warn("cargo not found on PATH"),
    }
}

fn check_python(cwd: &Path) {
    format::subheading("Python");
    let python = tool_version("python", &["--version"]).or_else(|| tool_version("python3", &["--version"]));
    match python {
        Some(v) => format::ok(v),
        None => {
            format::fail("python is not installed or not on PATH");
            return;
        }
    }
    let venvs = [".venv", "venv", "env"];
    if venvs.iter().any(|v| cwd.join(v).exists()) {
        format::ok("virtual environment found");
    } else {
        format::warn("no .venv/venv found - dependencies may be installed globally");
    }
}

fn check_go() {
    format::subheading("Go");
    match tool_version("go", &["version"]) {
        Some(v) => format::ok(v),
        None => format::fail("go is not installed or not on PATH"),
    }
}

fn check_ruby() {
    format::subheading("Ruby");
    match tool_version("ruby", &["--version"]) {
        Some(v) => format::ok(v),
        None => format::fail("ruby is not installed or not on PATH"),
    }
    match tool_version("bundle", &["--version"]) {
        Some(v) => format::ok(v),
        None => format::warn("bundler not found on PATH"),
    }
}

fn check_java() {
    format::subheading("Java");
    match tool_version("java", &["-version"]) {
        Some(v) => format::ok(v),
        None => format::fail("java is not installed or not on PATH"),
    }
}

fn check_docker() {
    format::subheading("Docker");
    match tool_version("docker", &["--version"]) {
        Some(v) => format::ok(v),
        None => {
            format::fail("docker is not installed or not on PATH");
            return;
        }
    }
    match Command::new("docker").args(["info"]).output() {
        Ok(o) if o.status.success() => format::ok("docker daemon is running"),
        _ => format::warn("docker is installed but the daemon doesn't seem to be running"),
    }
}

fn check_git() {
    format::subheading("Git");
    match tool_version("git", &["--version"]) {
        Some(v) => format::ok(v),
        None => format::fail("git is not installed or not on PATH"),
    }
}

fn check_env_files(cwd: &Path) {
    let example = [".env.example", ".env.sample", ".env.template"]
        .iter()
        .map(|f| cwd.join(f))
        .find(|p| p.exists());

    let Some(example) = example else { return };
    format::subheading("Environment variables");

    let env_path = cwd.join(".env");
    let example_keys = parse_env_keys(&example);

    if !env_path.exists() {
        format::fail(format!(".env is missing but {} exists", example.file_name().unwrap().to_string_lossy()));
        for key in &example_keys {
            format::bullet(format!("missing: {key}"));
        }
        return;
    }

    let actual_keys = parse_env_keys(&env_path);
    let missing: Vec<&String> = example_keys.difference(&actual_keys).collect();

    if missing.is_empty() {
        format::ok(".env has every key from the example file");
    } else {
        format::warn(format!(".env is missing {} key(s) present in the example file", missing.len()));
        for key in missing {
            format::bullet(format!("missing: {key}"));
        }
    }
}

fn parse_env_keys(path: &Path) -> HashSet<String> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return HashSet::new();
    };
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            line.split('=').next().map(|k| k.trim().to_string())
        })
        .collect()
}
