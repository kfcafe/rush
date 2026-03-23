---
id: '64'
title: 'P2: Git Builtins — git add, commit, branch'
slug: p2-git-builtins-git-add-commit-branch
status: closed
priority: 2
created_at: '2026-03-03T07:21:12.581233Z'
updated_at: '2026-03-03T08:24:44.086267Z'
labels:
- builtins
- git
- p2
closed_at: '2026-03-03T08:24:44.086267Z'
close_reason: 'Auto-closed: all children completed'
verify: grep -rq 'git_add\|"add"' src/builtins/git_*.rs && grep -rq 'git_commit\|"commit"' src/builtins/git_*.rs
is_archived: true
---

Complete the native git story: add, commit, branch via git2 bindings. Currently only status and log are native. See docs/builtin-checklist.md §5.
