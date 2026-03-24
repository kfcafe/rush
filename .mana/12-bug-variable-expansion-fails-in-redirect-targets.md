---
id: '12'
title: 'bug: variable expansion fails in redirect targets'
slug: bug-variable-expansion-fails-in-redirect-targets
status: open
priority: 0
created_at: '2026-03-24T03:11:08.558795Z'
updated_at: '2026-03-24T03:11:08.558795Z'
labels:
- bug
- parser
- redirect
verify: ~/bin/rush -c 'for i in 1 2; do echo "$i" > /tmp/rush-redir-$i.txt; done' && cat /tmp/rush-redir-1.txt | grep 1 && cat /tmp/rush-redir-2.txt | grep 2
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
