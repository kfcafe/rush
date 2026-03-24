---
id: '9'
title: 'bug: find -path flag not supported'
slug: bug-find-path-flag-not-supported
status: open
priority: 2
created_at: '2026-03-24T02:39:11.964693Z'
updated_at: '2026-03-24T16:35:29.955635Z'
notes: |2

  ## Attempt 1 — 2026-03-24T16:35:06Z
  Exit code: 1

  ```

  ```

  ## Attempt 2 — 2026-03-24T16:35:29Z
  Exit code: 1

  ```

  ```
labels:
- bug
- find
verify: ~/bin/rush -c 'find /tmp -name "*.txt" -path "*tmp*" -maxdepth 1' 2>&1 | grep -v "unknown flag"
attempts: 2
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
