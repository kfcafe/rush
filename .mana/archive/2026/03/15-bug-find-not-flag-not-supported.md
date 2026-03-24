---
id: '15'
title: 'bug: find -not flag not supported'
slug: bug-find-not-flag-not-supported
status: closed
priority: 2
created_at: '2026-03-24T03:28:10.413946Z'
updated_at: '2026-03-24T17:36:24.492945Z'
notes: |2

  ## Attempt 1 — 2026-03-24T16:35:07Z
  Exit code: 1

  ```

  ```

  ## Attempt 2 — 2026-03-24T16:35:30Z
  Exit code: 1

  ```

  ```

  ## Attempt 3 — 2026-03-24T17:28:25Z
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
closed_at: '2026-03-24T17:36:24.492945Z'
verify: echo x > /tmp/rush-find-a.txt && echo x > /tmp/rush-find-b.log && ~/bin/rush -c 'find /tmp -maxdepth 1 -name "rush-find*" -not -name "*.log"' 2>&1 | grep rush-find-a
fail_first: true
checkpoint: '13f0df43989fa88f9c5c923aa07e93a5ace0e1ad'
attempts: 4
claimed_by: pi-agent
claimed_at: '2026-03-24T17:28:47.695714Z'
is_archived: true
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
- attempt: 3
  started_at: '2026-03-24T17:28:25.075992Z'
  finished_at: '2026-03-24T17:28:25.129618Z'
  duration_secs: 0.053
  result: fail
  exit_code: 1
- attempt: 4
  started_at: '2026-03-24T17:28:41.562914Z'
  finished_at: '2026-03-24T17:28:41.621032Z'
  duration_secs: 0.058
  result: fail
  exit_code: 1
- attempt: 5
  started_at: '2026-03-24T17:36:24.404956Z'
  finished_at: '2026-03-24T17:36:24.463888Z'
  duration_secs: 0.058
  result: pass
  exit_code: 0
outputs:
  text: /tmp/rush-find-a.txt
attempt_log:
- num: 1
  outcome: success
  agent: pi-agent
  started_at: '2026-03-24T17:28:47.695714Z'
  finished_at: '2026-03-24T17:36:24.492945Z'
---

## Bug

`find -not` not recognized by rush builtin find.

```sh
~/bin/rush -c 'find /tmp -name "*.txt" -not -name "rush*"'
# Actual: find: unknown flag: -not
```

Also need `!` as synonym. Discovered via imp dogfooding.
