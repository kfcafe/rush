---
id: '57'
title: 'P1: String Builtin (fish-style)'
slug: p1-string-builtin-fish-style
status: closed
priority: 1
created_at: '2026-03-03T07:20:30.351915Z'
updated_at: '2026-03-03T08:03:03.858660Z'
labels:
- builtins
- modern-shell
- fish-parity
- p1
closed_at: '2026-03-03T08:03:03.858660Z'
close_reason: 'Auto-closed: all children completed'
verify: 'grep -q ''"string"'' src/builtins/mod.rs && cargo test string_builtin --lib 2>&1 | grep -q ''test result: ok'''
is_archived: true
---

Implement fish-style `string` builtin with subcommands: split, join, match, replace, trim, upper, lower, length, sub, pad, repeat, escape, collect. Single biggest gap vs fish — replaces sed/awk/tr/cut without forking. See docs/builtin-checklist.md §6.
