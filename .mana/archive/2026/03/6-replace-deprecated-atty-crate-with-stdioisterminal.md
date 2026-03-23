---
id: '6'
title: Replace deprecated atty crate with std::io::IsTerminal
slug: replace-deprecated-atty-crate-with-stdioisterminal
status: closed
priority: 2
created_at: '2026-02-19T08:24:55.322450Z'
updated_at: '2026-03-02T02:26:43.335126Z'
notes: |-
  ---
  2026-02-19T10:22:53.060821+00:00
  Superseded or completed by other beans
closed_at: '2026-03-02T02:26:43.335126Z'
verify: cd /Users/asher/rush && ! grep -q '^atty' Cargo.toml && cargo build 2>&1 | tail -1 | grep -q Finished
is_archived: true
tokens: 50689
tokens_updated: '2026-02-19T10:22:53.063424Z'
history:
- attempt: 1
  started_at: '2026-03-02T02:26:43.335361Z'
  finished_at: '2026-03-02T02:26:43.497053Z'
  duration_secs: 0.161
  result: pass
  exit_code: 0
---

Replace all `atty::is(atty::Stream::X)` calls with `std::io::X().is_terminal()` (from `std::io::IsTerminal`). Remove atty from Cargo.toml. Files: Cargo.toml, src/main.rs (lines 152,419,462), src/executor/mod.rs (line 948), src/value/render.rs (line 20), src/executor/value/render.rs (line 28). Add `use std::io::IsTerminal;` where needed.
