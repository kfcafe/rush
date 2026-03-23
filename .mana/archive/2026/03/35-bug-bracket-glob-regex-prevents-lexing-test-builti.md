---
id: '35'
title: 'bug: Bracket glob regex prevents lexing [ test builtin and standalone [abc] patterns'
slug: bug-bracket-glob-regex-prevents-lexing-test-builti
status: closed
priority: 2
created_at: '2026-02-19T18:44:02.145191Z'
updated_at: '2026-03-02T02:34:31.958008Z'
closed_at: '2026-03-02T02:34:31.958008Z'
verify: cd /Users/asher/rush && target/debug/rush -c "[ 1 -eq 1 ] && echo pass" 2>&1 | grep -q "pass"
fail_first: true
checkpoint: '061f5956fc5b614060d3a14408d92eff84e51863'
claimed_by: pi-agent
claimed_at: '2026-03-02T02:31:41.244243Z'
is_archived: true
tokens: 7578
tokens_updated: '2026-02-19T18:44:02.146371Z'
history:
- attempt: 1
  started_at: '2026-03-02T02:34:31.959473Z'
  finished_at: '2026-03-02T02:34:32.016382Z'
  duration_secs: 0.056
  result: pass
  exit_code: 0
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-02T02:31:41.244243Z'
  finished_at: '2026-03-02T02:34:31.958008Z'
---

The bracket glob regex in the lexer prevents tokenizing:
1. `[ 1 -eq 1 ]` (test builtin alternate syntax) — FAILS
2. `echo [abc]` (standalone bracket expression) — FAILS

Both produce: Invalid token error.

Root cause in src/lexer/mod.rs:
The second GlobPattern regex `r"[a-zA-Z0-9_.\-/]*\[[^\]]+\][a-zA-Z0-9_.*?\-/]+"` greedily matches `[^\]]+` which consumes everything (including spaces) until the last `]`, then fails because the required suffix `[a-zA-Z0-9_.*?\-/]+` is not present. Logos cannot fall back to LeftBracket after this partial match attempt.

Fix: The bracket glob regex needs to be redesigned so it does not consume content that should tokenize as LeftBracket. Options:
1. Require the bracket pattern to be preceded by word characters (not start at word boundary with `[`)
2. Change `[^\]]+` to `[^\]\s]+` to prevent matching across spaces
3. Remove the standalone bracket glob regex and handle bracket globs only when combined with star/question patterns

Also add tests:
- `Lexer::tokenize("[abc]")` should produce LeftBracket, Identifier, RightBracket
- `Lexer::tokenize("[ 1 -eq 1 ]")` should produce tokens for the test builtin
