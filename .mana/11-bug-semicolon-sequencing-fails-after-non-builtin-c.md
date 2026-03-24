---
id: '11'
title: 'bug: semicolon sequencing fails after non-builtin commands'
slug: bug-semicolon-sequencing-fails-after-non-builtin-c
status: open
priority: 0
created_at: '2026-03-24T02:43:58.753579Z'
updated_at: '2026-03-24T02:43:58.753579Z'
labels:
- bug
- parser
- critical
verify: ~/bin/rush -c 'sleep 0; echo done' 2>&1 | grep -v "Expected command name" | grep done
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
