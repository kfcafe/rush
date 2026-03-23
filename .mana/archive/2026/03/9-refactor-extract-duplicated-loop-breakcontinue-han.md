---
id: '9'
title: 'refactor: Extract duplicated loop break/continue handling into shared helper'
slug: refactor-extract-duplicated-loop-breakcontinue-han
status: closed
priority: 2
created_at: '2026-02-19T08:32:32.639669Z'
updated_at: '2026-03-02T02:26:46.624993Z'
notes: ''
closed_at: '2026-03-02T02:26:46.624993Z'
is_archived: true
---

2026-02-19T10:22:53.095336+00:00
  Superseded or completed by other beans
  ## Attempt 1 — 2026-03-02T02:26:43Z
  Exit code: 1

  ```

  ```
verify: cd /Users/asher/rush && grep -q 'fn execute_loop_body' src/executor/mod.rs && cargo build 2>&1 | tail -1 | grep -q Finished
attempts: 1
tokens: 36742
tokens_updated: '2026-02-19T10:22:53.097366Z'
history:
- attempt: 1
  started_at: '2026-03-02T02:26:43.519984Z'
  finished_at: '2026-03-02T02:26:43.575602Z'
  duration_secs: 0.055
  result: fail
  exit_code: 1
---

In src/executor/mod.rs, the break/continue BreakSignal/ContinueSignal handling is copy-pasted ~50 lines across execute_for_loop, execute_while_loop, and execute_until_loop. Extract a shared `execute_loop_body(&mut self, body: &[Statement], stdout: &mut String, stderr: &mut String, exit_code: &mut i32) -> Result<LoopControl>` helper. LoopControl enum: Continue, BreakLoop, PropagateBreak(levels), PropagateContinue(levels), Error. Keep behavior identical.
