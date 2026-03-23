---
id: '4'
title: 'chore: Fix compiler warnings — unused imports and dead code'
slug: chore-fix-compiler-warnings-unused-imports-and-dea
status: closed
priority: 2
created_at: '2026-02-19T08:23:57.438688Z'
updated_at: '2026-03-02T02:26:42.806847Z'
notes: |-
  ---
  2026-02-19T10:22:53.036377+00:00
  Superseded or completed by other beans
closed_at: '2026-03-02T02:26:42.806847Z'
verify: test $(cd /Users/asher/rush && cargo build 2>&1 | grep -c 'warning:') -lt 10
is_archived: true
tokens: 81131
tokens_updated: '2026-02-19T10:22:53.039497Z'
history:
- attempt: 1
  started_at: '2026-03-02T02:26:42.810704Z'
  finished_at: '2026-03-02T02:26:43.298940Z'
  duration_secs: 0.488
  result: pass
  exit_code: 0
---

## Context
Rush generates 214 compiler warnings. Fix them without changing behavior.

## Task
1. Run `cargo fix --lib -p rush --allow-dirty` to auto-fix what it can
2. Then manually fix remaining warnings:
   - Remove unused imports in src/daemon/mod.rs, src/executor/mod.rs, src/compat/mod.rs, src/config/mod.rs
   - Change `Cow<str>` to `Cow<'_, str>` in src/main.rs RushPrompt impl (lines ~384-405)
   - Prefix unused variables with `_` (e.g. `e` on line ~159 of main.rs)
   - Add `#[allow(dead_code)]` for intentionally-reserved variants/fields
3. Do NOT change logic — only fix warnings
4. `cargo build` must succeed with <10 warnings

## Key files
- src/main.rs
- src/daemon/mod.rs, src/daemon/server.rs, src/daemon/protocol.rs
- src/executor/mod.rs
- src/compat/mod.rs
- src/config/mod.rs
- src/builtins/exec.rs, src/builtins/mod.rs
- src/progress/mod.rs, src/completion/mod.rs
