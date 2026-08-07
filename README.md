# rooter

A CLI toolbelt for developers. Explains your system, ships your code, and keeps
your dev environment honest.

```
rooter why 3000
rooter why 48291
rooter why ~/Downloads/file.zip
rooter why "my disk is full"
```

## Commands

### `rooter why <query>`
Explains a PID, a port, a file path, or a plain-English question.
- **Number** — checked as both a PID (full process detail: parent, status,
  CPU/memory, cmdline, cwd, listening ports, lifetime disk I/O) and a port
  (who's bound to it, TCP/UDP, listen state).
- **Path** — size, type, timestamps, read-only flag, best-guess category, and
  (on Windows) whether the file looks locked by another process.
- **Free text** — recognizes phrases about disk space, CPU, memory, and ports,
  and gives you the relevant report (e.g. `rooter why "my disk is full"` shows
  disk usage per volume plus the largest items in your current directory).

### `rooter where <query>`
Finds anything: binaries in `PATH`, running processes by name, files under the
current directory, or (if you pass a number) the process/port that owns it.

### `rooter ship`
A guided git flow: pick which changes to stage, write (or auto-generate) a
commit message, commit, push, and optionally open a PR via the `gh` CLI. Runs
a `secrets` scan on staged files first and asks before shipping if it finds
something. Flags for scripting: `--message`, `--yes`, `--no-push`, `--pr`,
`--ai` (write the commit message from the staged diff via a local LLM).

### `rooter envcheck`
Detects the project type(s) in the current folder (Node, Rust, Python, Go,
Ruby, Java, Docker) and checks that the required tools are installed, that
dependencies are actually installed (`node_modules`, virtualenv, etc.), and
diffs `.env` against `.env.example`/`.env.sample`/`.env.template`.

### `rooter doctor`
A full health report on the current folder: project type, git status (branch,
dirty files, ahead/behind upstream), total size and largest top-level items,
file-type breakdown, files over 10MB, and TODO/FIXME/HACK markers.

### `rooter stash`
A developer-oriented clipboard, stored locally and auto-classified:

```
rooter stash             # capture clipboard, auto-detect code/url/json/password/text
rooter stash code        # force-classify as code
rooter stash url
rooter stash json
rooter stash password    # masked in listings
rooter stash list
rooter stash pop         # print (and remove) the most recent entry
rooter stash pop 2 --copy --keep
rooter stash grep docker
rooter stash clear
```

### `rooter kill <pid|port|name>`
Kills a process by PID, by port (kills whatever's bound to it), or by a
substring of its name. Lists what it found and asks for confirmation unless
you pass `--force`.

### `rooter ports [filter]`
Every listening/bound TCP and UDP socket, with the owning process resolved.
Optionally filtered by port number or process name.

### `rooter clean`
Finds build artifacts and caches under the current directory (`node_modules`,
`target`, `dist`, `build`, `.next`, `__pycache__`, `.turbo`, `coverage`, ...),
shows how much space each one takes, and lets you pick what to delete.
`--yes` selects everything found; `--force` also skips the final confirmation
(for scripting/CI).

### `rooter secrets [path]`
Scans for likely leaked credentials - AWS keys, GitHub/Slack tokens, Stripe
live keys, Google API keys, private key blocks, and generic
`secret=`/`token=`/`password=` assignments. Output is redacted (first 4 /
last 2 characters only). Exits non-zero if anything is found, so it's usable
as a CI gate. `ship` also runs this automatically against staged files before
committing and will ask before shipping if it finds something.

### `rooter gen`
```
rooter gen uuid [-c N]
rooter gen password [length] [--simple]
rooter gen hash <text> [--file path] [--algo sha256|md5]
rooter gen base64 encode|decode <text> [--file path]
rooter gen jwt <token>          # decodes header + payload, shows expiry; no signature check
```

### `rooter serve [port] [--dir path] [--open]`
An instant static file server for the current directory (or `--dir`),
defaulting to port 8080. Serves `index.html` automatically, falls back to a
directory listing, and refuses to serve anything outside the served
directory.

### `rooter net [host[:port]]`
With no arguments, shows your local outbound IP. With a host, resolves DNS
and checks TCP reachability (443 then 80 if no port is given), reporting
latency.

### `rooter ask <question>`
Asks a local LLM a question, streamed straight to your terminal. Works with
anything that speaks the OpenAI-compatible `/v1/chat/completions` dialect -
[Ollama](https://ollama.com) (`ollama serve`, zero-config default), LM
Studio, llama.cpp's server, etc. Fails fast with a clear message if nothing's
listening, rather than hanging or dumping a raw connection error.

### `rooter config [--url <url>] [--model <model>]`
Views or sets the local LLM endpoint used by `ask` and `ship --ai`. Defaults
to Ollama on `http://localhost:11434/v1` with model `llama3.2`. Also
configurable via `ROOTER_LLM_URL` / `ROOTER_LLM_MODEL` env vars (these take
priority over the saved config). Settings are saved to your OS config dir
(e.g. `%APPDATA%\rooter\config.json` on Windows).

### No arguments
Running `rooter` with no subcommand launches an arrow-key menu covering every
command above (built on `inquire`, so it needs a real terminal — it won't run
inside a non-interactive shell/CI).

## Building

Requires Rust (this project defaults to the GNU toolchain on Windows, so it
doesn't need the MSVC Build Tools — a MinGW-w64 linker such as
[WinLibs](https://winlibs.com) is enough):

```
cargo build --release
```

The binary lands at `target/release/rooter(.exe)`. Put it on your `PATH` to
use `rooter` from anywhere.
