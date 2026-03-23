---
id: '54'
title: 'P0: Quick Wins — umask, touch, history'
slug: p0-quick-wins-umask-touch-history
status: closed
priority: 0
created_at: '2026-03-03T07:20:10.922692Z'
updated_at: '2026-03-03T07:43:31.644502Z'
labels:
- builtins
- posix
- quick-wins
closed_at: '2026-03-03T07:43:31.644502Z'
close_reason: 'Auto-closed: all children completed'
verify: grep -q '"umask"' src/builtins/mod.rs && grep -q '"touch"' src/builtins/mod.rs && grep -q '"history"' src/builtins/mod.rs
is_archived: true
---

Three builtins that are either already written or trivial. See docs/builtin-checklist.md for full context.
