---
id: '15'
title: 'bug: find -not flag not supported'
slug: bug-find-not-flag-not-supported
status: open
priority: 2
created_at: '2026-03-24T03:28:10.413946Z'
updated_at: '2026-03-24T16:35:30.559906Z'
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
- find
verify: echo x > /tmp/rush-find-a.txt && echo x > /tmp/rush-find-b.log && ~/bin/rush -c 'find /tmp -maxdepth 1 -name "rush-find*" -not -name "*.log"' 2>&1 | grep rush-find-a
fail_first: true
attempts: 2
history:
- attempt: 1
  started_at: '2026-03-24T16:35:07.494037Z'
  finished_at: '2026-03-24T16:35:07.549800Z'
  duration_secs: 0.055
  result: fail
  exit_code: 1
- attempt: 2
  started_at: '2026-03-24T16:35:30.508447Z'
  finished_at: '2026-03-24T16:35:30.559896Z'
  duration_secs: 0.051
  result: fail
  exit_code: 1
---

## Bug

`find -not` not recognized by rush builtin find.

```sh
~/bin/rush -c 'find /tmp -name "*.txt" -not -name "rush*"'
# Actual: find: unknown flag: -not
```

Also need `!` as synonym. Discovered via imp dogfooding.
