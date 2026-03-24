---
id: '8'
title: 'bug: exit N always returns code 1, custom exit codes not propagated'
slug: bug-exit-n-always-returns-code-1-custom-exit-codes
status: closed
priority: 1
created_at: '2026-03-24T00:04:16.480299Z'
updated_at: '2026-03-24T00:46:42.013224Z'
notes: |-
  ## Attempt 1 Failed (0s, 0 tokens, $0.000)

  ### What was tried

  - 0 tool calls over 0 turns in 0s

  ### Why it failed

  - Exit code 1

  ### Verify command

  `~/bin/rush -c 'exit 42' ; test $? -eq 42`

  ### Suggestion for next attempt

  - Agent exited with an error. Check the verify command output and ensure the approach is correct before retrying.

  ---
  2026-03-24T00:45:30.975936+00:00
  ## Attempt Failed (30m2s, 3.0M tokens, $1.256)

  ### What was tried

  - 0 tool calls over 63 turns in 30m2s

  ### Why it failed

  - Timeout (30m)

  ### Verify command

  `~/bin/rush -c 'exit 42' ; test $? -eq 42`

  ### Suggestion for next attempt

  - Agent ran out of time. Consider increasing the timeout or simplifying the task scope.

  ---
  2026-03-24T00:46:41.820207+00:00
  Bug is already fixed. exit 42 returns code 42, exit 2 returns code 2, exit 0 returns code 0. Verify gate passes. Likely fixed in commit 8d0d0d3.
labels:
- bug
- exit-codes
closed_at: '2026-03-24T00:46:42.013224Z'
verify: ~/bin/rush -c 'exit 42' ; test $? -eq 42
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-24T00:46:41.851516Z'
  finished_at: '2026-03-24T00:46:41.947444Z'
  duration_secs: 0.095
  result: pass
  exit_code: 0
attempt_log:
- num: 1
  outcome: abandoned
  agent: pi-agent
  started_at: '2026-03-24T00:15:28.997996Z'
  finished_at: '2026-03-24T00:45:30.937828Z'
---

## Bug

`exit N` where N != 0 always returns exit code 1 instead of the requested code.

### Repro

```sh
~/bin/rush -c "exit 42"; echo $?
# Expected: 42
# Actual: 1

~/bin/rush -c "exit 0"; echo $?
# This works: 0

~/bin/rush -c "exit 2"; echo $?
# Expected: 2
# Actual: 1
```

### Impact

This breaks any tool/script that relies on specific exit codes:
- `grep -q` returns 1 (not found) vs other codes
- Build tools return specific codes for different failure types
- Agent harnesses (imp) need correct exit codes to interpret tool results
- `exit 42` in tests to verify exit code propagation

### Context

Discovered while testing imp's bash tool with rush as the shell backend. Had to revert rush auto-detection because exit code 42 came back as 1, breaking test assertions. Rush needs to propagate the integer argument to `exit` as the process exit code.

### Related

Also noticed `ls *.rs | wc -l` gives different counts than sh (32 vs 11) — likely a glob expansion issue. That's a separate bug but worth noting.
