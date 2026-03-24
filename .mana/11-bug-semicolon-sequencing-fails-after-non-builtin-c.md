---
id: '11'
title: 'bug: semicolon sequencing fails after non-builtin commands'
slug: bug-semicolon-sequencing-fails-after-non-builtin-c
status: in_progress
priority: 0
created_at: '2026-03-24T02:43:58.753579Z'
updated_at: '2026-03-24T16:36:05.070308Z'
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
- parser
- critical
verify: ~/bin/rush -c 'sleep 0; echo done' 2>&1 | grep -v "Expected command name" | grep done
attempts: 2
claimed_by: pi-agent
claimed_at: '2026-03-24T16:36:05.070308Z'
history:
- attempt: 1
  started_at: '2026-03-24T16:35:07.098377Z'
  finished_at: '2026-03-24T16:35:07.149618Z'
  duration_secs: 0.051
  result: fail
  exit_code: 1
- attempt: 2
  started_at: '2026-03-24T16:35:30.107726Z'
  finished_at: '2026-03-24T16:35:30.165044Z'
  duration_secs: 0.057
  result: fail
  exit_code: 1
attempt_log:
- num: 1
  outcome: abandoned
  agent: pi-agent
  started_at: '2026-03-24T16:36:05.070308Z'
---

## Bug

Semicolon (`;`) sequencing fails with "Expected command name" after certain commands, but works fine with builtins like `echo`.

### Repro

```sh
# These work:
~/bin/rush -c 'echo a; echo b'        # → a\nb
~/bin/rush -c 'echo a && echo b'       # → a\nb

# These fail:
~/bin/rush -c 'sleep 1; echo done'     # → rush: Expected command name
~/bin/rush -c 'true; echo done'        # → rush: Expected command name
~/bin/rush -c 'ls /tmp; echo done'     # → rush: Expected command name
```

### Precise Pattern (updated after further testing)

**Semicolons fail after external commands, work after builtins.**

```sh
# BUILTINS — all work with semicolons:
rush -c 'echo a; echo b'          # ✓ (echo = builtin)
rush -c 'cat /dev/null; echo a'   # ✓ (cat = builtin in rush)
rush -c 'ls /tmp; echo a'         # ✓ (ls = builtin in rush)
rush -c 'true; echo a'            # ✓ (true = builtin)
rush -c 'false; echo a'           # ✓ (false = builtin)

# EXTERNAL COMMANDS — all fail with semicolons:
rush -c 'sleep 0; echo done'      # ✗ "Expected command name"
rush -c 'date; echo done'         # ✗ "Expected command name"  
rush -c 'head -1 /dev/null; echo' # ✗ "Expected command name"
rush -c 'wc -l /dev/null; echo'   # ✗ "Expected command name"
rush -c 'tail -1 /dev/null; echo' # ✗ "Expected command name"

# && works fine for ALL commands (builtin and external):
rush -c 'sleep 0 && echo done'    # ✓
rush -c 'date && echo done'       # ✓
```

The bug is in how the executor transitions after an external (forked) command completes — the semicolon continuation path doesn't properly reset the parser state, while `&&` does.

### Impact

Critical — semicolons are the most basic command sequencing operator. This breaks most multi-command one-liners, verify gates, and agent-generated scripts. Agents will constantly generate `cmd1; cmd2` patterns.

### Additional note

`/dev/null` is reported as "Is a directory" by rush's builtin cat, which is a separate bug.

Discovered via imp bash tool dogfooding.
