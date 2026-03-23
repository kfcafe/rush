---
id: '8'
title: 'refactor: Extract loop break/continue signal handling into a shared helper'
slug: refactor-extract-loop-breakcontinue-signal-handlin
status: closed
priority: 2
created_at: '2026-02-19T08:27:24.828727Z'
updated_at: '2026-03-02T03:53:28.178921Z'
notes: ''
closed_at: '2026-03-02T03:53:28.178921Z'
is_archived: true
---

2026-02-19T10:22:53.077014+00:00
  Superseded or completed by other beans
  ## Attempt 1 — 2026-03-02T03:53:15Z
  Exit code: 1

  ```

  ```
verify: 'cd /Users/asher/rush && cargo test --lib 2>&1 | tail -1 | grep -q ''test result: ok'' && grep -q ''fn execute_loop_body'' src/executor/mod.rs'
attempts: 1
tokens: 2000
tokens_updated: '2026-02-19T10:22:53.079151Z'
history:
- attempt: 1
  started_at: '2026-03-02T03:53:14.083265Z'
  finished_at: '2026-03-02T03:53:15.154991Z'
  duration_secs: 1.071
  result: fail
  exit_code: 1
---

The break/continue signal handling code is duplicated ~50 lines across execute_for_loop, execute_while_loop, and execute_until_loop in src/executor/mod.rs. Extract a shared helper method `execute_loop_body` that handles BreakSignal/ContinueSignal propagation. The helper should take the body statements, accumulated output buffers, and return a LoopControl enum (Continue, Break, Error). Keep all existing behavior identical.
