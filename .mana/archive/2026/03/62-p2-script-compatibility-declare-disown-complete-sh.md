---
id: '62'
title: 'P2: Script Compatibility — declare, disown, complete, shopt, mapfile'
slug: p2-script-compatibility-declare-disown-complete-sh
status: closed
priority: 2
created_at: '2026-03-03T07:21:01.415404Z'
updated_at: '2026-03-03T08:25:38.570480Z'
labels:
- builtins
- bash-compat
- p2
closed_at: '2026-03-03T08:25:38.570480Z'
close_reason: 'Auto-closed: all children completed'
verify: grep -q '"declare"' src/builtins/mod.rs && grep -q '"disown"' src/builtins/mod.rs && grep -q '"complete"' src/builtins/mod.rs
is_archived: true
---

Bash-ism builtins commonly found in scripts and dotfiles. See docs/builtin-checklist.md §3.
