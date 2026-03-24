---
id: '10'
title: 'bug: grep -q (quiet mode) not supported'
slug: bug-grep-q-quiet-mode-not-supported
status: in_progress
priority: 1
created_at: '2026-03-24T02:40:45.241087Z'
updated_at: '2026-03-24T16:36:05.079051Z'
notes: |2

  ## Attempt 1 — 2026-03-24T16:35:07Z
  Exit code: 1

  ```
  grep: Unknown option: -q
  ```

  ## Attempt 2 — 2026-03-24T16:35:30Z
  Exit code: 1

  ```
  grep: Unknown option: -q
  ```
labels:
- bug
- grep
verify: echo "hello" | ~/bin/rush -c 'grep -q hello' && echo pass
attempts: 2
claimed_by: pi-agent
claimed_at: '2026-03-24T16:36:05.079051Z'
history:
- attempt: 1
  started_at: '2026-03-24T16:35:06.998009Z'
  finished_at: '2026-03-24T16:35:07.051927Z'
  duration_secs: 0.053
  result: fail
  exit_code: 1
  output_snippet: 'grep: Unknown option: -q'
- attempt: 2
  started_at: '2026-03-24T16:35:29.998761Z'
  finished_at: '2026-03-24T16:35:30.051634Z'
  duration_secs: 0.052
  result: fail
  exit_code: 1
  output_snippet: 'grep: Unknown option: -q'
attempt_log:
- num: 1
  outcome: abandoned
  agent: pi-agent
  started_at: '2026-03-24T16:36:05.079051Z'
---

## Bug

`grep -q` is not recognized by rush's built-in grep command.

### Repro

```sh
~/bin/rush -c 'echo hello | grep -q hello'
# Expected: exits 0 silently
# Actual: rush: Unknown option: -q
```

### Impact

`grep -q` is extremely common in shell scripts and verify commands — it's used to test if a pattern matches without producing output. This blocks many verify gates in mana units and general scripting.

`-q` / `--quiet` / `--silent` should suppress output and return exit code 0 on match, 1 on no match.

Discovered via imp headless dogfooding — agent tried to verify a file's content with `grep -q` and got an error.
