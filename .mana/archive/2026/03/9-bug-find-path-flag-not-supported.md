---
id: '9'
title: 'bug: find -path flag not supported'
slug: bug-find-path-flag-not-supported
status: closed
priority: 2
created_at: '2026-03-24T02:39:11.964693Z'
updated_at: '2026-03-24T17:35:08.492215Z'
notes: |2

  ## Attempt 1 — 2026-03-24T16:35:06Z
  Exit code: 1

  ```

  ```

  ## Attempt 2 — 2026-03-24T16:35:29Z
  Exit code: 1

  ```

  ```

  ## Attempt 3 — 2026-03-24T17:28:24Z
  Exit code: 1

  ```

  ```

  ## Attempt 4 — 2026-03-24T17:28:41Z
  Exit code: 1

  ```

  ```
labels:
- bug
- find
closed_at: '2026-03-24T17:35:08.492215Z'
verify: ~/bin/rush -c 'find /tmp -name "*.txt" -path "*tmp*" -maxdepth 1' 2>&1 | grep -v "unknown flag"
attempts: 4
claimed_by: pi-agent
claimed_at: '2026-03-24T17:28:47.635483Z'
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-24T16:35:06.899324Z'
  finished_at: '2026-03-24T16:35:06.950806Z'
  duration_secs: 0.051
  result: fail
  exit_code: 1
- attempt: 2
  started_at: '2026-03-24T16:35:29.901164Z'
  finished_at: '2026-03-24T16:35:29.955626Z'
  duration_secs: 0.054
  result: fail
  exit_code: 1
- attempt: 3
  started_at: '2026-03-24T17:28:24.545499Z'
  finished_at: '2026-03-24T17:28:24.652811Z'
  duration_secs: 0.107
  result: fail
  exit_code: 1
- attempt: 4
  started_at: '2026-03-24T17:28:41.258970Z'
  finished_at: '2026-03-24T17:28:41.313272Z'
  duration_secs: 0.054
  result: fail
  exit_code: 1
- attempt: 5
  started_at: '2026-03-24T17:35:08.404906Z'
  finished_at: '2026-03-24T17:35:08.465531Z'
  duration_secs: 0.06
  result: pass
  exit_code: 0
outputs:
  text: |-
    /tmp/rush-find-a.txt
    /tmp/rush-grep-combined-test.txt
    /tmp/rush-redir-1.txt
    /tmp/rush-redir-2.txt
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-24T17:28:47.635483Z'
  finished_at: '2026-03-24T17:35:08.492215Z'
---

## Bug

`find -path` is not recognized by rush's built-in find command.

### Repro

```sh
~/bin/rush -c 'find ~ -name "agent.rs" -path "*/imp-core/*"'
# Expected: finds matching files
# Actual: rush: find: unknown flag: -path
```

### Impact

`-path` is commonly used by agents and scripts to filter results by directory path. This is a standard POSIX find feature.

Discovered via imp bash tool dogfooding.
