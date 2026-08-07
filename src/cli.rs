use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

pub const EXAMPLES: &str = "\
EXAMPLES:
    rooter why 3000                    what's using port 3000
    rooter why 48291                   explain PID 48291
    rooter why ~/Downloads/file.zip    explain a file
    rooter why \"my disk is full\"       disk usage + biggest offenders
    rooter where node                  find a binary, process, file, or port
    rooter kill 3000                   free up a port (asks first)
    rooter ports                       everything currently listening
    rooter ship                        stage, commit, push, guided
    rooter envcheck                    check this project's toolchain/deps
    rooter doctor                      full health report on this folder
    rooter clean                       reclaim space from node_modules, target, ...
    rooter secrets                     scan for leaked API keys/tokens
    rooter stash                       grab the clipboard, auto-classified
    rooter stash grep docker           search what you've stashed
    rooter gen uuid                    quick generators: uuid/password/hash/base64/jwt
    rooter serve                       serve the current directory over HTTP
    rooter net github.com              DNS + reachability check
    rooter ask \"why is my build slow\"  ask a local LLM (Ollama, LM Studio, ...)
    rooter ship --ai                   AI-generated commit message from the diff
    rooter config                      show/set the local LLM endpoint

Run `rooter <command> --help` for a command's own flags, or just `rooter`
with no arguments for an arrow-key menu.";

#[derive(Parser)]
#[command(
    name = "rooter",
    version,
    about = "A CLI toolbelt for developers",
    long_about = "rooter explains your system, ships your code, and keeps your dev environment honest.\nRun with no arguments for an interactive menu.",
    after_help = EXAMPLES
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Explain a PID, port, file path, or plain-English question
    Why {
        /// e.g. `rooter why 3000`, `rooter why ~/Downloads/file.zip`, `rooter why "my disk is full"`
        query: Vec<String>,
    },

    /// Find anything: binaries in PATH, running processes, files, or ports
    Where {
        query: Vec<String>,
    },

    /// Stage, commit, and push your work with a guided flow
    Ship {
        /// Commit message (skips the prompt)
        #[arg(short, long)]
        message: Option<String>,

        /// Skip all confirmation prompts (implies committing everything staged/unstaged)
        #[arg(short, long)]
        yes: bool,

        /// Don't push after committing
        #[arg(long)]
        no_push: bool,

        /// Open a PR after pushing (requires the `gh` CLI)
        #[arg(long)]
        pr: bool,

        /// Generate the commit message from the staged diff using a local LLM
        #[arg(long)]
        ai: bool,
    },

    /// Check the current environment for broken or missing dependencies
    Envcheck,

    /// Full health report on the current folder
    Doctor,

    /// A developer-oriented clipboard stash, with auto-classification
    Stash {
        #[command(subcommand)]
        action: Option<StashCommand>,
    },

    /// Kill a process by PID, port, or name
    Kill {
        /// A PID, a port number, or part of a process name
        target: String,
        /// Skip the confirmation prompt
        #[arg(short, long)]
        force: bool,
    },

    /// List every listening/bound port, with its owning process
    Ports {
        /// Filter by port number or process name
        filter: Option<String>,
    },

    /// Find and remove build artifacts and caches (node_modules, target, dist, ...)
    Clean {
        /// Select everything found without prompting
        #[arg(short, long)]
        yes: bool,
        /// Skip the final confirmation too (implies --yes for scripting)
        #[arg(long)]
        force: bool,
    },

    /// Scan for likely leaked secrets (API keys, tokens, private keys)
    Secrets {
        /// Directory to scan (defaults to the current directory)
        path: Option<String>,
    },

    /// Quick generators: uuid, password, hash, base64, jwt decode
    Gen {
        #[command(subcommand)]
        action: GenCommand,
    },

    /// Serve the current directory (or a given one) over HTTP
    Serve {
        /// Port to listen on (default 8080)
        port: Option<u16>,
        /// Directory to serve (default: current directory)
        #[arg(short, long)]
        dir: Option<PathBuf>,
        /// Open the URL in your browser
        #[arg(short, long)]
        open: bool,
    },

    /// Basic network diagnostics: local IP, DNS resolution, reachability
    Net {
        /// A host, or host:port (defaults to checking 443 then 80)
        target: Option<String>,
    },

    /// Ask a local LLM a question (Ollama, LM Studio, llama.cpp server, ...)
    Ask {
        query: Vec<String>,
    },

    /// View or set the local LLM endpoint used by `ask` and `ship --ai`
    Config {
        /// e.g. http://localhost:11434/v1 (Ollama) or http://localhost:1234/v1 (LM Studio)
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        model: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum GenCommand {
    /// Generate one or more UUIDv4s
    Uuid {
        #[arg(short, long, default_value_t = 1)]
        count: u32,
    },
    /// Generate a random password
    Password {
        /// Password length
        #[arg(default_value_t = 20)]
        length: usize,
        /// Letters and digits only, no symbols
        #[arg(long)]
        simple: bool,
    },
    /// Hash text or a file
    Hash {
        text: Option<String>,
        #[arg(short, long)]
        file: Option<PathBuf>,
        #[arg(short, long, default_value = "sha256")]
        algo: String,
    },
    /// Base64 encode/decode text or a file
    Base64 {
        mode: Base64Mode,
        text: Option<String>,
        #[arg(short, long)]
        file: Option<PathBuf>,
    },
    /// Decode a JWT's header and payload (no signature verification)
    Jwt { token: String },
}

#[derive(Clone, ValueEnum)]
pub enum Base64Mode {
    Encode,
    Decode,
}

#[derive(Subcommand)]
pub enum StashCommand {
    /// Force-classify the clipboard contents as code
    Code,
    /// Force-classify the clipboard contents as a URL
    Url,
    /// Force-classify the clipboard contents as JSON
    Json,
    /// Force-classify the clipboard contents as a password/secret
    Password,
    /// List everything currently stashed
    List,
    /// Print (and optionally remove) a stashed entry, most recent first
    Pop {
        /// Index to pop (see `rooter stash list`), defaults to the most recent
        index: Option<usize>,
        /// Copy back to the clipboard instead of just printing
        #[arg(short, long)]
        copy: bool,
        /// Keep the entry in the stash instead of removing it
        #[arg(short, long)]
        keep: bool,
    },
    /// Search stashed contents
    Grep { term: String },
    /// Clear the entire stash
    Clear,
}
