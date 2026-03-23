---
id: '55'
title: 'P1: POSIX Compliance — ulimit, git diff, pushd/popd'
slug: p1-posix-compliance-ulimit-git-diff-pushdpopd
status: closed
priority: 1
created_at: '2026-03-03T07:20:16.461377Z'
updated_at: '2026-03-03T07:47:55.622891Z'
labels:
- builtins
- posix
- p1
closed_at: '2026-03-03T07:47:55.622891Z'
close_reason: 'Auto-closed: all children completed'
verify: grep -q '"ulimit"' src/builtins/mod.rs && grep -q '"pushd"' src/builtins/mod.rs && grep 'git_diff' src/builtins/mod.rs | grep -vq '//'
is_archived: true
---

POSIX builtins that can't be external + existing code that needs fixing. See docs/builtin-checklist.md.
