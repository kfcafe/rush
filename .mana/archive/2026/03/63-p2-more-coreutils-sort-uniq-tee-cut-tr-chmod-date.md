---
id: '63'
title: 'P2: More Coreutils — sort, uniq, tee, cut, tr, chmod, date, sleep'
slug: p2-more-coreutils-sort-uniq-tee-cut-tr-chmod-date
status: closed
priority: 2
created_at: '2026-03-03T07:21:06.907917Z'
updated_at: '2026-03-03T08:17:52.679306Z'
labels:
- builtins
- coreutils
- p2
closed_at: '2026-03-03T08:17:52.679306Z'
close_reason: 'Auto-closed: all children completed'
verify: grep -q '"sort"' src/builtins/mod.rs && grep -q '"tee"' src/builtins/mod.rs && grep -q '"chmod"' src/builtins/mod.rs && grep -q '"date"' src/builtins/mod.rs
is_archived: true
---

Second wave of coreutils builtins for pipeline and scripting use. See docs/builtin-checklist.md §4.
