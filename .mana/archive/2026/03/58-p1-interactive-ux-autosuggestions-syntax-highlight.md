---
id: '58'
title: 'P1: Interactive UX — Autosuggestions + Syntax Highlighting'
slug: p1-interactive-ux-autosuggestions-syntax-highlight
status: closed
priority: 1
created_at: '2026-03-03T07:20:37.551323Z'
updated_at: '2026-03-03T08:06:28.031956Z'
labels:
- interactive
- ux
- fish-parity
- p1
closed_at: '2026-03-03T08:06:28.031956Z'
close_reason: 'Auto-closed: all children completed'
verify: grep -rq 'autosuggestion\|auto_suggest\|ghost_text' src/ && grep -rq 'syntax_highlight\|SyntaxHighlight\|highlight_line' src/
is_archived: true
---

The #1 and #2 reasons people switch to fish. Autosuggestions show ghost text from history as you type. Syntax highlighting colors commands red (invalid) / green (valid) live. Both are reedline integration tasks. See docs/builtin-checklist.md §10.
