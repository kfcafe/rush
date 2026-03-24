# Rush

[![CI](https://github.com/opus-workshop/rush/actions/workflows/integration-tests.yml/badge.svg)](https://github.com/opus-workshop/rush/actions/workflows/integration-tests.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

**The AI-native shell with structured pipelines.**

Rush is a POSIX-compatible shell built in Rust with three things your current shell doesn't have: a real AI agent at the prompt, typed structured pipelines, and Lua extensions for anything else.

---

## What makes it different

**Structured pipelines** — builtins produce typed data, not text. Pipe into `| where`, `| select`, `| sort`, `| count` without touching `awk` or `jq`.

**AI agent built in** — prefix any query with `?` and Rush sends it to the LLM, shows you the command, and asks to run it. Works with Ollama (local), OpenAI, or Anthropic. No wrapper scripts.

**Lua extensions** — register custom builtins, prompt segments, completions, and shell hooks in `~/.rush/lua/`. No recompile.

Oh, and it's still a shell. Your existing scripts run unchanged.

---

## Quick examples

```bash
# Ask the AI to write a command — it generates, you confirm
? find all Rust files changed in the last week

# Structured pipeline: filter git status without awk
git status --json | where status == "modified" | select path

# Count TODO comments across the codebase
grep --json 'TODO' src/**/*.rs | count

# Register a custom builtin in Lua
# ~/.rush/lua/weather.lua
rush.register_builtin("weather", {
    description = "Current weather",
    run = function(args)
        local city = args[1] or "London"
        local data = rush.exec_structured("fetch https://wttr.in/" .. city .. "?format=j1")
        return { text = data.current_condition[1].temp_C .. "°C in " .. city }
    end
})
```

---

## Installation

### Cargo

```bash
cargo install --git https://github.com/opus-workshop/rush
```

### Build from source

```bash
git clone https://github.com/opus-workshop/rush.git
cd rush
cargo install --path .
```

**Requirements:** Rust 1.70+.

---

## AI setup

On first `?` use, Rush runs an interactive wizard. Or set it up manually:

```toml
# ~/.rush/ai.toml
provider = "ollama"           # ollama | openai | anthropic
model = "qwen2.5-coder:7b"
```

**Ollama** (local, private — recommended):
```bash
ollama pull qwen2.5-coder:7b
# That's it. Rush finds Ollama at localhost:11434 automatically.
```

**OpenAI:**
```toml
provider = "openai"
model = "gpt-4o"
# Set OPENAI_API_KEY in your environment
```

**Anthropic:**
```toml
provider = "anthropic"
model = "claude-3-5-sonnet-20241022"
# Set ANTHROPIC_API_KEY in your environment
```

### How the agent works

The `?` prefix sends your natural language query to the LLM with shell context (cwd, project type, recent history). The model returns a command with an explanation. Rush shows you both and asks to run, edit, or cancel. Destructive commands always require explicit confirmation.

```bash
? find files over 100MB and list them by size
# Suggests: find . -type f -size +100M | xargs ls -lh | sort -k5 -rh
# [r]un  [e]dit  [c]ancel?
```

---

## Lua extensions

Scripts in `~/.rush/lua/` load at startup in alphabetical order.

```lua
-- ~/.rush/lua/myconfig.lua

-- Custom builtin
rush.register_builtin("greet", {
    description = "Say hello",
    run = function(args)
        return { text = "Hello, " .. (args[1] or "world") }
    end
})

-- Prompt segment
rush.register_prompt("git_branch", function()
    local branch = rush.exec("git rev-parse --abbrev-ref HEAD 2>/dev/null")
    if branch ~= "" then
        return " " .. branch
    end
end)

-- Shell hooks
rush.on("precmd", function(exit_code, elapsed_ms)
    -- fires before every prompt draw
end)

-- Custom completion
rush.register_completion("deploy", function(args)
    return { "staging", "production", "preview" }
end)
```

The full API surface: `rush.exec()`, `rush.exec_structured()`, `rush.json_parse()`, `rush.json_encode()`, `rush.env.get/set()`, `rush.cwd()`.

---

## Structured pipelines

All Rush builtins emit typed `Value` objects — not text. The pipeline operators work on that data directly.

```bash
# Filter: keep rows where field matches value
ls --json | where type == "file"

# Select: keep only named columns
git log --json | select hash message author

# Sort: order by field
find --json . -name "*.rs" | sort size --reverse

# Count: number of rows
grep --json 'TODO' src/**/*.rs | count

# Chaining
find --json . -name "*.rs" | where size > 10000 | sort size | select path size
```

Text output from external commands is coerced into a single-column table (`{line: "..."}`) so operators work on anything.

### Built-in structured commands

| Command | Output |
|---------|--------|
| `ls --json` | File list with name, size, type, modified |
| `git status --json` | Staged, unstaged, untracked, branch info |
| `git log --json` | Commits with hash, author, date, message |
| `git diff --json` | Hunks with additions, deletions, context |
| `grep --json` | Matches with file, line number, context |
| `find --json` | Paths with size, type, permissions |
| `fetch --json` | HTTP response with status, headers, body |

---

## Designed for AI coding agents

Rush is optimized for AI assistants that make hundreds of shell calls per task.

```python
import subprocess, json

def rush(cmd: str):
    result = subprocess.run(
        ["rush", "-c", cmd],
        capture_output=True, text=True,
        env={"RUSH_ERROR_FORMAT": "json"}
    )
    return json.loads(result.stdout)

# Structured data, no text parsing
todos  = rush("grep --json 'TODO|FIXME' src/**/*.rs")
status = rush("git status --json")
staged = [f["path"] for f in status["staged"]]
```

Errors come back as typed JSON (`CommandNotFound`, `GitError`, `NetworkError`, etc.) so agents can handle them programmatically instead of parsing stderr.

For workloads with many rapid calls, the daemon mode cuts startup to **0.4ms**:

```bash
rushd start          # keep Rush warm in the background
rush -c "ls"         # 0.4ms instead of 4.9ms cold start
rushd stop
```

See [docs/AI_AGENT_GUIDE.md](docs/AI_AGENT_GUIDE.md) for the full integration guide including JSON schemas, error types, and Python/batch examples.

---

## POSIX compatibility

Rush targets 90%+ POSIX.1-2017 compliance. Your scripts work.

- **Control flow**: `if`/`elif`/`else`, `while`, `until`, `for`, `case`, functions
- **Job control**: background jobs, `fg`/`bg`, job specs (`%1`, `%+`, `%-`), process groups
- **Redirections**: `>`, `>>`, `<`, `2>&1`, here-docs (`<<EOF`), arbitrary FD redirection
- **Expansions**: variables, `$(...)`, `$((...))`, globbing, brace expansion
- **Signals**: `trap`, SIGCHLD, SIGTSTP/SIGCONT, SIGTTIN/SIGTTOU
- **Special vars**: `$$`, `$!`, `$?`, `$-`, `$_`, `$0`, `$@`, `$*`, `$#`, `$IFS`
- **50+ builtins**: `cd`, `pwd`, `echo`, `export`, `source`, `eval`, `exec`, `test`, `[`, `printf`, `read`, `trap`, `alias`, `jobs`, `fg`, `bg`, `kill`, `wait`, and more

```bash
#!/usr/bin/env rush

# This is valid POSIX sh — rush runs it fine
for file in $(find . -name "*.rs"); do
    if grep -q "TODO" "$file"; then
        echo "Found TODO in: $file"
    fi
done
```

---

## Performance

Rush builtins skip fork/exec entirely. Commands are native Rust, not subprocess calls.

| Operation | Bash/Zsh | Rush | Speedup |
|-----------|----------|------|---------|
| `ls` (1000 files) | 12–15ms | **0.1ms** | 120x |
| `grep` pattern | 42–45ms | **0.2ms** | 212x |
| `cat` small file | 8–9ms | **0.02ms** | 427x |
| Cold startup | 2.5–12ms | **4.9ms** | — |
| Daemon startup | — | **0.4ms** | — |

For AI agent workloads (git status × 3, file search × 5, JSON ops × 10, HTTP × 2): roughly **2–5s in Rush vs 10–20s in bash + external tools**.

---

## Architecture

```
rush/
├── src/
│   ├── ai/           # LLM client, agent loop, provider adapters (Ollama/OpenAI/Anthropic)
│   ├── lua/          # Lua 5.4 runtime (mlua), rush.* API, script loader
│   ├── lexer/        # Token stream (Logos)
│   ├── parser/       # AST (nom)
│   ├── executor/     # Command execution + structured_ops (where/select/sort/count)
│   ├── value/        # Typed Value system (String, Int, List, Table, Path, ...)
│   ├── builtins/     # 80+ native Rust commands
│   ├── daemon/       # Client-server fast startup
│   ├── intent/       # ? prefix: natural language → shell command
│   ├── runtime/      # Variable scoping, environment
│   ├── signal.rs     # POSIX signal handling
│   └── jobs/         # Job control
├── tests/
│   ├── posix/        # POSIX compliance suite
│   └── *.rs          # 52 integration test files
├── benches/          # Criterion benchmarks
├── examples/         # 12 example scripts
└── docs/             # 65+ documentation files
```

---

## Testing

```bash
cargo test
cargo test --test posix_compliance_tests
cargo test --test pipeline_tests
cargo bench
```

52 test files including a POSIX compliance suite.

---

## Documentation

- [AI Agent Integration Guide](docs/AI_AGENT_GUIDE.md)
- [POSIX Compliance Report](tests/posix/COMPLIANCE_REPORT.md)
- [Daemon Architecture](docs/daemon-architecture.md)
- [Performance Guide](docs/PERFORMANCE.md)
- [Builtin Reference](docs/builtins/)

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Help wanted on POSIX edge cases, platform support (BSD, WSL), and documentation.

## License

Dual-licensed under MIT or Apache-2.0 (your choice).

## Acknowledgments

Built on: [logos](https://github.com/maciejhirsz/logos), [nom](https://github.com/rust-bakery/nom), [reedline](https://github.com/nushell/reedline), [git2](https://github.com/rust-lang/git2-rs), [mlua](https://github.com/khvzalenko/mlua), [grep-*](https://github.com/BurntSushi/ripgrep)

---

**~68,000 lines of Rust** · **80+ builtins** · **Ollama/OpenAI/Anthropic** · **Lua extensions**
