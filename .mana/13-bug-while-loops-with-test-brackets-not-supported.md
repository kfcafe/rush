---
id: '13'
title: 'bug: while loops with test brackets [ ] not supported'
slug: bug-while-loops-with-test-brackets-not-supported
status: open
priority: 1
created_at: '2026-03-24T03:11:23.609547Z'
updated_at: '2026-03-24T03:11:23.609547Z'
labels:
- bug
- parser
- while
verify: ~/bin/rush -c 'i=0; while [ $i -lt 3 ]; do echo $i; i=$((i+1)); done' 2>&1 | head -1 | grep 0
---

## Bug

`while [ ... ]` (test bracket syntax) is not recognized by rush's parser.

### Repro

```sh
~/bin/rush -c 'i=1; while [ $i -le 3 ]; do echo $i; i=$((i+1)); done'
# Expected: 1 2 3
# Actual: rush: Invalid token at position 11: '[ $i -le 3 '
```

### Notes

`while` with `[` (test command) is standard POSIX shell. The `[` is actually the `test` builtin. Rush may not recognize `[` as a command name, or may not support `while` at all.

Discovered via imp bash tool dogfooding.
