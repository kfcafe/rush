---
id: '3'
title: 'chore: Fix all compiler warnings (214 warnings from cargo build)'
slug: chore-fix-all-compiler-warnings-214-warnings-from
status: closed
priority: 2
created_at: '2026-02-19T08:23:42.877232Z'
updated_at: '2026-03-02T03:35:15.895771Z'
notes: |-
  ---
  2026-02-19T10:22:53.020093+00:00
  Superseded or completed by other beans
closed_at: '2026-03-02T03:35:15.895771Z'
verify: test $(cd /Users/asher/rush && cargo build 2>&1 | grep -c 'warning:') -lt 10
is_archived: true
tokens: 3000
tokens_updated: '2026-02-19T10:22:53.024506Z'
history:
- attempt: 1
  started_at: '2026-03-02T03:35:15.896201Z'
  finished_at: '2026-03-02T03:35:16.054332Z'
  duration_secs: 0.158
  result: pass
  exit_code: 0
---

## Context
Rush currently generates 214 compiler warnings on `cargo build`. Most are unused imports, dead code, and elided lifetime suggestions. This clutters output and hides real issues.

## Task
1. Run `cargo build 2>&1 | grep 'warning:'` to see all warnings
2. Fix warnings by category:
   - **Unused imports**: Remove them (13+ instances across daemon, executor, compat modules)
   - **Dead code / never constructed variants**: Add `#[allow(dead_code)]` where the code is intentionally future-proofed, or remove truly dead code
   - **Elided lifetime confusion** in src/main.rs RushPrompt impl: Change `Cow<str>` to `Cow<'_, str>` (5 instances around lines 384-405)
   - **Unused variables**: Prefix with `_` or remove
   - **Clippy: stripping prefix manually**: Use `strip_prefix()` instead of `starts_with()` + manual slicing
   - **Clippy: unnecessary reference creation**: Remove unnecessary `&` on already-borrowed values
3. Do NOT change any logic or behavior — only suppress or fix warnings
4. Ensure `cargo build` and `cargo test --lib --no-run` still succeed

## Files
- src/main.rs (lifetime annotations in RushPrompt, unused variable `e` line ~159, unused `dim`)
- src/daemon/mod.rs (unused imports: PoolConfig, PoolStats, WorkerPool, etc.)
- src/daemon/server.rs (unused imports)
- src/daemon/protocol.rs (unused imports)
- src/executor/mod.rs (unused imports: Pid, ErrorFormatter, etc.)
- src/compat/mod.rs (unused import)
- src/config/mod.rs (unused imports)
- src/builtins/exec.rs (dead_code allows)
- src/builtins/mod.rs (dead_code)
- src/progress/mod.rs (dead_code)
- src/completion/mod.rs (dead_code)

## Approach
Run `cargo build 2>&1 | grep 'warning:' | sort | uniq -c | sort -rn` to prioritize.
Use `cargo fix --lib -p rush --allow-dirty` for auto-fixable warnings first, then handle the rest manually.
