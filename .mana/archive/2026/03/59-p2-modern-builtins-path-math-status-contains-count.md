---
id: '59'
title: 'P2: Modern Builtins — path, math, status, contains, count'
slug: p2-modern-builtins-path-math-status-contains-count
status: closed
priority: 2
created_at: '2026-03-03T07:20:42.496273Z'
updated_at: '2026-03-03T08:08:02.724375Z'
labels:
- builtins
- modern-shell
- p2
closed_at: '2026-03-03T08:08:02.724375Z'
close_reason: 'Auto-closed: all children completed'
verify: grep -q '"path"' src/builtins/mod.rs && grep -q '"math"' src/builtins/mod.rs && grep -q '"status"' src/builtins/mod.rs
is_archived: true
---

Modern shell builtins inspired by fish/nushell. See docs/builtin-checklist.md §7, §8, §13.
