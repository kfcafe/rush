---
id: '65'
title: 'P3: Low Priority — POSIX niche, rare coreutils, future features'
slug: p3-low-priority-posix-niche-rare-coreutils-future
status: closed
priority: 4
created_at: '2026-03-03T07:21:20.323198Z'
updated_at: '2026-03-03T08:35:49.582648Z'
labels:
- builtins
- p3
- future
closed_at: '2026-03-03T08:35:49.582648Z'
close_reason: 'Auto-closed: all children completed'
verify: grep -q '"hash"' src/builtins/mod.rs && grep -q '"ln"' src/builtins/mod.rs
is_archived: true
---

Niche POSIX builtins (hash, fc, times, newgrp), rare coreutils (ln, stat, readlink, mktemp, du, env), and future features (universal variables, event system, funced/funcsave). See docs/builtin-checklist.md §3, §4, P3 summary.
