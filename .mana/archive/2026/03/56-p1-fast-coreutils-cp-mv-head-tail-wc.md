---
id: '56'
title: 'P1: Fast Coreutils — cp, mv, head, tail, wc'
slug: p1-fast-coreutils-cp-mv-head-tail-wc
status: closed
priority: 1
created_at: '2026-03-03T07:20:23.094258Z'
updated_at: '2026-03-03T07:53:13.479630Z'
labels:
- builtins
- coreutils
- performance
- p1
closed_at: '2026-03-03T07:53:13.479630Z'
close_reason: 'Auto-closed: all children completed'
verify: grep -q '"cp"' src/builtins/mod.rs && grep -q '"mv"' src/builtins/mod.rs && grep -q '"head"' src/builtins/mod.rs && grep -q '"tail"' src/builtins/mod.rs && grep -q '"wc"' src/builtins/mod.rs
is_archived: true
---

High-frequency coreutils as in-process builtins for Rush's performance story. See docs/builtin-checklist.md §4.
