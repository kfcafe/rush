---
id: '5'
title: 'chore: Replace deprecated atty crate with std::io::IsTerminal'
slug: chore-replace-deprecated-atty-crate-with-stdioiste
status: closed
priority: 2
created_at: '2026-02-19T08:24:33.545532Z'
updated_at: '2026-03-02T02:30:56.541116Z'
notes: |-
  ---
  2026-02-19T10:22:53.050159+00:00
  Superseded or completed by other beans
closed_at: '2026-03-02T02:30:56.541116Z'
verify: cd /Users/asher/rush && ! grep -q 'atty' Cargo.toml && cargo build 2>&1 | tail -1 | grep -q 'Finished'
is_archived: true
tokens: 47330
tokens_updated: '2026-02-19T10:22:53.052549Z'
history:
- attempt: 1
  started_at: '2026-03-02T02:30:56.541738Z'
  finished_at: '2026-03-02T02:30:56.706726Z'
  duration_secs: 0.164
  result: pass
  exit_code: 0
---

## Context
The `atty` crate is deprecated. Since Rust 1.70, `std::io::IsTerminal` is available in the standard library.

## Task
1. Replace all uses of `atty::is(atty::Stream::Stdin)` with `std::io::stdin().is_terminal()`
2. Replace all uses of `atty::is(atty::Stream::Stdout)` with `std::io::stdout().is_terminal()`
3. Replace all uses of `atty::is(atty::Stream::Stderr)` with `std::io::stderr().is_terminal()`
4. Remove `atty` from Cargo.toml dependencies
5. Add `use std::io::IsTerminal;` where needed

## Files
- Cargo.toml (remove atty dep)
- src/main.rs (uses atty::is in run_interactive, run_interactive_with_init, fast path)
- src/executor/mod.rs (uses atty::is for should_inherit_io check)
- Any other files using atty (search with: grep -rn 'atty' src/)
