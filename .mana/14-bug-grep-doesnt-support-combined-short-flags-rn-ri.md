---
id: '14'
title: 'bug: grep doesn''t support combined short flags (-rn, -ri, etc.)'
slug: bug-grep-doesnt-support-combined-short-flags-rn-ri
status: open
priority: 1
created_at: '2026-03-24T03:27:55.196254Z'
updated_at: '2026-03-24T16:35:30.465263Z'
notes: |2

  ## Attempt 1 — 2026-03-24T16:35:07Z
  Exit code: 1

  ```

  ```

  ## Attempt 2 — 2026-03-24T16:35:30Z
  Exit code: 1

  ```

  ```
labels:
- bug
- grep
- flags
verify: echo "hello" > /tmp/rush-grep-combined-test.txt && ~/bin/rush -c 'grep -in HELLO /tmp/rush-grep-combined-test.txt' 2>&1 | grep hello
attempts: 2
history:
- attempt: 1
  started_at: '2026-03-24T16:35:07.395210Z'
  finished_at: '2026-03-24T16:35:07.446525Z'
  duration_secs: 0.051
  result: fail
  exit_code: 1
- attempt: 2
  started_at: '2026-03-24T16:35:30.411369Z'
  finished_at: '2026-03-24T16:35:30.465254Z'
  duration_secs: 0.053
  result: fail
  exit_code: 1
---

## Bug

Combined short flags like `-rn` are treated as a single unknown option instead of being split into `-r -n`.

### Repro

```sh
~/bin/rush -c 'grep -rn hello /tmp/test.txt'
# Expected: recursive grep with line numbers
# Actual: grep: Unknown option: -rn

~/bin/rush -c 'grep -r -n hello /tmp/test.txt'
# This also doesn't work — but that's because -n isn't supported either
```

### Impact

Almost all grep usage by agents uses combined flags (`-rn`, `-ri`, `-rni`). This is standard POSIX flag combining behavior that every command-line tool supports.

### Fix

When parsing flags, if a multi-character flag starting with `-` (not `--`) is not recognized, try splitting it into individual characters and looking up each as a separate flag.

Discovered via imp bash tool dogfooding.
