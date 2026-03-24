---
id: '12'
title: 'bug: variable expansion fails in redirect targets'
slug: bug-variable-expansion-fails-in-redirect-targets
status: closed
priority: 0
created_at: '2026-03-24T03:11:08.558795Z'
updated_at: '2026-03-24T17:04:47.878401Z'
notes: |2-

  ## Attempt 1 — 2026-03-24T16:35:07Z
  Exit code: 1

  ```
  cat: /tmp/rush-redir-1.txt: No such file or directory
  ```

  ## Attempt 2 — 2026-03-24T16:35:30Z
  Exit code: 1

  ```
  cat: /tmp/rush-redir-1.txt: No such file or directory
  ```


  ---
  2026-03-24T16:56:05.095232+00:00
  ## Attempt 2 Failed (19m59s, 11.6M tokens, $4.537)

  ### What was tried

  - 0 tool calls over 125 turns in 19m59s

  ### Why it failed

  - Timeout (20m)

  ### Verify command

  `~/bin/rush -c 'for i in 1 2; do echo "$i" > /tmp/rush-redir-$i.txt; done' && cat /tmp/rush-redir-1.txt | grep 1 && cat /tmp/rush-redir-2.txt | grep 2`

  ### Suggestion for next attempt

  - Agent ran out of time. Consider increasing the timeout or simplifying the task scope.
labels:
- bug
- parser
- redirect
closed_at: '2026-03-24T17:04:47.878401Z'
verify: ~/bin/rush -c 'for i in 1 2; do echo "$i" > /tmp/rush-redir-$i.txt; done' && cat /tmp/rush-redir-1.txt | grep 1 && cat /tmp/rush-redir-2.txt | grep 2
attempts: 2
claimed_by: pi-agent
claimed_at: '2026-03-24T17:02:10.133728Z'
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-24T16:35:07.196631Z'
  finished_at: '2026-03-24T16:35:07.248503Z'
  duration_secs: 0.051
  result: fail
  exit_code: 1
  output_snippet: 'cat: /tmp/rush-redir-1.txt: No such file or directory'
- attempt: 2
  started_at: '2026-03-24T16:35:30.212408Z'
  finished_at: '2026-03-24T16:35:30.266459Z'
  duration_secs: 0.054
  result: fail
  exit_code: 1
  output_snippet: 'cat: /tmp/rush-redir-1.txt: No such file or directory'
- attempt: 3
  started_at: '2026-03-24T17:04:47.765072Z'
  finished_at: '2026-03-24T17:04:47.832268Z'
  duration_secs: 0.067
  result: pass
  exit_code: 0
outputs:
  text: |-
    1
    2
attempt_log:
- num: 1
  outcome: abandoned
  agent: pi-agent
  started_at: '2026-03-24T16:36:05.076562Z'
  finished_at: '2026-03-24T16:56:05.073770Z'
- num: 2
  outcome: success
  agent: pi-agent
  started_at: '2026-03-24T17:02:10.133728Z'
  finished_at: '2026-03-24T17:04:47.878401Z'
---

## Bug

Variables are not expanded in redirect file paths. The literal `$i` is used as the filename instead of the variable's value.

### Repro

```sh
~/bin/rush -c 'for i in 1 2 3; do echo "$i" > /tmp/rush-test-$i.txt; done'
ls /tmp/rush-test*
# Expected: rush-test-1.txt, rush-test-2.txt, rush-test-3.txt
# Actual: rush-test-$i.txt (literal dollar-sign-i)
```

### Impact

This breaks any loop that writes to dynamically-named files — extremely common in scripts and agent-generated commands. Variable expansion works in command arguments but not in redirect targets.

Discovered via imp bash tool dogfooding — agent tried `for i in $(seq 1 20); do echo "$i" > /tmp/imp-loop-$i.txt; done` and all 20 writes went to the same literal file.
