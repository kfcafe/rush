---
id: '13'
title: 'bug: while loops with test brackets [ ] not supported'
slug: bug-while-loops-with-test-brackets-not-supported
status: closed
priority: 1
created_at: '2026-03-24T03:11:23.609547Z'
updated_at: '2026-03-24T17:28:24.847733Z'
notes: |2-

  ## Attempt 1 — 2026-03-24T16:35:07Z
  Exit code: 1

  ```

  ```

  ## Attempt 2 — 2026-03-24T16:35:30Z
  Exit code: 1

  ```

  ```


  ---
  2026-03-24T16:41:55.171793+00:00
  ## Attempt 2 Failed (5m50s, 343.0k tokens, $0.351)

  ### What was tried

  - 0 tool calls over 10 turns in 5m50s

  ### Why it failed

  - Idle timeout (5m)

  ### Verify command

  `~/bin/rush -c 'i=0; while [ $i -lt 3 ]; do echo $i; i=$((i+1)); done' 2>&1 | head -1 | grep 0`

  ### Suggestion for next attempt

  - Agent went idle — it may be stuck in a loop or waiting for input. Try a more focused prompt or break the task into smaller steps.

  ---
  2026-03-24T17:13:40.480868+00:00
  ## Attempt 2 Failed (11m30s, 675.6k tokens, $0.498)

  ### What was tried

  - 0 tool calls over 16 turns in 11m30s

  ### Why it failed

  - Idle timeout (10m)

  ### Verify command

  `~/bin/rush -c 'i=0; while [ $i -lt 3 ]; do echo $i; i=$((i+1)); done' 2>&1 | head -1 | grep 0`

  ### Suggestion for next attempt

  - Agent went idle — it may be stuck in a loop or waiting for input. Try a more focused prompt or break the task into smaller steps.
labels:
- bug
- parser
- while
closed_at: '2026-03-24T17:28:24.847733Z'
close_reason: verify passed (tidy sweep)
verify: ~/bin/rush -c 'i=0; while [ $i -lt 3 ]; do echo $i; i=$((i+1)); done' 2>&1 | head -1 | grep 0
attempts: 2
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-24T16:35:07.298532Z'
  finished_at: '2026-03-24T16:35:07.350661Z'
  duration_secs: 0.052
  result: fail
  exit_code: 1
- attempt: 2
  started_at: '2026-03-24T16:35:30.313589Z'
  finished_at: '2026-03-24T16:35:30.364627Z'
  duration_secs: 0.051
  result: fail
  exit_code: 1
- attempt: 3
  started_at: '2026-03-24T17:28:24.750633Z'
  finished_at: '2026-03-24T17:28:24.806947Z'
  duration_secs: 0.056
  result: pass
  exit_code: 0
outputs: 0
attempt_log:
- num: 1
  outcome: abandoned
  agent: pi-agent
  started_at: '2026-03-24T16:36:05.083507Z'
  finished_at: '2026-03-24T16:41:55.153916Z'
- num: 2
  outcome: abandoned
  agent: pi-agent
  started_at: '2026-03-24T17:02:10.140197Z'
  finished_at: '2026-03-24T17:13:40.455555Z'
---

## Bug

`while [ ... ]` (test bracket syntax) is not recognized by rush's parser.

### Repro

```sh
~/bin/rush -c 'i=1; while [ $i -le 3 ]; do echo $i; i=$((i+1)); done'
# Expected: 1 2 3
# Actual: rush: Invalid token at position 11: '[ $i -le 3 '
```

### Notes

`while` with `[` (test command) is standard POSIX shell. The `[` is actually the `test` builtin. Rush may not recognize `[` as a command name, or may not support `while` at all.

Discovered via imp bash tool dogfooding.
