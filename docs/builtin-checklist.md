# Rush Builtin Compatibility Checklist

Status of builtins vs POSIX.1-2017, common bash/zsh expectations, modern shell
features (fish/nushell/zsh), and Rush's "fast coreutils" differentiator.

**Legend:** ✅ Implemented · ⚠️ Code exists but not wired · ❌ Missing · 🔧 Partial

**Sources:**
- [Fish shell commands](https://fishshell.com/docs/current/commands.html)
- [Fish `string` builtin](https://fishshell.com/docs/current/cmds/string.html)
- [Nushell command reference](https://www.nushell.sh/commands/)
- [Fish 4.0 Rust port blog](https://fishshell.com/blog/rustport/)

---

## 1. POSIX Special Builtins

These are *required* by POSIX.1-2017 §2.14 to be built into the shell. Errors in
special builtins cause the shell to exit in strict mode.

| Builtin     | Status | Priority | Notes |
|-------------|--------|----------|-------|
| `break`     | ✅     | —        | |
| `:`         | ✅     | —        | (colon / no-op) |
| `continue`  | ✅     | —        | |
| `.` / `source` | ✅  | —        | |
| `eval`      | ✅     | —        | |
| `exec`      | ✅     | —        | Supports FD redirections |
| `exit`      | ✅     | —        | Subshell-aware via ExitSignal |
| `export`    | ✅     | —        | |
| `readonly`  | ✅     | —        | |
| `return`    | ✅     | —        | |
| `set`       | ✅     | —        | `-e`, `-u`, `-x`, `-o pipefail` |
| `shift`     | ✅     | —        | |
| `times`     | ❌     | P2       | Print accumulated user/sys times for shell + children. Simple `libc::times()` wrapper. |
| `trap`      | ✅     | —        | EXIT, ERR, signal names + numbers |
| `unset`     | ✅     | —        | |

**Score: 14/15** — only `times` missing (low impact).

---

## 2. POSIX Regular Builtins

Required by POSIX to be available as builtins (can't work correctly as externals
because they modify shell state or need shell internals).

| Builtin     | Status | Priority | Notes |
|-------------|--------|----------|-------|
| `alias`     | ✅     | —        | |
| `unalias`   | ✅     | —        | |
| `bg`        | ✅     | —        | |
| `fg`        | ✅     | —        | |
| `jobs`      | ✅     | —        | `-l`, `-r`, `-s` flags |
| `cd`        | ✅     | —        | `~`, `-`, `..` |
| `command`   | ✅     | —        | Verify: does `command -v` work? |
| `false`     | ✅     | —        | |
| `true`      | ✅     | —        | |
| `getopts`   | ✅     | —        | |
| `kill`      | ✅     | —        | Signal names, numbers, `-l`, job specs |
| `read`      | ✅     | —        | `-r`, `-s`, `-t`, `-p`, IFS splitting |
| `type`      | ✅     | —        | |
| `wait`      | ✅     | —        | |
| `fc`        | ❌     | P2       | List/edit/re-execute history entries. Needs `$EDITOR` integration. Interactive convenience — rarely used in scripts. |
| `hash`      | ❌     | P3       | Cache command path lookups. Rush may not need this if it already caches `$PATH` lookups internally. |
| `umask`     | ❌     | **P0**   | **Cannot be external.** Must modify the shell process's file creation mask. Scripts that set `umask 077` before creating files will silently fail without this. |
| `ulimit`    | ❌     | **P1**   | **Cannot be external.** Must call `setrlimit` on the shell process. Scripts that set resource limits (e.g. `ulimit -n 4096`) will silently fail. |
| `newgrp`    | ❌     | P3       | Change effective group ID. Rarely used in practice. |

**Score: 14/19** — `umask` is the critical gap.

---

## 3. Common Interactive / Bash-ism Builtins

Not required by POSIX, but expected by users coming from bash/zsh. Prioritized
by how often they appear in real dotfiles and scripts.

| Builtin         | Status | Priority | Notes |
|-----------------|--------|----------|-------|
| `history`       | ⚠️     | **P1**   | **Code exists** (`src/builtins/history.rs`) but not compiled or registered in `mod.rs`. Just needs wiring. |
| `pushd`         | ❌     | P1       | Runtime already has `push_dir`/`pop_dir` (`src/runtime/mod.rs:427`). Just needs the builtin wrapper. |
| `popd`          | ❌     | P1       | Same — runtime support exists. |
| `dirs`          | ❌     | P2       | Same — `get_dir_stack()` exists. |
| `declare` / `typeset` | ❌ | P2    | Variable attributes (`-i`, `-r`, `-x`, `-a`, `-A`). Commonly used in bash scripts. |
| `let`           | ❌     | P3       | Arithmetic evaluation. Rush already has `$(( ))` — `let` is syntactic sugar. |
| `shopt`         | ❌     | P2       | Bash-specific shell options (e.g. `globstar`, `nullglob`). Important for bash script compat. |
| `bind`          | ❌     | P3       | Readline/key bindings. Reedline handles this differently. |
| `complete` / `compgen` | ❌ | P2  | Programmable completion API. Needed for user-defined completions. |
| `disown`        | ❌     | P2       | Remove job from job table (keep running after shell exit). |
| `suspend`       | ❌     | P3       | Suspend the shell itself. Rarely used. |
| `enable`        | ❌     | P3       | Enable/disable builtins. Useful for plugin system later. |
| `logout`        | ❌     | P3       | Exit login shell. Could alias to `exit`. |
| `mapfile` / `readarray` | ❌ | P2 | Read lines into an array. Common in bash scripts that process files. |
| `select`        | ❌     | P3       | Interactive menu loop. Niche but part of bash. |
| `coproc`        | ❌     | P3       | Coprocess (bidirectional pipe). Rarely used. |

---

## 4. Fast Coreutils (Rush's Differentiator)

Commands Rush implements as in-process builtins for speed (no fork/exec).
This is Rush's unique value prop — the more of these it has, the faster
AI agent and CI/CD workloads become.

### Currently implemented

| Command     | Status | Notes |
|-------------|--------|-------|
| `ls`        | ✅     | Parallel dir reads, color, `-lahR` |
| `cat`       | ✅     | Memory-mapped I/O, binary detection |
| `find`      | ✅     | Parallel traversal, `.gitignore`-aware |
| `grep`      | ✅     | Ripgrep internals, regex |
| `mkdir`     | ✅     | `-p` support |
| `rm`        | ✅     | Undo-tracked |

### High-value additions

These are called *extremely* frequently in scripts and AI agent workflows.
Making them builtins eliminates thousands of fork/exec calls per session.

| Command     | Priority | Complexity | Why |
|-------------|----------|------------|-----|
| `touch`     | **P0**   | Low        | Trivially simple (`File::create` or `utime`). Called constantly in build scripts. |
| `cp`        | **P1**   | Medium     | Very common. `fs::copy` + recursive + preserve mode. |
| `mv`        | **P1**   | Medium     | Very common. `fs::rename` + cross-device fallback. Undo-trackable. |
| `head`      | **P1**   | Low        | Pipeline staple. Just read N lines/bytes. |
| `tail`      | **P1**   | Low-Med    | Pipeline staple. `-f` (follow) is medium complexity. |
| `wc`        | **P1**   | Low        | Pipeline staple. Lines/words/bytes/chars. |
| `basename`  | P2       | Trivial    | Very common in scripts. Pure string op. |
| `dirname`   | P2       | Trivial    | Very common in scripts. Pure string op. |
| `realpath`  | P2       | Trivial    | `fs::canonicalize`. Common in scripts. |
| `sort`      | P2       | Medium     | Pipeline staple. Rust's sort is already fast. |
| `uniq`      | P2       | Low        | Pipeline companion to `sort`. |
| `tee`       | P2       | Low        | Duplicate stdin to file + stdout. Common in pipelines. |
| `chmod`     | P2       | Low        | `fs::set_permissions`. Scripts need this. |
| `chown`     | P2       | Low        | `nix::unistd::chown`. Less common but useful. |
| `cut`       | P2       | Low        | Field extraction. Common in scripts. |
| `tr`        | P2       | Low-Med    | Character translation. Common in scripts. |
| `sleep`     | P2       | Trivial    | `thread::sleep`. Common in scripts + loops. |
| `date`      | P2       | Low        | Rush already depends on `chrono`. |
| `ln`        | ✅ P3    | Low        | Symlinks/hardlinks. `std::os::unix::fs`. |
| `stat`      | ✅ P3    | Low        | File metadata. `fs::metadata`. |
| `readlink`  | ✅ P3    | Trivial    | `fs::read_link`. |
| `mktemp`    | ✅ P3    | Low        | Already have `tempfile` in dev-deps. |
| `diff`      | P3       | High       | Complex. Might not be worth internalizing. |
| `sed`       | P3       | High       | Very complex to implement fully. External is fine. |
| `xargs`     | P3       | Medium     | Useful but complex edge cases. |
| `du`        | ✅ P3    | Medium     | Recursive size. WalkDir-based. |
| `env`       | ✅ P3    | Low        | Print/modify environment for a command. |

---

## 5. Rush-Specific Builtins

| Command         | Status | Priority | Notes |
|-----------------|--------|----------|-------|
| `undo`          | ✅     | —        | Unique to Rush |
| `profile`       | ✅     | —        | Command profiling |
| `time`          | ✅     | —        | Timing wrapper |
| `help`          | ✅     | —        | |
| `json_get`      | ✅     | —        | JSON path extraction |
| `json_set`      | ✅     | —        | JSON modification |
| `json_query`    | ✅     | —        | jq-like queries |
| `fetch`         | ✅     | —        | HTTP client (optional feature) |
| `git status`    | ✅     | —        | Native git2 bindings |
| `git log`       | ✅     | —        | Native git2 bindings |
| `git diff`      | ⚠️     | P1       | **Code exists** (`src/builtins/git_diff.rs`) but commented out in `mod.rs` due to compilation errors. |
| `git add`       | ❌     | P2       | Would complete the "basic git without forking" story |
| `git commit`    | ❌     | P2       | Same |
| `git branch`    | ❌     | P2       | Same |
| `git checkout`  | ❌     | P3       | More complex |

---

## Priority Summary

### P0 — Fix now (script compatibility / quick wins)
1. **`umask`** — POSIX required, can't be external, scripts break silently
2. **`touch`** — Trivial to implement, called everywhere
3. **`history`** — Already written, just needs `mod history;` + `m.insert()`

### P1 — Soon (common usage, moderate effort)
4. **`ulimit`** — POSIX, can't be external, CI scripts use this
5. **`git diff`** — Already written, just needs compilation fix
6. **`pushd`/`popd`** — Runtime support exists, just needs builtin wrappers
7. **`cp`** / **`mv`** — Very high frequency, undo-trackable
8. **`head`** / **`tail`** / **`wc`** — Pipeline staples, low effort

### P2 — When you get to it (interactive polish + script compat)
9. `dirs`, `disown`, `declare`, `shopt`, `complete`/`compgen`
10. `basename`/`dirname`/`realpath`, `sort`, `uniq`, `tee`, `cut`, `chmod`
11. `fc`, `times`, `mapfile`, `date`, `sleep`
12. `git add`/`commit`/`branch`

### P3 — Low priority (niche / complex / fine as externals)
13. `hash`, `newgrp`, `let`, `bind`, `suspend`, `enable`, `select`, `coproc`
14. `ln`, `stat`, `readlink`, `mktemp`, `diff`, `sed`, `xargs`, `du`, `env`

---
---

# Part 2: Modern Shell Feature Comparison

How Rush stacks up against fish, nushell, and zsh — the three shells most
likely to poach Rush's potential users. Organized by capability area, not
by individual command.

---

## 6. String Manipulation

The biggest single gap vs fish. Fish's `string` builtin is a Swiss army knife
that replaces `sed`, `tr`, `cut`, `awk`, and `grep` for 90% of common cases —
all without forking a process.

| Feature | Fish | Nushell | Zsh | Rush |
|---------|------|---------|-----|------|
| split / join | `string split`, `string join` | `str join`, `split row/column/chars/words` | `${(s:.:)var}` parameter expansion | ❌ |
| match (glob + regex) | `string match -r` | `str contains`, `str starts-with`, `str ends-with` | `[[ =~ ]]`, `${var:#pattern}` | ❌ (only `grep` or `test`) |
| replace | `string replace -r` | `str replace` | `${var/pat/rep}` | ❌ (only external `sed`) |
| trim | `string trim` | `str trim` | `${var## }`, `${var%% }` | ❌ |
| upper / lower | `string upper`, `string lower` | `str upcase`, `str downcase` | `${(U)var}`, `${(L)var}` | ❌ |
| substring | `string sub -s 2 -l 5` | `str substring` | `${var:offset:length}` | ❌ |
| length | `string length` | `str length` | `${#var}` | ❌ |
| pad / repeat | `string pad`, `string repeat` | (via `fill`) | `${(l:20:)var}` | ❌ |
| escape / unescape | `string escape --style=url` | `url encode/decode` | — | ❌ |
| case conversion | — | `str camel-case`, `str kebab-case`, `str snake-case`, `str pascal-case`, `str title-case` | — | ❌ |
| collect (no word-split) | `string collect` | `collect` | — | ❌ |

**Recommendation:** Implement a `string` builtin (fish-style) with subcommands.
This is probably the single highest-impact feature for interactive + scripting
use. It eliminates most reasons to reach for `sed`/`awk`/`tr`/`cut` and is
a huge win for the "fast builtins, no forking" story.

**Priority: P1** — high impact, medium effort.

---

## 7. Math / Arithmetic

| Feature | Fish | Nushell | Zsh | Rush |
|---------|------|---------|-----|------|
| Basic arithmetic | `math "1 + 2"` | `1 + 2` (native) | `$(( ))`, `(( ))` | ✅ `$(( ))` |
| Floating point | `math "3.14 * 2"` | Native floats | `$(( 3.14 * 2 ))` | ❌ (integer only) |
| Math functions | `math "sin(pi)"`, `ceil`, `floor`, `round` | `math avg`, `math sum`, `math max`, etc. | — | ❌ |
| Random number | `random` / `random choice` | `random int`, `random float`, `random dice` | `$RANDOM` | ❌ |

**Recommendation:** Add `math` builtin (float support + common functions) and
`$RANDOM` variable. The `math` builtin is low effort and high convenience.

**Priority: P2** — nice to have, low effort.

---

## 8. Path Manipulation

Fish added a `path` builtin; nushell has a full `path` category. These
eliminate the need for `basename`, `dirname`, `realpath` as separate commands.

| Feature | Fish | Nushell | Zsh | Rush |
|---------|------|---------|-----|------|
| basename | `path basename` | `path basename` | `${var:t}` | ❌ |
| dirname | `path dirname` | `path dirname` | `${var:h}` | ❌ |
| extension | `path extension` | `path parse` (gives stem + ext) | `${var:e}` | ❌ |
| stem (no ext) | `path change-extension ''` | `path parse` | `${var:r}` | ❌ |
| normalize | `path normalize` | `path expand` | `${var:A}` | ❌ |
| resolve (realpath) | `path resolve` | `path expand` | `realpath` | ❌ |
| join | `path join` | `path join` | — | ❌ |
| is-file / is-dir | `path is -f`, `path is -d` | — (use `test`) | `test -f`, `test -d` | ✅ `test -f/-d` |

**Recommendation:** Implement a `path` builtin with subcommands. Trivial to
implement (pure string ops + `fs::` calls), eliminates 3 external commands,
and feels very modern.

**Priority: P2** — low effort, nice ergonomics.

---

## 9. Structured Data / Tables

This is nushell's core differentiator. Fish and zsh are string-based. Rush
already has `json_get`/`json_set`/`json_query` + `--json` output mode, which
is a good start, but nushell goes much further.

| Feature | Fish | Nushell | Rush |
|---------|------|---------|------|
| JSON parsing | — (external `jq`) | Native: `open file.json`, `from json` | ✅ `json_get`, `json_set`, `json_query` |
| JSON output from builtins | — | All commands output structured data | ✅ `--json` flag on builtins |
| Table display | — | Rich table rendering with colors | ❌ |
| Filter/select/where | — | `where size > 10kb`, `select name size` | ❌ |
| Sort/group/aggregate | — | `sort-by`, `group-by`, `math sum` | ❌ |
| CSV / YAML / TOML | — | `from csv`, `from yaml`, `from toml` | ❌ |
| Parallel iteration | — | `par-each` | ❌ |

**Recommendation:** Rush is already ahead of fish here with `json_*` builtins.
Next steps: (1) add `--json` to more builtins, (2) consider a `table` display
mode for JSON arrays, (3) add `from csv`/`from yaml` for common formats.
Don't try to become nushell — lean into JSON as Rush's structured data format.

**Priority: P2** — extend what you have rather than building a type system.

---

## 10. Interactive UX

This is where fish absolutely dominates. These features are what make people
*switch* to a shell.

| Feature | Fish | Nushell | Zsh | Rush |
|---------|------|---------|-----|------|
| Autosuggestions (ghost text) | ✅ Built-in | ✅ Built-in | ⚠️ Plugin (zsh-autosuggestions) | ❌ |
| Syntax highlighting (live) | ✅ Built-in | ✅ Built-in | ⚠️ Plugin (zsh-syntax-highlighting) | ❌ |
| Rich completions (descriptions) | ✅ Built-in | ✅ Built-in | ✅ Built-in | 🔧 Basic tab completion |
| Abbreviations (expand inline) | ✅ `abbr` | ❌ | ⚠️ Plugin | ❌ |
| Web-based config | ✅ `fish_config` | ❌ | ❌ | ❌ |
| `commandline` manipulation | ✅ `commandline` | ✅ `commandline` | ✅ `zle` | ❌ |
| Directory history (prev/next) | ✅ `prevd`/`nextd`/`dirh` | ❌ | ⚠️ (with hooks) | ❌ |
| Right prompt | ✅ `fish_right_prompt` | ✅ | ✅ `RPROMPT` | ❌ |
| Transient prompt | ✅ | ✅ | ⚠️ Plugin | ❌ |
| Fuzzy history (Ctrl+R) | ✅ Built-in | ✅ Built-in | ⚠️ fzf plugin | 🔧 (history fuzzy search exists) |

**Recommendation:** These are the features that make or break daily-driver
adoption. Top priorities for human users:
1. **Autosuggestions** (ghost text from history) — the #1 reason people switch to fish
2. **Syntax highlighting** (red = bad command, green = valid) — the #2 reason
3. **Right prompt** (git branch, exec time) — table stakes for modern shells

These are reedline features more than builtins, but they belong on the roadmap.

**Priority: P1** for autosuggestions + syntax highlighting. Everything else P2.

---

## 11. Event System / Hooks

Fish has a rich event system that lets functions react to signals, variable
changes, job exits, and custom events. This enables things like auto-updating
prompts, cleanup on exit, and plugin coordination.

| Feature | Fish | Nushell | Zsh | Rush |
|---------|------|---------|-----|------|
| `--on-event` (custom events) | ✅ `emit`/`function --on-event` | ❌ | ❌ | ❌ |
| `--on-variable` | ✅ `function --on-variable` | ❌ | ❌ | ❌ |
| `--on-signal` | ✅ `function --on-signal` | ❌ | ✅ `TRAPINT()` etc | ✅ `trap` |
| `--on-job-exit` | ✅ `function --on-job-exit` | ❌ | ❌ | ❌ |
| `precmd` / `preexec` hooks | ✅ (via events) | ✅ `$env.PROMPT_COMMAND` | ✅ `precmd`/`preexec` | ❌ |
| `block` (defer events) | ✅ `block` | ❌ | ❌ | ❌ |

**Recommendation:** Start with `precmd`/`preexec` hooks (needed for prompt
themes, timing display, etc.). Full event system is P3.

**Priority: P2** for hooks, P3 for full event system.

---

## 12. Function Management

Fish treats functions as first-class citizens with autoloading, introspection,
and interactive editing.

| Feature | Fish | Nushell | Zsh | Rush |
|---------|------|---------|-----|------|
| Autoload from `~/.config/fish/functions/` | ✅ | ❌ (uses modules) | ✅ `$fpath` | ❌ |
| `functions` (list/inspect/erase) | ✅ `functions -n`, `functions -e` | ✅ `scope commands` | ✅ `functions`/`whence` | ❌ |
| `funced` (edit function live) | ✅ | ❌ | ✅ `zed` | ❌ |
| `funcsave` (persist to file) | ✅ | ❌ | ❌ | ❌ |
| `argparse` (structured arg parsing for functions) | ✅ | ✅ (via signatures) | ❌ (use `zparseopts`) | ❌ |

**Recommendation:** Autoloading functions from a config directory is the
key feature here. It's how fish's 960+ completion scripts work without
slowing down startup.

**Priority: P2** — autoloading is important for the completion/plugin story.

---

## 13. Scoping & Variables

| Feature | Fish | Nushell | Zsh | Rush |
|---------|------|---------|-----|------|
| Local scope (`-l`) | ✅ | ✅ `let` | ✅ `local` | ✅ `local` |
| Global scope (`-g`) | ✅ | ✅ `$env` | ✅ | ✅ `export` |
| Universal variables (`-U`) | ✅ (persist across all sessions) | ❌ | ❌ | ❌ |
| `contains` (test list membership) | ✅ `contains` | ✅ (`in` operator) | ✅ `(( ${+array[$key]} ))` | ❌ |
| `count` (array length) | ✅ `count` | ✅ `length` | ✅ `${#array}` | ❌ |
| `status` (query shell state) | ✅ `status is-interactive`, `status is-login`, etc. | ✅ `is-admin`, `$nu` | ✅ `[[ -o interactive ]]` | ❌ |
| `set` with `--show` | ✅ `set --show` | ✅ `scope variables` | ✅ `typeset` | ❌ |

**Recommendation:** `status` (query if interactive, login, etc.) and `contains`
are easy wins. Universal variables are a fish-unique feature that's very cool
but complex to implement (requires a persistence daemon or file-watching).

**Priority: P2** for `status`/`contains`/`count`, P3 for universal variables.

---

## Updated Priority Summary (including modern shell features)

### P0 — Fix now
1. `umask` — POSIX, scripts break silently
2. `touch` — trivial, called everywhere
3. `history` — already written, just wire it up

### P1 — High impact
4. `ulimit` — POSIX, CI scripts need this
5. `git diff` — already written, fix compilation
6. `pushd`/`popd` — runtime support exists
7. **`string` builtin** — single biggest gap vs fish. Replaces sed/awk/tr/cut
8. **Autosuggestions** — #1 reason people switch to fish
9. **Syntax highlighting** — #2 reason people switch to fish
10. `cp`/`mv`, `head`/`tail`/`wc` — fast coreutils expansion

### P2 — Meaningful improvements
11. `path` builtin — trivial, modern ergonomics
12. `math` builtin — float support, common functions, `$RANDOM`
13. Right prompt, `precmd`/`preexec` hooks
14. `status` builtin (is-interactive, is-login, etc.)
15. Function autoloading from config dir
16. Table display for JSON output
17. `contains`, `count` builtins
18. `dirs`, `disown`, `complete`/`compgen`, `declare`
19. `from csv`/`from yaml` format converters
20. Abbreviations (`abbr`)

### P3 — Future / niche
21. Universal variables
22. Full event system (`emit`, `--on-event`, `--on-variable`)
23. `funced`/`funcsave`
24. Web-based config (`fish_config`-style)
25. `hash`, `fc`, `times`, `newgrp`
