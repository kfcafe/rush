---
id: '10'
title: 'bug: grep -q (quiet mode) not supported'
slug: bug-grep-q-quiet-mode-not-supported
status: open
priority: 1
created_at: '2026-03-24T02:40:45.241087Z'
updated_at: '2026-03-24T02:40:45.241087Z'
labels:
- bug
- grep
verify: echo "hello" | ~/bin/rush -c 'grep -q hello' && echo pass
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
